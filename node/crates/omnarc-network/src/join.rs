//! Joining a cluster over the ALPN `omnarc/join/1` (`protocol/CLUSTER.md` §2.2).
//!
//! The new device dials the inviter named in the ticket, proves it holds the ticket's secret and waits; both devices
//! show the same four emoji, and the inviter's owner confirms. The answer carries a membership token signed by the
//! cluster key.

use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, TransportAddr};
use omnarc_protocol::v1;
use prost::Message;

use crate::cluster::{self, ClusterId, ClusterKey, Right};
use crate::ticket::{self, Emoji, Ticket};
use crate::{Error, Result};

/// ALPN of a join connection.
pub const JOIN_ALPN: &[u8] = b"omnarc/join/1";

/// Wrong proofs a ticket survives.
pub const MAX_ATTEMPTS: u8 = 3;

/// How long the inviter's owner has to compare the emoji.
pub const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);

/// The largest message on a join stream.
const MAX_MESSAGE: usize = 64 * 1024;

/// The rights a device gets when its owner adds it.
pub const JOINED_RIGHTS: [Right; 2] = [Right::Node, Right::User];

/// A device asking to join, as the inviter's owner sees it.
#[derive(Clone, Debug)]
pub struct JoinAttempt {
    /// The device's node id, authenticated by the connection.
    pub joiner: EndpointId,
    /// What the device says it is.
    pub device: v1::DeviceInfo,
    /// The four emoji that must match the device's screen.
    pub emoji: [Emoji; 4],
}

/// A device the inviter admitted.
#[derive(Clone, Debug)]
pub struct Admitted {
    /// Its node id.
    pub joiner: EndpointId,
    /// What it said it is.
    pub device: v1::DeviceInfo,
    /// The membership token it received.
    pub token: Vec<u8>,
}

/// The cluster a device joined.
#[derive(Clone, Debug)]
pub struct Joined {
    /// The cluster's id, checked against the token's signature.
    pub cluster: ClusterId,
    /// Its name.
    pub name: String,
    /// The device's membership token.
    pub token: Vec<u8>,
    /// The members at the moment of joining.
    pub members: Vec<EndpointId>,
    /// The device that admitted it.
    pub inviter: EndpointId,
}

/// An open invitation on the inviter: one ticket, one device, void after [`MAX_ATTEMPTS`] wrong proofs.
#[derive(Debug)]
pub struct Invitation {
    ticket: Ticket,
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    attempts: u8,
    // a device waits for the owner, or the ticket is spent
    taken: bool,
}

impl Invitation {
    /// An invitation for `ticket`.
    #[must_use]
    pub fn new(ticket: Ticket) -> Self {
        Self { ticket, state: Mutex::default() }
    }

    /// The ticket of the invitation.
    #[must_use]
    pub const fn ticket(&self) -> &Ticket {
        &self.ticket
    }

    /// Whether the invitation can still admit a device at `now_unix_ms`.
    ///
    /// # Panics
    /// When another thread panicked while holding the invitation.
    #[must_use]
    pub fn is_open(&self, now_unix_ms: i64) -> bool {
        let state = self.state.lock().expect("invitation state");
        !state.taken && state.attempts < MAX_ATTEMPTS && !self.ticket.is_expired(now_unix_ms)
    }

    // Checks a request and takes the invitation for this device; the reason for a refusal otherwise.
    fn take(&self, joiner: EndpointId, proof: &[u8], now_unix_ms: i64) -> std::result::Result<(), &'static str> {
        let mut state = self.state.lock().expect("invitation state");
        if state.taken {
            return Err("this ticket is already in use");
        }
        if state.attempts >= MAX_ATTEMPTS {
            return Err("this ticket was voided after wrong attempts");
        }
        if self.ticket.is_expired(now_unix_ms) {
            return Err("this ticket has expired");
        }
        if !ticket::check_proof(&self.ticket.secret, joiner, proof) {
            state.attempts += 1;
            return Err("this is not the ticket's secret");
        }
        state.taken = true;
        Ok(())
    }
}

/// What the inviter knows about its cluster when it admits a device.
#[derive(Debug)]
pub struct ClusterContext<'a> {
    /// The cluster key, which signs the new token.
    pub key: &'a ClusterKey,
    /// The cluster's name.
    pub name: &'a str,
    /// The current members.
    pub members: &'a [EndpointId],
}

/// Handles one join connection on the inviter.
///
/// `confirm` shows the attempt to the owner and answers whether the emoji match; without an answer within
/// [`CONFIRM_TIMEOUT`] the device is refused. A refused or unanswered device voids the ticket. Returns the admitted
/// device, or `None` when the request was refused.
///
/// # Errors
/// The connection breaks or the request is malformed.
pub async fn admit<F, Fut>(
    connection: Connection,
    invitation: &Invitation,
    cluster: &ClusterContext<'_>,
    now_unix_ms: impl Fn() -> i64,
    confirm: F,
) -> Result<Option<Admitted>>
where
    F: FnOnce(JoinAttempt) -> Fut,
    Fut: Future<Output = bool>,
{
    let joiner = connection.remote_id();
    let (mut send, mut recv) = connection.accept_bi().await.map_err(|e| Error::new("accepting a join stream", e))?;
    let request: v1::JoinRequest = read_message(&mut recv).await?;
    let device = request.device.unwrap_or_default();

    if let Err(reason) = invitation.take(joiner, &request.secret_proof, now_unix_ms()) {
        answer(&connection, &mut send, refused(reason)).await?;
        return Ok(None);
    }
    let emoji = ticket::sas(invitation.ticket.inviter, joiner, &invitation.ticket.secret);
    let attempt = JoinAttempt { joiner, device: device.clone(), emoji };
    let confirmed = tokio::time::timeout(CONFIRM_TIMEOUT, confirm(attempt)).await.unwrap_or(false);
    if !confirmed {
        answer(&connection, &mut send, refused("the owner did not confirm the symbols")).await?;
        return Ok(None);
    }

    let token = cluster.key.issue(joiner, &JOINED_RIGHTS, now_unix_ms() / 1000)?;
    let mut members: Vec<v1::NodeId> = cluster.members.iter().map(|id| node_id(*id)).collect();
    if !cluster.members.contains(&joiner) {
        members.push(node_id(joiner));
    }
    let response = v1::JoinResponse {
        accepted: true,
        reason: String::new(),
        membership_token: token.clone(),
        cluster_name: cluster.name.to_owned(),
        members,
    };
    answer(&connection, &mut send, response).await?;
    Ok(Some(Admitted { joiner, device, token }))
}

/// Joins the cluster of `ticket` from `endpoint`.
///
/// `show_emoji` gets the four emoji as soon as the request is sent, to show them while the inviter's owner compares.
///
/// # Errors
/// The inviter cannot be reached, it refuses, or its answer is not signed by the ticket's cluster.
pub async fn join(
    endpoint: &Endpoint,
    ticket: &Ticket,
    device: v1::DeviceInfo,
    show_emoji: impl FnOnce([Emoji; 4]),
) -> Result<Joined> {
    let mut addrs: Vec<TransportAddr> = ticket.direct_addrs.iter().copied().map(TransportAddr::Ip).collect();
    for relay in crate::relay::parse_all(&ticket.relay_urls)? {
        addrs.push(relay.transport_addr());
    }
    let addr = EndpointAddr::from_parts(ticket.inviter, addrs);
    let connection = endpoint.connect(addr, JOIN_ALPN).await.map_err(|e| Error::new("connecting to the inviter", e))?;
    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| Error::new("opening a join stream", e))?;
    let proof = ticket::secret_proof(&ticket.secret, endpoint.id());
    write_message(&mut send, &v1::JoinRequest { device: Some(device), secret_proof: proof.to_vec() }).await?;
    show_emoji(ticket::sas(ticket.inviter, endpoint.id(), &ticket.secret));

    let response: v1::JoinResponse =
        tokio::time::timeout(CONFIRM_TIMEOUT + Duration::from_secs(30), read_message(&mut recv))
            .await
            .map_err(|_| Error::new("joining", "the inviter did not answer"))??;
    connection.close(0u32.into(), b"joined");
    if !response.accepted {
        return Err(Error::new("joining", response.reason));
    }
    cluster::verify(ticket.cluster, endpoint.id(), &response.membership_token, |_| false)?;
    let members = response.members.iter().map(ticket::node_id).collect::<Result<_>>()?;
    Ok(Joined {
        cluster: ticket.cluster,
        name: response.cluster_name,
        token: response.membership_token,
        members,
        inviter: ticket.inviter,
    })
}

fn refused(reason: &str) -> v1::JoinResponse {
    v1::JoinResponse { accepted: false, reason: reason.to_owned(), ..Default::default() }
}

fn node_id(id: EndpointId) -> v1::NodeId {
    v1::NodeId { ed25519_public_key: id.as_bytes().to_vec() }
}

// Sends the answer and waits until the joiner has it and closes the connection.
async fn answer(connection: &Connection, send: &mut SendStream, response: v1::JoinResponse) -> Result<()> {
    write_message(send, &response).await?;
    send.finish().map_err(|e| Error::new("finishing a join stream", e))?;
    let _ = tokio::time::timeout(Duration::from_secs(10), connection.closed()).await;
    Ok(())
}

async fn write_message(send: &mut SendStream, message: &impl Message) -> Result<()> {
    let body = message.encode_to_vec();
    let len = u32::try_from(body.len()).map_err(|_| Error::new("join message", "too large"))?;
    send.write_all(&len.to_le_bytes()).await.map_err(|e| Error::new("writing a join message", e))?;
    send.write_all(&body).await.map_err(|e| Error::new("writing a join message", e))
}

async fn read_message<M: Message + Default>(recv: &mut RecvStream) -> Result<M> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await.map_err(|e| Error::new("reading a join message", e))?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_MESSAGE {
        return Err(Error::new("join message", "too large"));
    }
    let mut body = vec![0u8; len];
    recv.read_exact(&mut body).await.map_err(|e| Error::new("reading a join message", e))?;
    M::decode(body.as_slice()).map_err(|e| Error::new("join message", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    const NOW: i64 = 1_760_000_000_000;

    struct Setup {
        inviter: Endpoint,
        joiner: Endpoint,
        key: ClusterKey,
    }

    async fn setup() -> Setup {
        let inviter = crate::bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![JOIN_ALPN.to_vec()])
            .await
            .unwrap();
        let joiner = crate::bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
        Setup { inviter, joiner, key: ClusterKey::generate() }
    }

    fn ticket_for(setup: &Setup, now: i64) -> Ticket {
        Ticket::new(
            setup.key.id(),
            setup.inviter.id(),
            setup.inviter.bound_sockets(),
            vec![],
            Duration::from_secs(600),
            now,
        )
        .unwrap()
    }

    fn device(name: &str) -> v1::DeviceInfo {
        v1::DeviceInfo { name: name.into(), ..Default::default() }
    }

    // Runs the inviter for one connection while the joiner joins with `ticket`.
    async fn attempt(
        setup: &Setup,
        invitation: &Invitation,
        ticket: &Ticket,
        owner_says: bool,
    ) -> (Result<Option<Admitted>>, Result<Joined>, Option<[Emoji; 4]>) {
        let members = [setup.inviter.id()];
        let cluster = ClusterContext { key: &setup.key, name: "Home cluster", members: &members };
        let seen = Mutex::new(None);
        let inviter = async {
            let connection = setup.inviter.accept().await.unwrap().await.unwrap();
            admit(
                connection,
                invitation,
                &cluster,
                || NOW,
                |attempt| {
                    *seen.lock().unwrap() = Some(attempt.emoji);
                    async move { owner_says }
                },
            )
            .await
        };
        let mut shown = None;
        let joining = join(&setup.joiner, ticket, device("Laptop"), |emoji| shown = Some(emoji));
        let (admitted, joined) = tokio::join!(inviter, joining);
        let seen = seen.into_inner().unwrap();
        if seen.is_some() {
            assert_eq!(seen, shown, "both screens show the same emoji");
        }
        (admitted, joined, shown)
    }

    #[tokio::test]
    async fn a_device_joins_once_the_owner_confirms() {
        let setup = setup().await;
        let ticket = ticket_for(&setup, NOW);
        let invitation = Invitation::new(ticket.clone());

        let (admitted, joined, _) = attempt(&setup, &invitation, &ticket, true).await;
        let admitted = admitted.unwrap().expect("admitted");
        let joined = joined.unwrap();
        assert_eq!(admitted.joiner, setup.joiner.id());
        assert_eq!(admitted.device.name, "Laptop");
        assert_eq!(joined.cluster, setup.key.id());
        assert_eq!(joined.name, "Home cluster");
        assert_eq!(joined.members, vec![setup.inviter.id(), setup.joiner.id()]);
        let membership = cluster::verify(setup.key.id(), setup.joiner.id(), &joined.token, |_| false).unwrap();
        assert!(membership.has(Right::Node) && membership.has(Right::User) && !membership.has(Right::Admin));

        // the ticket is spent: the same ticket cannot add a second device
        assert!(!invitation.is_open(NOW));
        let (admitted, joined, _) = attempt(&setup, &invitation, &ticket, true).await;
        assert!(admitted.unwrap().is_none());
        assert!(joined.unwrap_err().to_string().contains("already in use"));
    }

    #[tokio::test]
    async fn a_device_the_owner_refuses_does_not_join_and_voids_the_ticket() {
        let setup = setup().await;
        let ticket = ticket_for(&setup, NOW);
        let invitation = Invitation::new(ticket.clone());
        let (admitted, joined, shown) = attempt(&setup, &invitation, &ticket, false).await;
        assert!(admitted.unwrap().is_none());
        assert!(joined.unwrap_err().to_string().contains("did not confirm"));
        assert!(shown.is_some(), "the joiner showed its emoji");
        assert!(!invitation.is_open(NOW));
    }

    #[tokio::test]
    async fn a_wrong_secret_counts_as_an_attempt_and_three_void_the_ticket() {
        let setup = setup().await;
        let ticket = ticket_for(&setup, NOW);
        let invitation = Invitation::new(ticket.clone());
        let mut forged = ticket.clone();
        forged.secret = [7; ticket::SECRET_LEN];
        for _ in 0..MAX_ATTEMPTS {
            let (admitted, joined, _) = attempt(&setup, &invitation, &forged, true).await;
            assert!(admitted.unwrap().is_none());
            assert!(joined.unwrap_err().to_string().contains("not the ticket's secret"));
        }
        let (_, joined, _) = attempt(&setup, &invitation, &ticket, true).await;
        assert!(joined.unwrap_err().to_string().contains("voided"), "even the right secret is refused now");
    }

    #[tokio::test]
    async fn an_expired_ticket_is_refused() {
        let setup = setup().await;
        let ticket = ticket_for(&setup, NOW - 11 * 60 * 1000);
        let invitation = Invitation::new(ticket.clone());
        let (_, joined, _) = attempt(&setup, &invitation, &ticket, true).await;
        assert!(joined.unwrap_err().to_string().contains("expired"));
    }

    #[tokio::test]
    async fn a_ticket_from_another_cluster_is_caught_by_the_joiner() {
        // an inviter that signs with a different key than the ticket names
        let setup = setup().await;
        let mut ticket = ticket_for(&setup, NOW);
        ticket.cluster = ClusterKey::generate().id();
        let invitation = Invitation::new(ticket.clone());
        let (admitted, joined, _) = attempt(&setup, &invitation, &ticket, true).await;
        assert!(admitted.unwrap().is_some());
        assert!(joined.unwrap_err().to_string().contains("membership token"));
    }
}
