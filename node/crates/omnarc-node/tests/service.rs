//! Three services on this machine form one cluster: the first creates it and admits the other two through its
//! invitations; then every device hears every other one over gossip — even the two that never met.

use std::path::{Path, PathBuf};
use std::time::Duration;

use omnarc_network::EndpointId;
use omnarc_node::pairing;
use omnarc_node::service::{Config, Event, Service};
use omnarc_protocol::v1;
use omnarc_store::{BLOBS_ALPN, Window};

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnarc-service-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn device(name: &str) -> v1::DeviceInfo {
    // the phone is a phone: it keeps no Raft
    let platform = if name == "Phone" { v1::Platform::Android } else { v1::Platform::Linux };
    v1::DeviceInfo { name: name.into(), platform: platform as i32, ..Default::default() }
}

async fn start(data: &Path, name: &str) -> Service {
    let config = Config {
        data: data.to_path_buf(),
        listen: "127.0.0.1:0".parse().unwrap(),
        device: device(name),
        telemetry_every: Duration::from_millis(200),
        // the tests find each other through tickets and gossip, as across networks
        lan: false,
    };
    Service::start(config).await.unwrap()
}

// Waits until `check` holds, for at most 20 seconds.
async fn eventually(what: &str, check: impl Fn() -> bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while !check() {
        assert!(tokio::time::Instant::now() < deadline, "timed out waiting until {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// Admits the device in `data` into the cluster of `admin`, answering its join attempt with yes.
async fn admit(admin: &Service, data: &Path, name: &str) {
    let ticket = admin.invite(Duration::from_secs(60), vec![]).unwrap();
    let mut events = admin.events();
    let joining = tokio::spawn({
        let (data, name) = (data.to_path_buf(), name.to_owned());
        async move { pairing::join(&data, &ticket, device(&name), |_| {}).await.map_err(|e| e.to_string()) }
    });
    loop {
        if let Ok(Event::JoinAttempt(attempt)) = events.recv().await {
            assert_eq!(attempt.device.name, name);
            assert!(admin.answer_join(attempt.joiner, true));
            break;
        }
    }
    joining.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn devices_admitted_by_an_admin_hear_each_other() {
    let (a, b, c) = (data_dir("a"), data_dir("b"), data_dir("c"));
    pairing::create(&a, "Home cluster", device("Desktop"), &[]).unwrap();
    let desktop = start(&a, "Desktop").await;

    admit(&desktop, &b, "Laptop").await;
    // the service describes the device by its host name; the name it joined with wins
    let laptop = start(&b, "laptop-host-name").await;
    let (desktop_id, laptop_id) = (desktop.id(), laptop.id());
    eventually("the desktop hears the laptop", || desktop.telemetry().contains_key(&laptop_id)).await;
    eventually("the laptop hears the desktop", || laptop.telemetry().contains_key(&desktop_id)).await;
    eventually("the laptop knows the desktop's name", || {
        laptop.devices().get(&desktop_id).is_some_and(|d| d.name == "Desktop")
    })
    .await;
    eventually("the desktop knows the laptop by the name it joined with", || {
        desktop.devices().get(&laptop_id).is_some_and(|d| d.name == "Laptop")
    })
    .await;
    // each measures its link to the other: a ping's round trip and a probe's bandwidth
    eventually("the desktop measures its link to the laptop", || {
        desktop.telemetry().get(&desktop_id).is_some_and(|t| {
            t.links.iter().any(|link| {
                link.peer.as_ref().is_some_and(|p| p.ed25519_public_key == laptop_id.as_bytes())
                    && link.rtt_ms > 0.0
                    && link.bandwidth_mbps > 0.0
            })
        })
    })
    .await;
    let snapshot = desktop.api().snapshot();
    assert_eq!(snapshot.name, "Home cluster");
    assert!(snapshot.can_invite);
    assert!(snapshot.devices.iter().any(|d| d.name == "Laptop"));
    assert!(!laptop.api().snapshot().can_invite, "the laptop holds no cluster key");

    // the phone is admitted by the desktop alone; the laptop learns of it through gossip
    admit(&desktop, &c, "Phone").await;
    let phone = start(&c, "Phone").await;
    let phone_id = phone.id();
    eventually("the laptop lets the phone in", || laptop.members().contains(&phone_id)).await;
    eventually("the laptop hears the phone", || laptop.telemetry().contains_key(&phone_id)).await;
    eventually("the phone hears the laptop", || phone.telemetry().contains_key(&laptop_id)).await;
    assert!(laptop.devices().get(&phone_id).is_some_and(|d| d.name == "Phone"));

    // a device that is not a member is not heard
    let stranger: EndpointId = omnarc_network::SecretKey::generate().public();
    assert!(!laptop.members().contains(&stranger));

    phone.shutdown().await;
    laptop.shutdown().await;
    desktop.shutdown().await;
    for dir in [a, b, c] {
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_an_admin_opens_invitations() {
    let (a, b) = (data_dir("admin"), data_dir("member"));
    pairing::create(&a, "Home cluster", device("Desktop"), &[]).unwrap();
    let desktop = start(&a, "Desktop").await;
    admit(&desktop, &b, "Laptop").await;
    let laptop = start(&b, "Laptop").await;
    assert!(laptop.invite(Duration::from_secs(60), vec![]).unwrap_err().to_string().contains("no cluster key"));
    laptop.shutdown().await;
    desktop.shutdown().await;
    for dir in [a, b] {
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_fetches_layers_that_a_stranger_cannot() {
    let (a, b) = (data_dir("models-a"), data_dir("models-b"));
    pairing::create(&a, "Home cluster", device("Desktop"), &[]).unwrap();
    let desktop = start(&a, "Desktop").await;
    admit(&desktop, &b, "Laptop").await;
    let laptop = start(&b, "Laptop").await;
    // the laptop learns where the desktop listens from gossip
    eventually("the laptop hears the desktop", || laptop.telemetry().contains_key(&desktop.id())).await;

    let file = a.join("Tiny-6L.gguf");
    omnarc_store::testing::model(&file, 6);
    let model = desktop.models().add(&file, |_| {}).await.unwrap();
    let window: Window = "0:3".parse().unwrap();
    let fetched = laptop.fetch_model(desktop.id(), None, model.hash, window, |_| {}).await.unwrap();
    assert_eq!(fetched.layout, model.layout);
    assert_eq!(laptop.models().missing(&fetched, window).await.unwrap(), 0);
    assert!(laptop.models().missing(&fetched, "3:7".parse().unwrap()).await.unwrap() > 0);

    // a device outside the cluster is turned away before it can ask
    let stranger =
        omnarc_network::bind_with(omnarc_network::SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![])
            .await
            .unwrap();
    let store = omnarc_store::ModelStore::open(&data_dir("models-stranger").join("models")).await.unwrap();
    let addr = omnarc_network::EndpointAddr::from_parts(
        desktop.id(),
        desktop.addrs().into_iter().map(omnarc_network::TransportAddr::Ip),
    );
    let connection = stranger.connect(addr, BLOBS_ALPN).await.unwrap();
    assert!(store.fetch(&connection, None, model.hash, window, |_| {}).await.is_err(), "a stranger fetched a model");
    let reason = connection.close_reason().expect("the desktop closed the connection").to_string();
    assert!(reason.contains("not a member"), "{reason}");

    store.shutdown().await;
    stranger.close().await;
    laptop.shutdown().await;
    desktop.shutdown().await;
    for dir in [a, b, data_dir("models-stranger")] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// A relay on this machine over plain HTTP, as `iroh-relay --dev` runs one.
async fn relay() -> (iroh_relay::server::Server, String) {
    let mut config = iroh_relay::server::ServerConfig::default();
    config.relay = Some(iroh_relay::server::RelayConfig::new((std::net::Ipv4Addr::LOCALHOST, 0)));
    let server = iroh_relay::server::Server::spawn(config).await.unwrap();
    let url = format!("http://{}", server.http_addr().unwrap());
    (server, url)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_member_that_knows_no_address_comes_through_the_relay() {
    let (_relay, url) = relay().await;
    let (a, b) = (data_dir("relay-a"), data_dir("relay-b"));
    pairing::create(&a, "Home cluster", device("Desktop"), std::slice::from_ref(&url)).unwrap();
    let desktop = start(&a, "Desktop").await;
    admit(&desktop, &b, "Laptop").await;
    // the laptop took the cluster's relay from the ticket; now it forgets where the desktop listens, as a device
    // on another network never knew
    {
        let state = omnarc_node::state::Store::open(&b).unwrap();
        assert_eq!(state.relays().unwrap(), vec![url.clone()]);
        state.update_member(desktop.id(), None, Some(vec![])).unwrap();
    }
    let laptop = start(&b, "Laptop").await;
    eventually("the laptop hears the desktop through the relay", || laptop.telemetry().contains_key(&desktop.id()))
        .await;
    eventually("the desktop announces its relay", || {
        laptop.devices().get(&desktop.id()).is_some_and(|d| d.relay_url.trim_end_matches('/') == url)
    })
    .await;
    laptop.shutdown().await;
    desktop.shutdown().await;
    for dir in [a, b] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_computers_agree_and_a_removed_member_is_let_in_no_more() {
    let (a, b, c) = (data_dir("agree-a"), data_dir("agree-b"), data_dir("agree-c"));
    pairing::create(&a, "Home cluster", device("Desktop"), &[]).unwrap();
    let desktop = start(&a, "Desktop").await;
    admit(&desktop, &b, "Laptop").await;
    let laptop = start(&b, "Laptop").await;
    admit(&desktop, &c, "Phone").await;
    let phone = start(&c, "Phone").await;
    let (laptop_id, phone_id) = (laptop.id(), phone.id());

    // the desktop started the log; the laptop, a computer, follows it and agrees
    // an id in the agreed state is its key in lowercase hex, as it prints
    let hex = |id: EndpointId| id.to_string();
    eventually("the laptop agrees on all three members", || {
        laptop.agreed().is_some_and(|agreed| {
            agreed.name == "Home cluster"
                && [desktop.id(), laptop_id, phone_id].iter().all(|id| agreed.members.contains_key(&hex(*id)))
        })
    })
    .await;
    eventually("the laptop hears the phone", || laptop.telemetry().contains_key(&phone_id)).await;

    // only an admin removes, and a device cannot remove itself
    assert!(laptop.remove_member(phone_id).await.is_err(), "the laptop is no admin");
    assert!(desktop.remove_member(desktop.id()).await.is_err());

    desktop.remove_member(phone_id).await.unwrap();
    eventually("the laptop lets the phone in no more", || !laptop.members().contains(&phone_id)).await;
    eventually("the desktop lets the phone in no more", || !desktop.members().contains(&phone_id)).await;
    assert!(laptop.agreed().is_some_and(|agreed| agreed.removed.contains(&hex(phone_id))));
    // announced again, the phone stays out
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(!laptop.members().contains(&phone_id) && !desktop.members().contains(&phone_id));

    phone.shutdown().await;
    laptop.shutdown().await;
    desktop.shutdown().await;
    for dir in [a, b, c] {
        let _ = std::fs::remove_dir_all(dir);
    }
}
