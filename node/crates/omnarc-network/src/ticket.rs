//! The one-time ticket that adds a device to a cluster, and the four-emoji check (`protocol/CLUSTER.md` §2).

use std::time::Duration;

use iroh::EndpointId;
use omnarc_protocol::v1;
use prost::Message as _;

use crate::cluster::ClusterId;
use crate::{Error, Result};

/// What a QR code starts with; the serialized ticket follows in base32.
pub const QR_PREFIX: &str = "OMNARC:";

/// A ticket is valid for at most this long.
pub const MAX_TTL: Duration = Duration::from_secs(10 * 60);

/// Bytes of the one-time secret.
pub const SECRET_LEN: usize = 16;

/// An invitation to join a cluster, shown as a QR code on an admin device.
#[derive(Clone, PartialEq, Eq)]
pub struct Ticket {
    /// The cluster to join.
    pub cluster: ClusterId,
    /// The device that invites; the new device dials it.
    pub inviter: EndpointId,
    /// Relays to use when no direct connection works.
    pub relay_urls: Vec<String>,
    /// Valid once and only until `expires_unix_ms`.
    pub secret: [u8; SECRET_LEN],
    /// Milliseconds since the Unix epoch.
    pub expires_unix_ms: i64,
}

impl Ticket {
    /// A new ticket with a random secret, valid for `ttl` (at most [`MAX_TTL`]) from `now_unix_ms`.
    ///
    /// # Errors
    /// The operating system has no randomness to give.
    pub fn new(
        cluster: ClusterId,
        inviter: EndpointId,
        relay_urls: Vec<String>,
        ttl: Duration,
        now_unix_ms: i64,
    ) -> Result<Self> {
        let mut secret = [0u8; SECRET_LEN];
        getrandom::fill(&mut secret).map_err(|e| Error::new("ticket secret", e))?;
        let ttl_ms = i64::try_from(ttl.min(MAX_TTL).as_millis()).unwrap_or(i64::MAX);
        Ok(Self { cluster, inviter, relay_urls, secret, expires_unix_ms: now_unix_ms.saturating_add(ttl_ms) })
    }

    /// Whether the ticket has expired at `now_unix_ms`.
    #[must_use]
    pub const fn is_expired(&self, now_unix_ms: i64) -> bool {
        now_unix_ms >= self.expires_unix_ms
    }

    /// The ticket as its protobuf message.
    #[must_use]
    pub fn to_proto(&self) -> v1::ClusterTicket {
        v1::ClusterTicket {
            cluster: Some(v1::ClusterId { ed25519_public_key: self.cluster.as_bytes().to_vec() }),
            inviter: Some(v1::NodeId { ed25519_public_key: self.inviter.as_bytes().to_vec() }),
            relay_urls: self.relay_urls.clone(),
            one_time_secret: self.secret.to_vec(),
            expires_unix_ms: self.expires_unix_ms,
        }
    }

    /// The ticket from its protobuf message.
    ///
    /// # Errors
    /// A field is missing or malformed.
    pub fn from_proto(ticket: &v1::ClusterTicket) -> Result<Self> {
        let cluster = ClusterId::from_bytes(
            &ticket.cluster.as_ref().ok_or_else(|| Error::new("ticket", "no cluster"))?.ed25519_public_key,
        )?;
        let inviter = node_id(ticket.inviter.as_ref().ok_or_else(|| Error::new("ticket", "no inviter"))?)?;
        let secret = ticket
            .one_time_secret
            .as_slice()
            .try_into()
            .map_err(|_| Error::new("ticket", "the secret is not 16 bytes"))?;
        Ok(Self {
            cluster,
            inviter,
            relay_urls: ticket.relay_urls.clone(),
            secret,
            expires_unix_ms: ticket.expires_unix_ms,
        })
    }

    /// The text of the QR code: `OMNARC:` and the serialized ticket in base32 without padding.
    #[must_use]
    pub fn encode(&self) -> String {
        format!("{QR_PREFIX}{}", data_encoding::BASE32_NOPAD.encode(&self.to_proto().encode_to_vec()))
    }

    /// A ticket from the text of a QR code, in any case, with surrounding space ignored.
    ///
    /// # Errors
    /// The text is not a ticket.
    pub fn decode(text: &str) -> Result<Self> {
        let text = text.trim().to_ascii_uppercase();
        let body =
            text.strip_prefix(QR_PREFIX).ok_or_else(|| Error::new("ticket", "it does not start with OMNARC:"))?;
        let bytes = data_encoding::BASE32_NOPAD.decode(body.as_bytes()).map_err(|e| Error::new("ticket", e))?;
        let message = v1::ClusterTicket::decode(bytes.as_slice()).map_err(|e| Error::new("ticket", e))?;
        Self::from_proto(&message)
    }
}

impl std::fmt::Debug for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ticket")
            .field("cluster", &self.cluster)
            .field("inviter", &format_args!("{}", self.inviter.fmt_short()))
            .field("relay_urls", &self.relay_urls)
            .field("expires_unix_ms", &self.expires_unix_ms)
            .finish_non_exhaustive()
    }
}

/// The joiner's proof that it holds the ticket's secret: keyed BLAKE3 of its node id, keyed with a key derived from
/// the secret.
#[must_use]
pub fn secret_proof(secret: &[u8; SECRET_LEN], joiner: EndpointId) -> [u8; 32] {
    let key = blake3::derive_key("omnarc/v1/join-proof", secret);
    *blake3::keyed_hash(&key, joiner.as_bytes()).as_bytes()
}

/// Whether `proof` is the joiner's proof for `secret`, compared in constant time.
#[must_use]
pub fn check_proof(secret: &[u8; SECRET_LEN], joiner: EndpointId, proof: &[u8]) -> bool {
    let expected = blake3::Hash::from(secret_proof(secret, joiner));
    <[u8; 32]>::try_from(proof).is_ok_and(|proof| expected == blake3::Hash::from(proof))
}

/// One of the 64 symbols of the four-emoji check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emoji {
    /// The emoji itself.
    pub symbol: &'static str,
    /// Its English name, for screen readers and for reading the symbols aloud.
    pub name: &'static str,
}

/// The four emoji both screens show while a device joins:
/// `BLAKE3("omnarc/v1/sas" ‖ inviter ‖ joiner ‖ secret)`, its first 24 bits as four 6-bit indices into [`EMOJI`].
#[must_use]
pub fn sas(inviter: EndpointId, joiner: EndpointId, secret: &[u8; SECRET_LEN]) -> [Emoji; 4] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"omnarc/v1/sas").update(inviter.as_bytes()).update(joiner.as_bytes()).update(secret);
    let hash = hasher.finalize();
    let bits = u32::from(hash.as_bytes()[0]) << 16 | u32::from(hash.as_bytes()[1]) << 8 | u32::from(hash.as_bytes()[2]);
    [18, 12, 6, 0].map(|shift| EMOJI[((bits >> shift) & 0x3f) as usize])
}

/// The table of the Matrix specification's SAS verification: easy to tell apart, translated into many languages.
pub const EMOJI: [Emoji; 64] = {
    const fn e(symbol: &'static str, name: &'static str) -> Emoji {
        Emoji { symbol, name }
    }
    [
        e("🐶", "Dog"),
        e("🐱", "Cat"),
        e("🦁", "Lion"),
        e("🐎", "Horse"),
        e("🦄", "Unicorn"),
        e("🐷", "Pig"),
        e("🐘", "Elephant"),
        e("🐰", "Rabbit"),
        e("🐼", "Panda"),
        e("🐓", "Rooster"),
        e("🐧", "Penguin"),
        e("🐢", "Turtle"),
        e("🐟", "Fish"),
        e("🐙", "Octopus"),
        e("🦋", "Butterfly"),
        e("🌷", "Flower"),
        e("🌳", "Tree"),
        e("🌵", "Cactus"),
        e("🍄", "Mushroom"),
        e("🌏", "Globe"),
        e("🌙", "Moon"),
        e("☁️", "Cloud"),
        e("🔥", "Fire"),
        e("🍌", "Banana"),
        e("🍎", "Apple"),
        e("🍓", "Strawberry"),
        e("🌽", "Corn"),
        e("🍕", "Pizza"),
        e("🎂", "Cake"),
        e("❤️", "Heart"),
        e("😀", "Smiley"),
        e("🤖", "Robot"),
        e("🎩", "Hat"),
        e("👓", "Glasses"),
        e("🔧", "Spanner"),
        e("🎅", "Santa"),
        e("👍", "Thumbs up"),
        e("☂️", "Umbrella"),
        e("⌛", "Hourglass"),
        e("⏰", "Clock"),
        e("🎁", "Gift"),
        e("💡", "Light bulb"),
        e("📕", "Book"),
        e("✏️", "Pencil"),
        e("📎", "Paperclip"),
        e("✂️", "Scissors"),
        e("🔒", "Lock"),
        e("🔑", "Key"),
        e("🔨", "Hammer"),
        e("☎️", "Telephone"),
        e("🏁", "Flag"),
        e("🚂", "Train"),
        e("🚲", "Bicycle"),
        e("✈️", "Aeroplane"),
        e("🚀", "Rocket"),
        e("🏆", "Trophy"),
        e("⚽", "Ball"),
        e("🎸", "Guitar"),
        e("🎺", "Trumpet"),
        e("🔔", "Bell"),
        e("⚓", "Anchor"),
        e("🎧", "Headphones"),
        e("📁", "Folder"),
        e("📌", "Pin"),
    ]
};

pub(crate) fn node_id(id: &v1::NodeId) -> Result<EndpointId> {
    let bytes: [u8; 32] =
        id.ed25519_public_key.as_slice().try_into().map_err(|_| Error::new("node id", "not 32 bytes"))?;
    EndpointId::from_bytes(&bytes).map_err(|e| Error::new("node id", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::ClusterKey;
    use iroh::SecretKey;

    fn ticket() -> Ticket {
        let cluster = ClusterKey::generate().id();
        let inviter = SecretKey::generate().public();
        Ticket::new(cluster, inviter, vec!["https://relay.example/".into()], Duration::from_secs(600), 1_000).unwrap()
    }

    #[test]
    fn a_ticket_survives_its_qr_text_in_any_case() {
        let ticket = ticket();
        let text = ticket.encode();
        assert!(text.starts_with("OMNARC:"));
        assert!(
            text.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b':'),
            "QR alphanumeric mode"
        );
        assert_eq!(Ticket::decode(&text).unwrap(), ticket);
        assert_eq!(Ticket::decode(&format!("  {}\n", text.to_lowercase())).unwrap(), ticket);
        assert!(Ticket::decode("OMNARC:NOTBASE32!").is_err());
        assert!(Ticket::decode(&text.replace("OMNARC:", "OTHER:")).is_err());
    }

    #[test]
    fn a_ticket_lives_ten_minutes_at_most() {
        let cluster = ClusterKey::generate().id();
        let inviter = SecretKey::generate().public();
        let ticket = Ticket::new(cluster, inviter, vec![], Duration::from_secs(3600), 0).unwrap();
        assert_eq!(ticket.expires_unix_ms, 600_000);
        assert!(!ticket.is_expired(599_999));
        assert!(ticket.is_expired(600_000));
    }

    #[test]
    fn only_the_secret_holder_proves_it() {
        let ticket = ticket();
        let joiner = SecretKey::generate().public();
        let proof = secret_proof(&ticket.secret, joiner);
        assert!(check_proof(&ticket.secret, joiner, &proof));
        assert!(
            !check_proof(&ticket.secret, SecretKey::generate().public(), &proof),
            "another device replays the proof"
        );
        assert!(!check_proof(&[0; SECRET_LEN], joiner, &proof));
        assert!(!check_proof(&ticket.secret, joiner, &proof[..31]));
    }

    #[test]
    fn both_sides_see_the_same_emoji_and_a_stranger_sees_others() {
        let ticket = ticket();
        let joiner = SecretKey::generate().public();
        let a = sas(ticket.inviter, joiner, &ticket.secret);
        let b = sas(ticket.inviter, joiner, &ticket.secret);
        assert_eq!(a, b);
        // a device in the middle shows its own key: across many tries the symbols almost never match
        let same =
            (0..200).filter(|_| sas(ticket.inviter, SecretKey::generate().public(), &ticket.secret) == a).count();
        assert!(same <= 1);
    }

    #[test]
    fn the_emoji_table_has_64_distinct_symbols() {
        let mut symbols: Vec<_> = EMOJI.iter().map(|e| e.symbol).collect();
        symbols.sort_unstable();
        symbols.dedup();
        assert_eq!(symbols.len(), 64);
        assert_eq!(EMOJI[2].name, "Lion");
        assert_eq!(EMOJI[63].name, "Pin");
    }
}
