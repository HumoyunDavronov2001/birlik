//! A ring of two nodes on this machine generates exactly what one device generates. Built only with the
//! `engine-tests` feature; needs `BIRLIK_ENGINE` (the engine executable) and `BIRLIK_TEST_MODEL` (a small
//! llama-architecture GGUF model, e.g. stories15M-q8_0.gguf).

use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};

use birlik_engine_host::{Backend, DType, Engine, Input, SamplerParams, Step};
use birlik_network::Peer;
use birlik_node::ring::{self, Config, Window};

const PROMPT: &str = "Once upon a time, there was a little girl named Lily.";
const GENERATE: u32 = 24;

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name} to run the ring test")))
}

fn free_port() -> SocketAddr {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap()
}

/// Greedy generation on one device: the whole model in one shard.
fn generate_whole(engine_path: &Path, model: &Path, layers: u32) -> Vec<i32> {
    let mut engine = Engine::spawn(engine_path, 16 << 20).unwrap();
    let whole = engine.open(model, 0..layers + 1, Backend::Cpu, 512).unwrap();
    let mut tokens = engine.tokenize(whole.id, PROMPT, true).unwrap();
    let sampler = engine.sampler_open(SamplerParams::default()).unwrap();
    let mut position = 0;
    let mut generated = Vec::new();
    while generated.len() < GENERATE as usize {
        let step = Step { request: 1, position, input: Input::Tokens(&tokens), all_logits: false, output: DType::F32 };
        engine.run(&whole, &step).unwrap();
        position += u32::try_from(tokens.len()).unwrap();
        let token = engine.sample(sampler, whole.model.vocab_size).unwrap();
        generated.push(token);
        if engine.token_text(whole.id, token).unwrap().1 {
            break;
        }
        tokens = vec![token];
    }
    engine.quit().unwrap();
    generated
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_ring_generates_what_one_device_generates() {
    let engine = env_path("BIRLIK_ENGINE");
    let model = env_path("BIRLIK_TEST_MODEL");
    let layers = {
        let mut probe = Engine::spawn(&engine, 1 << 20).unwrap();
        let info = probe.model_info(&model).unwrap();
        probe.quit().unwrap();
        info.layer_count
    };

    let dir = std::env::temp_dir().join(format!("birlik-ring-test-{}", std::process::id()));
    let (head_key, worker_key) = (dir.join("head.key"), dir.join("worker.key"));
    let head_id = ring::node_id(&head_key).unwrap();
    let worker_id = ring::node_id(&worker_key).unwrap();
    let (head_addr, worker_addr) = (free_port(), free_port());

    let config = |windows: Vec<Window>, listen, next, prev, key, prompt| Config {
        model: model.clone(),
        windows,
        listen,
        next,
        prev,
        key,
        engine: engine.clone(),
        backend: Backend::Cpu,
        max_tokens: 512,
        chunk: 8, // a prompt of several chunks
        wire: DType::F32,
        prompt,
        generate: GENERATE,
    };
    let half = layers / 2;
    let worker = config(
        vec![Window(half..layers)],
        worker_addr,
        Peer { id: head_id, addr: head_addr },
        head_id,
        worker_key,
        None,
    );
    let head = config(
        vec![Window(0..half), Window(layers..layers + 1)],
        head_addr,
        Peer { id: worker_id, addr: worker_addr },
        worker_id,
        head_key,
        Some(PROMPT.to_owned()),
    );

    let worker_task = tokio::spawn(ring::serve(worker));
    let report = ring::generate(head).await.unwrap();
    worker_task.abort();

    let expected = tokio::task::spawn_blocking(move || generate_whole(&engine, &model, layers)).await.unwrap();
    assert_eq!(report.generated, expected, "ring: {:?}", report.text);
    assert!(report.prompt_tokens > 8);
    assert!(report.tpot.is_some());
    let _ = std::fs::remove_dir_all(dir);
}
