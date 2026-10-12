//! The cluster's gossip topic: signed telemetry and membership news (`protocol/CLUSTER.md` §3.1).
//!
//! [`Members`] is the set of devices a node lets in on cluster protocols; [`MembersOnly`] puts that gate in front of
//! a protocol handler such as gossip's. Every message is signed by its author ([`seal`]) and checked on arrival
//! ([`open`]), so a member that relays a message cannot change it.

use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh::{EndpointId, SecretKey};
pub use iroh_gossip::api::{Event as GossipEvent, GossipReceiver, GossipSender};
pub use iroh_gossip::net::Gossip;
pub use iroh_gossip::{ALPN as GOSSIP_ALPN, TopicId};
use omnarc_protocol::v1;
use prost::Message as _;

use crate::cluster::ClusterId;
use crate::ticket::node_id;
use crate::{Error, Result};

/// The devices a node lets in on cluster protocols.
#[derive(Clone, Debug, Default)]
pub struct Members(Arc<RwLock<BTreeSet<EndpointId>>>);

impl Members {
    /// A set of members.
    pub fn new(ids: impl IntoIterator<Item = EndpointId>) -> Self {
        Self(Arc::new(RwLock::new(ids.into_iter().collect())))
    }

    /// Adds a member; whether it was new.
    ///
    /// # Panics
    /// When another thread panicked while holding the set.
    #[must_use = "whether the member was new"]
    pub fn insert(&self, id: EndpointId) -> bool {
        self.0.write().expect("members").insert(id)
    }

    /// Removes a member; whether it was there.
    ///
    /// # Panics
    /// When another thread panicked while holding the set.
    #[must_use = "whether the member was there"]
    pub fn remove(&self, id: &EndpointId) -> bool {
        self.0.write().expect("members").remove(id)
    }

    /// Whether `id` is a member.
    ///
    /// # Panics
    /// When another thread panicked while holding the set.
    #[must_use]
    pub fn contains(&self, id: &EndpointId) -> bool {
        self.0.read().expect("members").contains(id)
    }

    /// Every member, in the order of their ids.
    ///
    /// # Panics
    /// When another thread panicked while holding the set.
    #[must_use]
    pub fn list(&self) -> Vec<EndpointId> {
        self.0.read().expect("members").iter().copied().collect()
    }
}

/// A protocol handler that only members may reach: a connection from any other device is closed at once.
#[derive(Debug, Clone)]
pub struct MembersOnly<H> {
    inner: H,
    members: Members,
}

impl<H> MembersOnly<H> {
    /// Puts `members` in front of `inner`.
    pub const fn new(inner: H, members: Members) -> Self {
        Self { inner, members }
    }
}

/// Error code a node closes a connection with when the device is not a member.
pub const NOT_A_MEMBER: u32 = 403;

impl<H: ProtocolHandler> ProtocolHandler for MembersOnly<H> {
    async fn accept(&self, connection: Connection) -> std::result::Result<(), AcceptError> {
        if !self.members.contains(&connection.remote_id()) {
            connection.close(NOT_A_MEMBER.into(), b"not a member of this cluster");
            return Ok(());
        }
        self.inner.accept(connection).await
    }

    async fn shutdown(&self) {
        self.inner.shutdown().await;
    }
}

/// The gossip topic of a cluster: `BLAKE3("omnarc/v1/gossip" ‖ cluster id)`.
#[must_use]
pub fn topic(cluster: ClusterId) -> TopicId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"omnarc/v1/gossip").update(cluster.as_bytes());
    TopicId::from_bytes(*hasher.finalize().as_bytes())
}

fn signed_bytes(unix_ms: i64, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16 + 8 + payload.len());
    bytes.extend_from_slice(b"omnarc/v1/gossip");
    bytes.extend_from_slice(&unix_ms.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// A gossip message from this node: `payload` signed with its key, ready to broadcast.
#[must_use]
pub fn seal(key: &SecretKey, payload: &v1::GossipPayload, unix_ms: i64) -> Vec<u8> {
    let payload = payload.encode_to_vec();
    let signature = key.sign(&signed_bytes(unix_ms, &payload));
    v1::SignedGossip {
        sender: Some(v1::NodeId { ed25519_public_key: key.public().as_bytes().to_vec() }),
        unix_ms,
        payload,
        signature: signature.to_bytes().to_vec(),
    }
    .encode_to_vec()
}

/// A gossip message whose signature and author have been checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The member that wrote it.
    pub sender: EndpointId,
    /// When it was written, in milliseconds since the Unix epoch.
    pub unix_ms: i64,
    /// What it says.
    pub payload: v1::GossipPayload,
}

/// Opens a gossip message: it must be signed by its sender's node key, and the sender must be a member.
///
/// # Errors
/// The message is malformed, its signature is wrong, or its sender is not a member.
pub fn open(bytes: &[u8], members: &Members) -> Result<Heard> {
    let message = v1::SignedGossip::decode(bytes).map_err(|e| Error::new("gossip", e))?;
    let sender = node_id(message.sender.as_ref().ok_or_else(|| Error::new("gossip", "no sender"))?)?;
    if !members.contains(&sender) {
        return Err(Error::new("gossip", "the sender is not a member"));
    }
    let signature: [u8; 64] =
        message.signature.as_slice().try_into().map_err(|_| Error::new("gossip", "the signature is not 64 bytes"))?;
    sender
        .verify(&signed_bytes(message.unix_ms, &message.payload), &iroh::Signature::from_bytes(&signature))
        .map_err(|_| Error::new("gossip", "the signature does not match"))?;
    let payload = v1::GossipPayload::decode(message.payload.as_slice()).map_err(|e| Error::new("gossip", e))?;
    Ok(Heard { sender, unix_ms: message.unix_ms, payload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::ClusterKey;

    fn telemetry(load: f32) -> v1::GossipPayload {
        v1::GossipPayload {
            kind: Some(v1::gossip_payload::Kind::Telemetry(v1::Telemetry { cpu_load: load, ..Default::default() })),
        }
    }

    #[test]
    fn a_sealed_message_opens_for_members_only() {
        let key = SecretKey::generate();
        let bytes = seal(&key, &telemetry(0.5), 42);
        let heard = open(&bytes, &Members::new([key.public()])).unwrap();
        assert_eq!(heard.sender, key.public());
        assert_eq!(heard.unix_ms, 42);
        assert_eq!(heard.payload, telemetry(0.5));
        assert!(open(&bytes, &Members::default()).is_err(), "a stranger's message is dropped");
    }

    #[test]
    fn a_changed_message_is_refused() {
        let key = SecretKey::generate();
        let members = Members::new([key.public()]);
        let mut message = v1::SignedGossip::decode(seal(&key, &telemetry(0.5), 42).as_slice()).unwrap();
        message.payload = telemetry(0.9).encode_to_vec();
        assert!(open(&message.encode_to_vec(), &members).is_err(), "a relay changed the payload");
        let mut message = v1::SignedGossip::decode(seal(&key, &telemetry(0.5), 42).as_slice()).unwrap();
        message.unix_ms = 43;
        assert!(open(&message.encode_to_vec(), &members).is_err(), "a relay changed the time");
        let other = SecretKey::generate();
        let mut message = v1::SignedGossip::decode(seal(&key, &telemetry(0.5), 42).as_slice()).unwrap();
        message.sender = Some(v1::NodeId { ed25519_public_key: other.public().as_bytes().to_vec() });
        assert!(
            open(&message.encode_to_vec(), &Members::new([key.public(), other.public()])).is_err(),
            "a forged author"
        );
    }

    #[test]
    fn each_cluster_has_its_own_topic() {
        let (a, b) = (ClusterKey::generate().id(), ClusterKey::generate().id());
        assert_eq!(topic(a), topic(a));
        assert_ne!(topic(a), topic(b));
    }

    #[test]
    fn members_change() {
        let (a, b) = (SecretKey::generate().public(), SecretKey::generate().public());
        let members = Members::new([a]);
        assert!(members.contains(&a) && !members.contains(&b));
        assert!(members.insert(b));
        assert!(!members.insert(b));
        assert_eq!(members.list().len(), 2);
        assert!(members.remove(&a));
        assert!(!members.contains(&a));
    }
}
