//! Three services on this machine form one cluster: the first creates it and admits the other two through its
//! invitations; then every device hears every other one over gossip — even the two that never met.

use std::path::{Path, PathBuf};
use std::time::Duration;

use omnarc_network::EndpointId;
use omnarc_node::pairing;
use omnarc_node::service::{Config, Event, Service};
use omnarc_protocol::v1;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("omnarc-service-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn device(name: &str) -> v1::DeviceInfo {
    v1::DeviceInfo { name: name.into(), platform: v1::Platform::Linux as i32, ..Default::default() }
}

async fn start(data: &Path, name: &str) -> Service {
    let config = Config {
        data: data.to_path_buf(),
        listen: "127.0.0.1:0".parse().unwrap(),
        device: device(name),
        telemetry_every: Duration::from_millis(200),
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
    pairing::create(&a, "Home cluster", device("Desktop")).unwrap();
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
    pairing::create(&a, "Home cluster", device("Desktop")).unwrap();
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
