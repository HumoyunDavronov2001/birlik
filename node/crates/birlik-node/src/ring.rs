//! A ring of devices that runs one model together — prima.cpp's piped ring on BIRLIK's engine and network.
//!
//! Every node of the ring holds windows of layers. The node that serves the request, the head, holds the first
//! layers and the output head; the hidden state of every token travels around the ring as tensor frames, one window
//! after another, in as many rounds as the nodes have windows, and comes back to the head as logits. While the other
//! nodes compute, a node reads its next window's weights from disk (prefetching), so a model larger than a device's
//! memory still runs.
//!
//! Stage B1 wires the ring by hand: each node is told its windows, the next node (`<id>@<ip:port>`) and the previous
//! node's id. Discovery, pairing and planning come in stages B2 and B3. Every node of a ring must use the same wire
//! format for hidden states.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Write as _;
use std::net::SocketAddr;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use birlik_engine_host::{Backend, DType, Engine, Input, SamplerParams, ShardInfo, Step};
use birlik_network::{Connection, Endpoint, EndpointId, Peer, SendStream};
use birlik_protocol::frame::{Flags, FrameHeader};
use tokio::sync::mpsc;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A window of layers written `begin:end`; layer N of an N-layer model is the output head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window(pub Range<u32>);

impl FromStr for Window {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let (a, b) = s.split_once(':').ok_or_else(|| format!("expected begin:end, got {s}"))?;
        let begin = a.trim().parse().map_err(|_| format!("bad layer {a}"))?;
        let end = b.trim().parse().map_err(|_| format!("bad layer {b}"))?;
        if begin >= end {
            return Err(format!("the window {s} is empty"));
        }
        Ok(Self(begin..end))
    }
}

impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0.start, self.0.end)
    }
}

/// What a ring node needs to know.
#[derive(Debug)]
pub struct Config {
    pub model: PathBuf,
    pub windows: Vec<Window>,
    pub listen: SocketAddr,
    pub next: Peer,
    pub prev: EndpointId,
    pub key: PathBuf,
    pub engine: PathBuf,
    pub backend: Backend,
    pub max_tokens: u32,
    /// Tokens per prefill step.
    pub chunk: u32,
    /// Element type of the hidden state between nodes: F16 (default), Bf16 or F32.
    pub wire: DType,
    /// Only on the head.
    pub prompt: Option<String>,
    pub generate: u32,
}

/// Timings of one generation, measured on the head.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub prompt_tokens: usize,
    pub generated: Vec<i32>,
    pub text: String,
    /// Time to first token.
    pub ttft: Duration,
    /// Mean time per output token after the first.
    pub tpot: Option<Duration>,
}

/// The engine is synchronous; calls run on blocking threads.
type SharedEngine = Arc<Mutex<Engine>>;

async fn with_engine<T: Send + 'static>(
    engine: &SharedEngine,
    f: impl FnOnce(&mut Engine) -> Result<T, birlik_engine_host::Error> + Send + 'static,
) -> Result<T, BoxError> {
    let engine = Arc::clone(engine);
    let result = tokio::task::spawn_blocking(move || {
        let mut guard = engine.lock().map_err(|_| "the engine lock is poisoned".to_owned())?;
        f(&mut guard).map_err(|e| e.to_string())
    })
    .await?;
    result.map_err(Into::into)
}

/// The shards of this node, in ring order.
struct Node {
    engine: SharedEngine,
    shards: Vec<ShardInfo>,
    hidden_dim: u32,
    vocab_size: u32,
    wire: DType,
}

impl Node {
    /// Runs a frame through one of this node's shards; returns its output bytes.
    async fn run(&self, shard: usize, header: &FrameHeader, payload: Vec<u8>) -> Result<Vec<u8>, BoxError> {
        let info = self.shards[shard].clone();
        let (request, position, count) = (header.request_id, header.position_start, u32::from(header.token_count));
        let (dtype, wire) = (header.dtype, self.wire);
        let output = with_engine(&self.engine, move |engine| {
            let input = Input::Hidden { dtype, count, data: &payload };
            let step = Step { request, position, input, all_logits: false, output: wire };
            engine.run(&info, &step).map(|out| out.data.to_vec())
        })
        .await?;
        // the next round's window is read from disk while the rest of the ring computes
        if self.shards.len() > 1 {
            let next = self.shards[(shard + 1) % self.shards.len()].id;
            with_engine(&self.engine, move |engine| engine.prefetch(next)).await?;
        }
        Ok(output)
    }

    async fn run_tokens(
        &self,
        shard: usize,
        request: u64,
        position: u32,
        tokens: Vec<i32>,
    ) -> Result<Vec<u8>, BoxError> {
        let info = self.shards[shard].clone();
        let wire = self.wire;
        with_engine(&self.engine, move |engine| {
            let step = Step { request, position, input: Input::Tokens(&tokens), all_logits: false, output: wire };
            engine.run(&info, &step).map(|out| out.data.to_vec())
        })
        .await
    }

    async fn drop_requests(&self, requests: &HashSet<u64>) {
        for &request in requests {
            for shard in &self.shards {
                let id = shard.id;
                // a failed drop only leaves KV cache cells behind; the next session reuses the shard anyway
                let _ = with_engine(&self.engine, move |engine| engine.drop_request(id, request)).await;
            }
        }
    }

    fn frame(
        &self,
        request: u64,
        step: u32,
        position: u32,
        count: u32,
        to_logits: bool,
    ) -> Result<FrameHeader, BoxError> {
        let flags = if to_logits { Flags::LAST_STAGE } else { Flags::empty() };
        let token_count = u16::try_from(count)?;
        let hidden = u16::try_from(self.hidden_dim)?;
        Ok(FrameHeader::new(self.wire, flags, request, step, position, token_count, hidden)?)
    }
}

/// Starts the engine and opens this node's windows.
fn open_node(config: &Config) -> Result<Node, BoxError> {
    // each half of the shared memory holds one prefill chunk of f32 hidden states or one row of logits
    let mut probe = Engine::spawn(&config.engine, 1 << 20)?;
    let info = probe.model_info(&config.model)?;
    probe.quit()?;
    let hidden = info.hidden_dim as usize;
    let half = (config.chunk as usize * hidden * 4).max(info.vocab_size as usize * 4) + (1 << 16);
    let mut engine = Engine::spawn(&config.engine, 2 * half)?;

    let mut shards = Vec::new();
    for window in &config.windows {
        if window.0.end > info.layer_count + 1 {
            return Err(format!("window {window} ends after the output head (layer {})", info.layer_count).into());
        }
        let shard = engine.open(&config.model, window.0.clone(), config.backend, config.max_tokens)?;
        let backend = shard.backend.as_str_name().trim_start_matches("BACKEND_").to_lowercase();
        eprintln!("birlik: layers {window} loaded on {backend}");
        shards.push(shard);
    }
    Ok(Node {
        engine: Arc::new(Mutex::new(engine)),
        shards,
        hidden_dim: info.hidden_dim,
        vocab_size: info.vocab_size,
        wire: config.wire,
    })
}

/// Connects to the next node, retrying while it starts, and opens one link per window boundary that leads to it.
async fn connect_next(
    endpoint: &Endpoint,
    config: &Config,
    node: &Node,
) -> Result<(Connection, HashMap<u32, SendStream>), BoxError> {
    let connection = loop {
        match birlik_network::connect(endpoint, &config.next).await {
            Ok(c) => break c,
            Err(e) => {
                eprintln!("birlik: waiting for the next node ({e})");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    };
    let mut links = HashMap::new();
    for shard in &node.shards {
        if !shard.gives_logits() {
            // the window that starts where this one ends lives on the next node
            links.insert(shard.layers.end, birlik_network::open_link(&connection, shard.layers.end).await?);
        }
    }
    Ok((connection, links))
}

/// One frame for one of this node's windows.
struct Inbound {
    shard: usize,
    header: FrameHeader,
    payload: Vec<u8>,
}

/// Accepts the previous node's connection and the links it opens, one per window of this node that takes hidden
/// states, and forwards their frames to `inbox`.
async fn accept_prev(
    endpoint: &Endpoint,
    config: &Config,
    node: &Node,
    inbox: mpsc::Sender<Inbound>,
) -> Result<Connection, BoxError> {
    let connection = loop {
        let incoming = endpoint.accept().await.ok_or("the endpoint closed")?;
        let connection = incoming.await?;
        if connection.remote_id() == config.prev {
            break connection;
        }
        eprintln!("birlik: refused a connection from {}, not the previous node", connection.remote_id());
        connection.close(1u32.into(), b"not the previous node");
    };
    let expected = node.shards.iter().filter(|s| !s.takes_tokens()).count();
    for _ in 0..expected {
        let (begin, mut stream) = birlik_network::accept_link(&connection).await?.ok_or("the previous node left")?;
        let shard = node
            .shards
            .iter()
            .position(|s| s.layers.start == begin)
            .ok_or_else(|| format!("the previous node feeds layer {begin}, which this node does not hold"))?;
        let inbox = inbox.clone();
        tokio::spawn(async move {
            let mut payload = Vec::new();
            while let Ok(Some(header)) = birlik_network::read_frame(&mut stream, &mut payload).await {
                if inbox.send(Inbound { shard, header, payload: std::mem::take(&mut payload) }).await.is_err() {
                    break;
                }
            }
        });
    }
    Ok(connection)
}

/// Runs a node that serves the ring until it is stopped: every session starts when the previous node connects and
/// ends when it leaves.
///
/// # Errors
/// The engine or the model cannot be opened, the address cannot be bound, or a link breaks inside a session.
pub async fn serve(config: Config) -> Result<(), BoxError> {
    let node = open_node(&config)?;
    let endpoint = bind(&config).await?;
    loop {
        let (tx, mut inbox) = mpsc::channel(256);
        let prev = accept_prev(&endpoint, &config, &node, tx).await?;
        let (next, mut links) = connect_next(&endpoint, &config, &node).await?;
        eprintln!("birlik: ring session started");
        let mut requests = HashSet::new();
        loop {
            tokio::select! {
                message = inbox.recv() => {
                    let Some(Inbound { shard, header, payload }) = message else { break };
                    requests.insert(header.request_id);
                    let output = node.run(shard, &header, payload).await?;
                    let info = &node.shards[shard];
                    let count = u32::from(header.token_count);
                    let to_logits = info.layers.end == info.model.layer_count;
                    let frame = node.frame(header.request_id, header.step, header.position_start, count, to_logits)?;
                    let link = links.get_mut(&info.layers.end).ok_or("no link for the next window")?;
                    birlik_network::write_frame(link, &frame, &output).await?;
                }
                _ = prev.closed() => break,
            }
        }
        node.drop_requests(&requests).await;
        next.close(0u32.into(), b"session over");
        eprintln!("birlik: ring session ended");
    }
}

/// Runs the head: sends the prompt around the ring, samples every next token from the logits that come back, and
/// reports the timings.
///
/// # Errors
/// There is no prompt, the head lacks layer 0 or the output head, the engine fails, or the ring breaks.
pub async fn generate(config: Config) -> Result<Report, BoxError> {
    let prompt = config.prompt.clone().ok_or("the head needs a prompt")?;
    let node = open_node(&config)?;
    let first = node.shards.iter().position(ShardInfo::takes_tokens).ok_or("the head holds no window at layer 0")?;
    if !node.shards.iter().any(ShardInfo::gives_logits) {
        return Err("the head must hold the output head too (layer N, e.g. N:N+1)".into());
    }
    let endpoint = bind(&config).await?;
    let (next, mut links) = connect_next(&endpoint, &config, &node).await?;
    let (tx, mut inbox) = mpsc::channel(256);
    let _prev = accept_prev(&endpoint, &config, &node, tx).await?;

    let shard_id = node.shards[first].id;
    let tokens = with_engine(&node.engine, move |engine| engine.tokenize(shard_id, &prompt, true)).await?;
    let sampler = with_engine(&node.engine, |engine| engine.sampler_open(SamplerParams::default())).await?;
    let request = u64::from(std::process::id()) << 32 | u64::from(rand_u32());
    let prompt_len = u32::try_from(tokens.len())?;
    let start = Instant::now();

    // prefill: the prompt goes around the ring in chunks, one after another
    let first_end = node.shards[first].layers.end;
    for (i, chunk) in tokens.chunks(config.chunk as usize).enumerate() {
        let position = u32::try_from(i * config.chunk as usize)?;
        let count = u32::try_from(chunk.len())?;
        let output = node.run_tokens(first, request, position, chunk.to_vec()).await?;
        let frame = node.frame(request, 0, position, count, false)?;
        let link = links.get_mut(&first_end).ok_or("no link after the first window")?;
        birlik_network::write_frame(link, &frame, &output).await?;
    }

    let mut generated = Vec::new();
    let mut text = String::new();
    let mut times = Vec::new();
    let mut stdout = std::io::stdout();
    while let Some(Inbound { shard, header, payload }) = inbox.recv().await {
        let info = node.shards[shard].clone();
        let count = u32::from(header.token_count);
        let output = node.run(shard, &header, payload).await?;
        if !info.gives_logits() {
            let to_logits = info.layers.end == info.model.layer_count;
            let frame = node.frame(request, header.step, header.position_start, count, to_logits)?;
            let link = links.get_mut(&info.layers.end).ok_or("no link for the next window")?;
            birlik_network::write_frame(link, &frame, &output).await?;
            continue;
        }
        // logits: only the end of the prompt and every decoded token pick a next token
        let position = header.position_start + count;
        if position < prompt_len {
            continue;
        }
        let vocab = node.vocab_size;
        let token = with_engine(&node.engine, move |engine| engine.sample(sampler, vocab)).await?;
        times.push(start.elapsed());
        let (piece, is_end) = with_engine(&node.engine, move |engine| engine.token_text(shard_id, token)).await?;
        generated.push(token);
        text.push_str(&piece);
        print!("{piece}");
        let _ = stdout.flush();
        if is_end || generated.len() >= config.generate as usize {
            break;
        }
        let step = u32::try_from(generated.len())?;
        let output = node.run_tokens(first, request, position, vec![token]).await?;
        let frame = node.frame(request, step, position, 1, false)?;
        let link = links.get_mut(&first_end).ok_or("no link after the first window")?;
        birlik_network::write_frame(link, &frame, &output).await?;
    }
    println!();

    with_engine(&node.engine, move |engine| engine.sampler_close(sampler)).await?;
    node.drop_requests(&HashSet::from([request])).await;
    next.close(0u32.into(), b"done");
    endpoint.close().await;

    let ttft = times.first().copied().unwrap_or_default();
    let tpot = (times.len() > 1).then(|| {
        let decode = times[times.len() - 1].saturating_sub(times[0]);
        decode / u32::try_from(times.len() - 1).unwrap_or(u32::MAX)
    });
    Ok(Report { prompt_tokens: tokens.len(), generated, text, ttft, tpot })
}

async fn bind(config: &Config) -> Result<Endpoint, BoxError> {
    let key = birlik_network::load_or_create_key(&config.key)?;
    let endpoint = birlik_network::bind(key, config.listen).await?;
    eprintln!("birlik: node {} listening on {}", endpoint.id(), config.listen);
    Ok(endpoint)
}

fn rand_u32() -> u32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    nanos ^ std::process::id().rotate_left(16)
}

/// Where this device keeps its node key.
#[must_use]
pub fn default_key_path() -> PathBuf {
    data_dir().join("birlik").join("node.key")
}

fn data_dir() -> PathBuf {
    let env = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) {
        env("APPDATA").unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        env("HOME").map_or_else(|| PathBuf::from("."), |h| h.join("Library/Application Support"))
    } else {
        env("XDG_DATA_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// The engine executable next to this program, unless given.
#[must_use]
pub fn default_engine_path() -> PathBuf {
    let name = if cfg!(windows) { "birlik-engine.exe" } else { "birlik-engine" };
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Prints this node's id, creating its key if needed.
///
/// # Errors
/// The key file cannot be read or written.
pub fn node_id(key: &Path) -> Result<EndpointId, BoxError> {
    Ok(birlik_network::load_or_create_key(key)?.public())
}

#[cfg(test)]
mod tests {
    use super::Window;

    #[test]
    fn windows_parse() {
        assert_eq!("0:20".parse::<Window>().unwrap().0, 0..20);
        assert_eq!(" 80 : 81 ".parse::<Window>().unwrap().0, 80..81);
        assert!("20:20".parse::<Window>().is_err());
        assert!("20".parse::<Window>().is_err());
        assert_eq!(Window(3..9).to_string(), "3:9");
    }
}
