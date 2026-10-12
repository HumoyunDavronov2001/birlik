//! Creating a cluster, inviting a device and joining one, as `omnarc-node create`, `invite` and `join` run them
//! (`protocol/CLUSTER.md` §2).

use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use omnarc_network::join::{self, Admitted, ClusterContext, Invitation, JOIN_ALPN, JoinAttempt};
use omnarc_network::relay::{self, Relay};
use omnarc_network::ticket::{Emoji, Ticket};
use omnarc_network::{Endpoint, EndpointId, load_or_create_key};
use omnarc_protocol::v1;

use crate::ring::BoxError;
use crate::state::{Cluster, Member, Store};

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_unix_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn with_id(mut device: v1::DeviceInfo, id: EndpointId) -> v1::DeviceInfo {
    device.id = Some(v1::NodeId { ed25519_public_key: id.as_bytes().to_vec() });
    device
}

/// Creates a cluster named `name` in the node's data directory `data`, with this device as its admin; its members
/// meet through `relays` across networks (none: only on one network).
///
/// # Errors
/// The device already belongs to a cluster, a relay address is not one, or the state cannot be written.
pub fn create(data: &Path, name: &str, device: v1::DeviceInfo, relays: &[String]) -> Result<Cluster, BoxError> {
    relay::parse_all(relays)?;
    let store = Store::open(data)?;
    let me = load_or_create_key(&data.join("node.key"))?.public();
    let cluster = store.create_cluster(name, me, with_id(device, me), now_unix_ms())?;
    store.set_relays(relays)?;
    Ok(cluster)
}

/// How long a node waits for its relay before it shows a ticket that names it.
const RELAY_PATIENCE: Duration = Duration::from_secs(10);

/// Waits until `endpoint` is reachable through one of `relays`, for at most [`RELAY_PATIENCE`].
pub(crate) async fn reach_relay(endpoint: &Endpoint, relays: &[Relay]) -> Result<(), BoxError> {
    if relays.is_empty() {
        return Ok(());
    }
    tokio::time::timeout(RELAY_PATIENCE, endpoint.online())
        .await
        .map_err(|_| "no relay of the cluster answers: devices on other networks cannot reach this one")?;
    Ok(())
}

/// How an invitation listens and how long it lasts.
#[derive(Clone, Debug)]
pub struct InviteOptions {
    /// Where the inviting node listens for the new device.
    pub listen: SocketAddr,
    /// How long the ticket is valid (at most 10 minutes).
    pub ttl: Duration,
    /// Relays for devices on other networks, besides the cluster's own.
    pub relays: Vec<String>,
}

/// Invites one device into the cluster of the node in `data`.
///
/// `ready` gets the ticket once the node listens, to show it as a QR code; `confirm` shows a device's request with
/// its four emoji to the owner and answers whether they match. Returns the new member.
///
/// # Errors
/// The node belongs to no cluster or holds no cluster key, the ticket expires or is voided, or the network fails.
pub async fn invite<C, Fut>(
    data: &Path,
    options: InviteOptions,
    ready: impl FnOnce(&Ticket),
    confirm: C,
) -> Result<Member, BoxError>
where
    C: Fn(JoinAttempt) -> Fut,
    Fut: Future<Output = bool>,
{
    let store = Store::open(data)?;
    let cluster = store.cluster()?.ok_or("this device belongs to no cluster yet: run omnarc-node create first")?;
    let key = store
        .cluster_key()?
        .ok_or("this device holds no cluster key: invite from the device that created the cluster")?;
    let secret = load_or_create_key(&data.join("node.key"))?;
    let mut relays = store.relays()?;
    for extra in options.relays {
        if !relays.contains(&extra) {
            relays.push(extra);
        }
    }
    let parsed = relay::parse_all(&relays)?;
    let endpoint = omnarc_network::bind_with_relays(secret, options.listen, vec![JOIN_ALPN.to_vec()], &parsed).await?;
    reach_relay(&endpoint, &parsed).await?;
    let addrs: Vec<SocketAddr> =
        endpoint.addr().ip_addrs().copied().filter(|addr| !addr.ip().is_unspecified()).collect();
    let ticket = Ticket::new(cluster.id, endpoint.id(), addrs, relays, options.ttl, now_unix_ms())?;
    let deadline = tokio::time::Instant::now()
        + Duration::from_millis(u64::try_from(ticket.expires_unix_ms - now_unix_ms()).unwrap_or(0));
    ready(&ticket);

    let invitation = Invitation::new(ticket);
    let members: Vec<EndpointId> = store.members()?.iter().map(|member| member.id).collect();
    let context = ClusterContext { key: &key, name: &cluster.name, members: &members };
    let result = loop {
        let incoming = match tokio::time::timeout_at(deadline, endpoint.accept()).await {
            Err(_) => break Err("the ticket expired before a device joined".into()),
            Ok(None) => break Err("the endpoint closed".into()),
            Ok(Some(incoming)) => incoming,
        };
        let Ok(connection) = incoming.await else { continue };
        match join::admit(connection, &invitation, &context, now_unix_ms, &confirm).await {
            Ok(Some(Admitted { joiner, device, token })) => {
                let member = Member { id: joiner, device, token, joined_unix_ms: now_unix_ms(), addrs: vec![] };
                store.put_member(&member)?;
                break Ok(member);
            }
            // a wrong secret: keep waiting while the ticket is still open
            Ok(None) if invitation.is_open(now_unix_ms()) => {}
            Ok(None) => break Err("the ticket is void now: run omnarc-node invite again".into()),
            Err(_) => {}
        }
    };
    endpoint.close().await;
    result
}

/// Joins the cluster of `ticket` from the node in `data`; `show_emoji` gets the four emoji to compare with the
/// inviting device's screen.
///
/// # Errors
/// The device already belongs to a cluster, the inviter refuses or cannot be reached, or the state cannot be written.
pub async fn join(
    data: &Path,
    ticket: &Ticket,
    device: v1::DeviceInfo,
    show_emoji: impl FnOnce([Emoji; 4]),
) -> Result<Cluster, BoxError> {
    let store = Store::open(data)?;
    if let Some(cluster) = store.cluster()? {
        return Err(format!("this device already belongs to {}", cluster.name).into());
    }
    if ticket.is_expired(now_unix_ms()) {
        return Err("this ticket has expired: ask for a new one".into());
    }
    let secret = load_or_create_key(&data.join("node.key"))?;
    // the inviter may be on another network: it is reached through the relays the ticket names
    let relays = relay::parse_all(&ticket.relay_urls)?;
    let endpoint = omnarc_network::bind_with_relays(secret, "0.0.0.0:0".parse()?, vec![], &relays).await?;
    reach_relay(&endpoint, &relays).await?;
    let me = endpoint.id();
    let device = with_id(device, me);
    let joined = join::join(&endpoint, ticket, device.clone(), show_emoji).await;
    endpoint.close().await;
    let cluster = store.save_joined(&joined?, me, device, now_unix_ms())?;
    // the cluster's relays become this device's too
    store.set_relays(&ticket.relay_urls)?;
    // the inviter is the one member this device can dial until gossip tells it more
    store.update_member(ticket.inviter, None, Some(ticket.direct_addrs.clone()))?;
    Ok(cluster)
}

/// The four emoji as a line: symbol and name.
#[must_use]
pub fn format_emoji(emoji: &[Emoji; 4]) -> String {
    emoji.iter().map(|e| format!("{} {}", e.symbol, e.name)).collect::<Vec<_>>().join("   ")
}
