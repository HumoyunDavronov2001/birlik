//! Three computers keep one log: the first starts it alone, the other two follow and then vote; a change proposed
//! on a follower reaches all three; when the leader goes, the other two elect a new one and go on; and the log
//! survives a restart.

use std::path::{Path, PathBuf};
use std::time::Duration;

use omnarc_network::protocol::Router;
use omnarc_network::{EndpointAddr, MemoryLookup, SecretKey, TransportAddr};
use omnarc_node::consensus::network::RAFT_ALPN;
use omnarc_node::consensus::{Agreed, Command, Consensus};

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnarc-consensus-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Node {
    consensus: Consensus,
    router: Router,
}

async fn start(dir: &Path, key: SecretKey, lookup: MemoryLookup) -> Node {
    let endpoint = omnarc_network::bind_cluster(key, "127.0.0.1:0".parse().unwrap(), lookup, None, &[]).await.unwrap();
    let (consensus, handler) = Consensus::start(dir, endpoint.clone()).await.unwrap();
    let router = Router::builder(endpoint).accept(RAFT_ALPN, handler).spawn();
    Node { consensus, router }
}

async fn eventually(what: &str, mut check: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !check() {
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting until {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn agreed(node: &Node) -> Agreed {
    node.consensus.agreed().borrow().clone()
}

fn add(id: &str) -> Command {
    Command::AddMember { id: id.into(), token: vec![1, 2, 3] }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_computers_keep_one_log() {
    let dirs = [data_dir("a"), data_dir("b"), data_dir("c")];
    let keys = [SecretKey::generate(), SecretKey::generate(), SecretKey::generate()];
    // each node finds the others by their addresses, as gossip would tell it
    let lookups = [MemoryLookup::new(), MemoryLookup::new(), MemoryLookup::new()];
    let mut nodes = Vec::new();
    for i in 0..3 {
        nodes.push(start(&dirs[i], keys[i].clone(), lookups[i].clone()).await);
    }
    for (i, lookup) in lookups.iter().enumerate() {
        for (j, node) in nodes.iter().enumerate() {
            if i != j {
                let endpoint = node.router.endpoint();
                let addrs = endpoint.bound_sockets().into_iter().map(TransportAddr::Ip);
                lookup.add_endpoint_info(EndpointAddr::from_parts(endpoint.id(), addrs));
            }
        }
    }

    // the creator starts the log alone and writes to it
    nodes[0].consensus.initialize_alone().await.unwrap();
    eventually("the creator leads", || nodes[0].consensus.is_leader()).await;
    nodes[0].consensus.propose(Command::SetName("Home cluster".into())).await.unwrap();
    nodes[0].consensus.propose(add("laptop")).await.unwrap();

    // the two others follow, then vote once they keep up
    for node in &nodes[1..] {
        nodes[0].consensus.add_learner(node.router.endpoint().id()).await.unwrap();
    }
    let voters =
        |node: &Node| node.consensus.raft().metrics().borrow().membership_config.membership().voter_ids().count();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while voters(&nodes[0]) < 3 {
        assert!(tokio::time::Instant::now() < deadline, "the followers never became voters");
        nodes[0].consensus.promote().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // a change proposed on a follower goes through the leader to everyone
    nodes[1].consensus.propose(add("phone")).await.unwrap();
    eventually("all three agree", || nodes.iter().all(|n| agreed(n).members.len() == 2)).await;
    assert_eq!(agreed(&nodes[2]).name, "Home cluster");

    // the leader goes away; the other two elect one of them and go on
    let leader = nodes.remove(0);
    leader.consensus.shutdown().await;
    leader.router.shutdown().await.unwrap();
    eventually("a new leader", || nodes.iter().any(|n| n.consensus.is_leader())).await;
    nodes[1]
        .consensus
        .propose(Command::RemoveMember { id: "phone".into(), revocation_ids: vec!["ab".into()] })
        .await
        .unwrap();
    eventually("both agree on the removal", || {
        nodes.iter().all(|n| !agreed(n).members.contains_key("phone") && agreed(n).revoked.contains("ab"))
    })
    .await;

    // the log and the state survive a restart
    let survivor = nodes.remove(0);
    let key = keys[1].clone();
    survivor.consensus.shutdown().await;
    survivor.router.shutdown().await.unwrap();
    let again = start(&dirs[1], key, MemoryLookup::new()).await;
    let state = agreed(&again);
    assert_eq!(state.name, "Home cluster");
    assert!(state.members.contains_key("laptop") && !state.members.contains_key("phone"));
    assert!(state.revoked.contains("ab"));

    again.consensus.shutdown().await;
    again.router.shutdown().await.unwrap();
    for node in nodes {
        node.consensus.shutdown().await;
        node.router.shutdown().await.unwrap();
    }
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }
}
