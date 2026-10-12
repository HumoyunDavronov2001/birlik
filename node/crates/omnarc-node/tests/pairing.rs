//! Two nodes on this machine, each with its own data directory: one creates a cluster and invites, the other joins
//! with the ticket — the path `omnarc-node create`, `invite` and `join` take.

use std::path::PathBuf;
use std::time::Duration;

use omnarc_network::cluster::{self, Right};
use omnarc_node::pairing::{self, InviteOptions};
use omnarc_node::state::Store;
use omnarc_protocol::v1;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnarc-pairing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn device(name: &str) -> v1::DeviceInfo {
    v1::DeviceInfo { name: name.into(), platform: v1::Platform::Linux as i32, ..Default::default() }
}

fn options(ttl: Duration) -> InviteOptions {
    InviteOptions { listen: "127.0.0.1:0".parse().unwrap(), ttl, relays: vec![] }
}

#[tokio::test]
async fn a_device_joins_a_cluster_created_on_another() {
    let (desktop, laptop) = (data_dir("desktop"), data_dir("laptop"));
    let cluster = pairing::create(&desktop, "Home cluster", device("Desktop"), &[]).unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel();
    let inviting = pairing::invite(
        &desktop,
        options(Duration::from_secs(60)),
        move |ticket| tx.send(ticket.clone()).unwrap(),
        |attempt| async move { attempt.device.name == "Laptop" },
    );
    let joining = async {
        let ticket = rx.await.unwrap();
        let mut shown = None;
        let joined = pairing::join(&laptop, &ticket, device("Laptop"), |emoji| shown = Some(emoji)).await;
        (joined, shown)
    };
    let (member, (joined, shown)) = tokio::join!(inviting, joining);
    let member = member.unwrap();
    let joined = joined.unwrap();
    assert!(shown.is_some());
    assert_eq!(member.device.name, "Laptop");
    assert_eq!(joined.id, cluster.id);
    assert_eq!(joined.name, "Home cluster");

    // the laptop holds a node token of this cluster, and knows both members; the desktop knows the laptop
    let laptop_store = Store::open(&laptop).unwrap();
    let laptop_id = laptop_store.members().unwrap().iter().find(|m| m.device.name == "Laptop").unwrap().id;
    let membership = cluster::verify(cluster.id, laptop_id, &joined.token, |_| false).unwrap();
    assert!(membership.has(Right::Node) && !membership.has(Right::Admin));
    assert_eq!(laptop_store.members().unwrap().len(), 2);
    assert!(laptop_store.cluster_key().unwrap().is_none());
    drop(laptop_store);
    let desktop_store = Store::open(&desktop).unwrap();
    assert!(desktop_store.members().unwrap().iter().any(|m| m.id == laptop_id && m.device.name == "Laptop"));

    // a joined device cannot join again, and cannot invite without the cluster key
    let again = pairing::invite(&laptop, options(Duration::from_secs(60)), |_| {}, |_| async { true }).await;
    assert!(again.unwrap_err().to_string().contains("no cluster key"));

    drop(desktop_store);
    std::fs::remove_dir_all(&desktop).unwrap();
    std::fs::remove_dir_all(&laptop).unwrap();
}

#[tokio::test]
async fn an_invitation_ends_when_its_ticket_expires() {
    let desktop = data_dir("expiry");
    pairing::create(&desktop, "Home cluster", device("Desktop"), &[]).unwrap();
    let result = pairing::invite(&desktop, options(Duration::from_millis(300)), |_| {}, |_| async { true }).await;
    assert!(result.unwrap_err().to_string().contains("expired"));
    std::fs::remove_dir_all(&desktop).unwrap();
}
