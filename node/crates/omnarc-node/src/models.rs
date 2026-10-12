//! The models on this device (`protocol/CLUSTER.md` §4): files added where they lie, and windows of layers fetched
//! from other members of the cluster while their service runs.

use std::net::SocketAddr;
use std::path::Path;

use omnarc_network::{EndpointId, MemoryLookup};
use omnarc_store::{BLOBS_ALPN, Hash, Model, ModelStore, Window};

use crate::ring::BoxError;
use crate::service::addr_of;
use crate::state::{Member, Store};

/// The model store in the node's data directory. Only one process can hold it: the service, while it runs.
///
/// # Errors
/// The store cannot be opened, for example because the service holds it.
pub async fn open(data: &Path) -> Result<ModelStore, BoxError> {
    ModelStore::open(&data.join("models")).await.map_err(|e| {
        if e.to_string().contains("already open") {
            "the service holds the model store: stop omnarc-node serve first".into()
        } else {
            e.into()
        }
    })
}

/// The member that `who` names: its device name, or the start of its id.
///
/// # Errors
/// No member, or more than one, answers to `who`.
pub fn find<'a>(members: &'a [Member], who: &str) -> Result<&'a Member, BoxError> {
    let matches: Vec<&Member> = members
        .iter()
        .filter(|m| m.device.name.eq_ignore_ascii_case(who) || (who.len() >= 4 && m.id.to_string().starts_with(who)))
        .collect();
    match matches[..] {
        [one] => Ok(one),
        [] => Err(format!("no member is called {who}").into()),
        _ => Err(format!("{} members answer to {who}: give more of the id", matches.len()).into()),
    }
}

/// Fetches `window` of the model `hash` from the member `from`, or from the other members in turn until one has it.
/// `progress` hears which member sends and how many bytes have arrived.
///
/// # Errors
/// The device belongs to no cluster, the service holds the store, or no member could send the window.
pub async fn fetch(
    data: &Path,
    hash: Hash,
    window: Window,
    from: Option<&str>,
    name: Option<&str>,
    mut progress: impl FnMut(EndpointId, u64),
) -> Result<(Model, EndpointId), BoxError> {
    let state = Store::open(data)?;
    state.cluster()?.ok_or("this device belongs to no cluster")?;
    let store = open(data).await?;
    let key = omnarc_network::load_or_create_key(&data.join("node.key"))?;
    let me = key.public();
    let others: Vec<Member> = state.members()?.into_iter().filter(|m| m.id != me).collect();
    let candidates: Vec<&Member> = match from {
        Some(who) => vec![find(&others, who)?],
        None => others.iter().filter(|m| !m.addrs.is_empty()).collect(),
    };
    if candidates.is_empty() {
        return Err("no other member has been reached yet: run omnarc-node serve once".into());
    }
    let lookup = MemoryLookup::new();
    for member in others.iter().filter(|m| !m.addrs.is_empty()) {
        lookup.add_endpoint_info(addr_of(member.id, &member.addrs));
    }
    let any: SocketAddr = "0.0.0.0:0".parse()?;
    let endpoint = omnarc_network::bind_cluster(key, any, lookup).await?;

    let mut failures = Vec::new();
    let mut fetched = None;
    for member in candidates {
        let attempt = async {
            let connection = endpoint.connect(member.id, BLOBS_ALPN).await?;
            let model = store.fetch(&connection, name, hash, window, |n| progress(member.id, n)).await?;
            Ok::<_, BoxError>(model)
        };
        match attempt.await {
            Ok(model) => {
                fetched = Some((model, member.id));
                break;
            }
            Err(e) => failures.push(format!("{}: {e}", label(member))),
        }
    }
    endpoint.close().await;
    store.shutdown().await;
    fetched.ok_or_else(|| format!("no member sent the layers {window}:\n  {}", failures.join("\n  ")).into())
}

fn label(member: &Member) -> String {
    if member.device.name.is_empty() { member.id.fmt_short().to_string() } else { member.device.name.clone() }
}
