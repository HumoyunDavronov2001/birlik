//! The cluster's agreed state under a simulated network (PLAN.md §8, SPEC NO-50).
//!
//! Each scenario runs three or five computers on simulated hosts ([turmoil](https://github.com/tokio-rs/turmoil)):
//! the real Raft of `omnarc-node` with its real log, kept in memory across restarts, talking over simulated TCP.
//! While every node keeps proposing changes, the scenario crashes and restarts nodes, cuts and heals links, and loses
//! and delays messages, all drawn from one seed. Then it heals everything and checks that
//!
//! - no two nodes that applied the same log entries ever disagree (checked throughout),
//! - every change a leader acknowledged is in the state of every node — nothing committed is lost,
//! - a leader is elected and all nodes agree again within a minute.
//!
//! openraft draws its own election timeouts, so a failing seed replays the same faults, not always the same
//! interleaving.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use omnarc_node::consensus::{Agreed, Command, Raft, TypeConfig, storage};
use openraft::error::{InstallSnapshotError, NetworkError, RPCError, RaftError, RemoteError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse, VoteRequest,
    VoteResponse,
};
use openraft::{BasicNode, Config, ServerState};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use redb::Database;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use turmoil::net::{TcpListener, TcpStream};

const PORT: u16 = 9000;

/// How long faults are injected, in simulated time.
const CHAOS: Duration = Duration::from_secs(40);

/// How long the nodes have to agree again once everything is healed.
const SETTLE: Duration = Duration::from_secs(60);

type NodeId = u64;
type BoxError = Box<dyn std::error::Error>;

/// What a scenario did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub seed: u64,
    pub nodes: usize,
    /// Faults injected: crashes, restarts, partitions, repairs, loss and delay changes.
    pub faults: usize,
    /// Changes acknowledged as committed.
    pub committed: usize,
}

fn host(id: NodeId) -> String {
    format!("node{id}")
}

#[derive(Debug, Serialize, Deserialize)]
enum Call {
    Append(AppendEntriesRequest<TypeConfig>),
    Vote(VoteRequest<NodeId>),
    Snapshot(InstallSnapshotRequest<TypeConfig>),
    Write(Command),
}

#[derive(Debug, Serialize, Deserialize)]
enum Answer {
    Append(Result<AppendEntriesResponse<NodeId>, RaftError<NodeId>>),
    Vote(Result<VoteResponse<NodeId>, RaftError<NodeId>>),
    Snapshot(Result<InstallSnapshotResponse<NodeId>, RaftError<NodeId, InstallSnapshotError>>),
    Write(Result<(), String>),
}

async fn send<T: Serialize>(stream: &mut TcpStream, message: &T) -> Result<(), BoxError> {
    let body = serde_json::to_vec(message)?;
    stream.write_all(&u32::try_from(body.len())?.to_le_bytes()).await?;
    stream.write_all(&body).await?;
    Ok(())
}

async fn receive<T: for<'de> Deserialize<'de>>(stream: &mut TcpStream) -> Result<T, BoxError> {
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).await?;
    let mut body = vec![0u8; usize::try_from(u32::from_le_bytes(len))?];
    stream.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

/// One call to another node, on a connection of its own.
async fn call(target: NodeId, message: &Call) -> Result<Answer, Failure> {
    let connect = tokio::time::timeout(Duration::from_secs(1), TcpStream::connect((host(target), PORT))).await;
    let mut stream = match connect {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(Failure::Unreachable(e.to_string())),
        Err(_) => return Err(Failure::Unreachable("connecting timed out".into())),
    };
    let exchange = async {
        send(&mut stream, message).await?;
        receive::<Answer>(&mut stream).await
    };
    match tokio::time::timeout(Duration::from_secs(2), exchange).await {
        Ok(Ok(answer)) => Ok(answer),
        Ok(Err(e)) => Err(Failure::Network(e.to_string())),
        Err(_) => Err(Failure::Network("no answer in time".into())),
    }
}

#[derive(Debug)]
struct Message(String);

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Message {}

enum Failure {
    Unreachable(String),
    Network(String),
}

impl<E: std::error::Error> From<Failure> for RPCError<NodeId, BasicNode, E> {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::Unreachable(e) => RPCError::Unreachable(Unreachable::new(&Message(e))),
            Failure::Network(e) => RPCError::Network(NetworkError::new(&Message(e))),
        }
    }
}

fn unexpected<E: std::error::Error>() -> RPCError<NodeId, BasicNode, E> {
    RPCError::Network(NetworkError::new(&Message("another answer".into())))
}

/// The simulated network as Raft sees it.
#[derive(Debug, Clone)]
struct Network;

impl RaftNetworkFactory<TypeConfig> for Network {
    type Network = Client;

    #[allow(clippy::unused_async_trait_impl)] // the trait is async
    async fn new_client(&mut self, target: NodeId, _node: &BasicNode) -> Client {
        Client { target }
    }
}

#[derive(Debug)]
struct Client {
    target: NodeId,
}

impl RaftNetwork<TypeConfig> for Client {
    async fn append_entries(
        &mut self,
        request: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        match call(self.target, &Call::Append(request)).await? {
            Answer::Append(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }

    async fn install_snapshot(
        &mut self,
        request: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<InstallSnapshotResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>>
    {
        match call(self.target, &Call::Snapshot(request)).await? {
            Answer::Snapshot(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }

    async fn vote(
        &mut self,
        request: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        match call(self.target, &Call::Vote(request)).await? {
            Answer::Vote(result) => result.map_err(|e| RPCError::RemoteError(RemoteError::new(self.target, e))),
            _ => Err(unexpected()),
        }
    }
}

async fn answer(raft: Raft, mut stream: TcpStream) -> Result<(), BoxError> {
    let request: Call = receive(&mut stream).await?;
    let answer = match request {
        Call::Append(request) => Answer::Append(raft.append_entries(request).await),
        Call::Vote(request) => Answer::Vote(raft.vote(request).await),
        Call::Snapshot(request) => Answer::Snapshot(raft.install_snapshot(request).await),
        Call::Write(command) => Answer::Write(raft.client_write(command).await.map(|_| ()).map_err(|e| e.to_string())),
    };
    send(&mut stream, &answer).await
}

/// Commits `command` through `raft`, or through the leader it names.
async fn write(raft: &Raft, command: Command) -> Result<(), String> {
    match raft.client_write(command.clone()).await {
        Ok(_) => Ok(()),
        Err(RaftError::APIError(openraft::error::ClientWriteError::ForwardToLeader(forward))) => {
            let leader = forward.leader_id.ok_or("no leader")?;
            match call(leader, &Call::Write(command)).await {
                Ok(Answer::Write(result)) => result,
                Ok(_) => Err("another answer".into()),
                Err(Failure::Unreachable(e) | Failure::Network(e)) => Err(e),
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// What the hosts share with the scenario: the running nodes and the changes committed.
#[derive(Default)]
struct World {
    running: HashMap<NodeId, (Raft, watch::Receiver<Agreed>)>,
    committed: BTreeSet<String>,
    next_change: u64,
}

fn config() -> Arc<Config> {
    let config = Config {
        cluster_name: "simulation".into(),
        heartbeat_interval: 300,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        ..Config::default()
    };
    Arc::new(config.validate().expect("a valid configuration"))
}

/// Runs the scenario of `seed`.
///
/// # Errors
/// An invariant breaks: two nodes disagree on the same applied log, a committed change is lost, or the nodes do not
/// agree again in time.
///
/// # Panics
/// Never: the simulation's own setup is fixed.
pub fn run(seed: u64) -> Result<Outcome, String> {
    let mut rng = StdRng::seed_from_u64(seed);
    let nodes: usize = if rng.random_bool(0.5) { 3 } else { 5 };
    let ids: Vec<NodeId> = (1..=nodes as u64).collect();
    let world = Rc::new(RefCell::new(World::default()));
    let mut sim = turmoil::Builder::new()
        .simulation_duration(CHAOS + SETTLE + Duration::from_secs(30))
        .tick_duration(Duration::from_millis(10))
        .min_message_latency(Duration::from_millis(1))
        .max_message_latency(Duration::from_millis(20))
        .rng_seed(seed)
        .build();

    for &id in &ids {
        // the log outlives the host: a restarted node finds what it wrote
        let db =
            Arc::new(Database::builder().create_with_backend(redb::backends::InMemoryBackend::new()).expect("memory"));
        let world = Rc::clone(&world);
        let voters: BTreeMap<NodeId, BasicNode> = ids.iter().map(|&i| (i, BasicNode { addr: host(i) })).collect();
        sim.host(host(id), move || {
            let (db, world, voters) = (Arc::clone(&db), Rc::clone(&world), voters.clone());
            async move {
                let (log, machine, agreed) = storage::open_db(db).map_err(|e| e.to_string())?;
                let raft = Raft::new(id, config(), Network, log, machine).await?;
                if id == 1 {
                    // the first start of the first node; afterwards the log is there and this is refused
                    let _ = raft.initialize(voters).await;
                }
                world.borrow_mut().running.insert(id, (raft.clone(), agreed));
                let listener = TcpListener::bind(("0.0.0.0", PORT)).await?;
                let serve = async {
                    loop {
                        if let Ok((stream, _)) = listener.accept().await {
                            tokio::task::spawn_local(answer(raft.clone(), stream));
                        }
                    }
                };
                let propose = async {
                    loop {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        let change = {
                            let mut world = world.borrow_mut();
                            world.next_change += 1;
                            format!("{id}-{}", world.next_change)
                        };
                        let command = Command::AddMember { id: change.clone(), token: vec![] };
                        if write(&raft, command).await.is_ok() {
                            world.borrow_mut().committed.insert(change);
                        }
                    }
                };
                tokio::join!(serve, propose);
                Ok(())
            }
        });
    }

    let mut faults = Faults::default();
    let mut next_fault = Duration::from_secs(3);
    while sim.elapsed() < CHAOS {
        sim.step().map_err(|e| format!("seed {seed}: the simulation failed: {e}"))?;
        check_agreement(&world, seed)?;
        if sim.elapsed() >= next_fault {
            next_fault = sim.elapsed() + Duration::from_millis(rng.random_range(500..3000));
            faults.inject(&mut sim, &mut rng, &ids, &world);
        }
    }
    faults.heal(&mut sim);
    let healed = sim.elapsed();
    loop {
        sim.step().map_err(|e| format!("seed {seed}: the simulation failed: {e}"))?;
        check_agreement(&world, seed)?;
        if let Some(state) = settled(&world, nodes) {
            // nothing a leader acknowledged is lost
            let world = world.borrow();
            if let Some(lost) = world.committed.iter().find(|change| !state.members.contains_key(*change)) {
                return Err(format!("seed {seed}: the committed change {lost} is lost"));
            }
            return Ok(Outcome { seed, nodes, faults: faults.count, committed: world.committed.len() });
        }
        if sim.elapsed().saturating_sub(healed) > SETTLE {
            return Err(format!("seed {seed}: {nodes} nodes did not agree again within {SETTLE:?} of healing"));
        }
    }
}

/// The faults in force, and how many were injected.
#[derive(Default)]
struct Faults {
    crashed: BTreeSet<NodeId>,
    cut: BTreeSet<(NodeId, NodeId)>,
    count: usize,
}

impl Faults {
    /// One fault drawn from `rng`: a crash, a restart, a cut or healed link, a loss rate or a delay.
    fn inject(&mut self, sim: &mut turmoil::Sim<'_>, rng: &mut StdRng, ids: &[NodeId], world: &Rc<RefCell<World>>) {
        self.count += 1;
        let node = ids[rng.random_range(0..ids.len())];
        match rng.random_range(0..6) {
            0 if self.crashed.len() < ids.len() => {
                sim.crash(host(node));
                world.borrow_mut().running.remove(&node);
                self.crashed.insert(node);
            }
            1 => {
                if let Some(&node) = self.crashed.iter().next() {
                    sim.bounce(host(node));
                    self.crashed.remove(&node);
                }
            }
            2 => {
                let other = ids[rng.random_range(0..ids.len())];
                if other != node {
                    sim.partition(host(node), host(other));
                    self.cut.insert((node.min(other), node.max(other)));
                }
            }
            3 => {
                if let Some(&(a, b)) = self.cut.iter().next() {
                    sim.repair(host(a), host(b));
                    self.cut.remove(&(a, b));
                }
            }
            4 => sim.set_fail_rate(rng.random_range(0.0..0.1)),
            _ => sim.set_max_message_latency(Duration::from_millis(rng.random_range(1..80))),
        }
    }

    /// Everything heals: links, crashed nodes, loss and delay.
    fn heal(&mut self, sim: &mut turmoil::Sim<'_>) {
        for (a, b) in std::mem::take(&mut self.cut) {
            sim.repair(host(a), host(b));
        }
        for node in std::mem::take(&mut self.crashed) {
            sim.bounce(host(node));
        }
        sim.set_fail_rate(0.0);
        sim.set_max_message_latency(Duration::from_millis(20));
    }
}

// Two nodes that applied the log up to the same entry must hold the same state.
fn check_agreement(world: &Rc<RefCell<World>>, seed: u64) -> Result<(), String> {
    let world = world.borrow();
    let mut by_index: HashMap<u64, (NodeId, Agreed)> = HashMap::new();
    for (&id, (raft, agreed)) in &world.running {
        let Some(applied) = raft.metrics().borrow().last_applied else { continue };
        let state = agreed.borrow().clone();
        match by_index.get(&applied.index) {
            Some((other, theirs)) if *theirs != state => {
                return Err(format!(
                    "seed {seed}: nodes {other} and {id} applied the log up to {} and disagree",
                    applied.index
                ));
            }
            Some(_) => {}
            None => {
                by_index.insert(applied.index, (id, state));
            }
        }
    }
    Ok(())
}

// Once every node runs, one leads and all applied the same log: the state they agree on.
fn settled(world: &Rc<RefCell<World>>, nodes: usize) -> Option<Agreed> {
    let world = world.borrow();
    if world.running.len() < nodes {
        return None;
    }
    let metrics: Vec<_> = world.running.values().map(|(raft, _)| raft.metrics().borrow().clone()).collect();
    if !metrics.iter().any(|m| m.state == ServerState::Leader) {
        return None;
    }
    let applied: BTreeSet<_> = metrics.iter().map(|m| m.last_applied.map(|id| id.index)).collect();
    if applied.len() != 1 {
        return None;
    }
    let states: Vec<Agreed> = world.running.values().map(|(_, agreed)| agreed.borrow().clone()).collect();
    if states.windows(2).any(|pair| pair[0] != pair[1]) {
        return None;
    }
    states.into_iter().next()
}
