//! A cluster's identity: the cluster key and the membership tokens it signs (`protocol/CLUSTER.md` §1).

use std::fmt;

use biscuit_auth::macros::{authorizer, biscuit, rule};
use biscuit_auth::{Algorithm, Biscuit, KeyPair, PrivateKey, PublicKey};
use iroh::EndpointId;

use crate::{Error, Result};

/// A right that a membership token grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Right {
    /// Holding layers, gossip, model slices, Raft.
    Node,
    /// The API: asking the cluster's models.
    User,
    /// Adding and removing devices and people, settings.
    Admin,
}

impl Right {
    /// Every right, in the order tokens list them.
    pub const ALL: [Self; 3] = [Self::Node, Self::User, Self::Admin];

    /// The name of the right in a token.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::User => "user",
            Self::Admin => "admin",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|right| right.name() == name)
    }
}

/// The public identity of a cluster: the Ed25519 public key that signs its membership tokens.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClusterId([u8; 32]);

impl ClusterId {
    /// A cluster id from its 32 bytes.
    ///
    /// # Errors
    /// The bytes are not an Ed25519 public key.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let bytes: [u8; 32] = bytes.try_into().map_err(|_| Error::new("cluster id", "not 32 bytes"))?;
        PublicKey::from_bytes(&bytes, Algorithm::Ed25519).map_err(|e| Error::new("cluster id", e))?;
        Ok(Self(bytes))
    }

    /// The 32 bytes of the public key.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The public key as lowercase hex, as tokens name it.
    #[must_use]
    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }

    fn public_key(self) -> PublicKey {
        // checked when the id was made
        PublicKey::from_bytes(&self.0, Algorithm::Ed25519).expect("a cluster id holds a valid key")
    }
}

impl fmt::Display for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for ClusterId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ClusterId({})", &self.to_hex()[..10])
    }
}

/// The private key of a cluster. It stays on admin devices and signs membership tokens.
pub struct ClusterKey {
    pair: KeyPair,
    id: ClusterId,
}

impl ClusterKey {
    /// A new random cluster key.
    #[must_use]
    pub fn generate() -> Self {
        Self::from_pair(KeyPair::new())
    }

    /// A cluster key from its 32-byte Ed25519 seed.
    ///
    /// # Errors
    /// The bytes are not an Ed25519 private key.
    pub fn from_bytes(seed: &[u8; 32]) -> Result<Self> {
        let private = PrivateKey::from_bytes(seed, Algorithm::Ed25519).map_err(|e| Error::new("cluster key", e))?;
        Ok(Self::from_pair(KeyPair::from(&private)))
    }

    fn from_pair(pair: KeyPair) -> Self {
        let public = pair.public().to_bytes();
        let id = ClusterId(public.try_into().expect("an Ed25519 public key is 32 bytes"));
        Self { pair, id }
    }

    /// The 32-byte seed, to be stored where only this device can read it.
    ///
    /// # Panics
    /// Never: an Ed25519 private key is 32 bytes.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 32] {
        let bytes = self.pair.private().to_bytes();
        bytes.as_slice().try_into().expect("an Ed25519 private key is 32 bytes")
    }

    /// The cluster's public identity.
    #[must_use]
    pub const fn id(&self) -> ClusterId {
        self.id
    }

    /// A membership token for `node` with `rights`, issued at `issued_unix` (seconds).
    ///
    /// # Errors
    /// The token cannot be built or serialized.
    pub fn issue(&self, node: EndpointId, rights: &[Right], issued_unix: i64) -> Result<Vec<u8>> {
        let mut builder = biscuit!(
            r#"cluster({cluster}); node({node}); issued({issued});"#,
            cluster = self.id.to_hex(),
            node = hex(node.as_bytes()),
            issued = issued_unix,
        );
        for right in rights {
            builder = builder
                .fact(biscuit_auth::builder::fact("right", &[biscuit_auth::builder::string(right.name())]))
                .map_err(|e| Error::new("membership token", e))?;
        }
        builder.build(&self.pair).and_then(|token| token.to_vec()).map_err(|e| Error::new("membership token", e))
    }
}

impl fmt::Debug for ClusterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClusterKey").field("id", &self.id).finish_non_exhaustive()
    }
}

/// What a verified membership token grants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    /// The device the token was issued to.
    pub node: EndpointId,
    /// Its rights.
    pub rights: Vec<Right>,
    /// Identifiers that revoke the token when they are on the cluster's revocation list.
    pub revocation_ids: Vec<Vec<u8>>,
}

impl Membership {
    /// Whether the token grants `right`.
    #[must_use]
    pub fn has(&self, right: Right) -> bool {
        self.rights.contains(&right)
    }
}

/// Verifies a membership token presented by `node`, whose id the connection has authenticated.
///
/// The token must be signed by the cluster key of `cluster`, issued to `node`, and none of its revocation ids may be
/// `revoked`.
///
/// # Errors
/// The token is malformed, signed by another key, issued to another device or cluster, or revoked.
pub fn verify(
    cluster: ClusterId,
    node: EndpointId,
    token: &[u8],
    revoked: impl Fn(&[u8]) -> bool,
) -> Result<Membership> {
    let token = Biscuit::from(token, cluster.public_key()).map_err(|e| Error::new("membership token", e))?;
    let revocation_ids = token.revocation_identifiers();
    if revocation_ids.iter().any(|id| revoked(id)) {
        return Err(Error::new("membership token", "revoked"));
    }
    let mut authorizer = authorizer!(
        r#"allow if cluster({cluster}), node({node});"#,
        cluster = cluster.to_hex(),
        node = hex(node.as_bytes()),
    )
    // biscuit's default allows a millisecond, which a loaded device can overrun and so refuse a good token
    .set_limits(biscuit_auth::AuthorizerLimits { max_time: std::time::Duration::from_secs(1), ..Default::default() })
    .build(&token)
    .map_err(|e| Error::new("membership token", e))?;
    authorizer.authorize().map_err(|e| match e {
        biscuit_auth::error::Token::RunLimit(limit) => Error::new("membership token", limit),
        _ => Error::new("membership token", "issued to another device or cluster"),
    })?;
    let names: Vec<(String,)> =
        authorizer.query(rule!(r#"data($right) <- right($right)"#)).map_err(|e| Error::new("membership token", e))?;
    let rights = Right::ALL
        .into_iter()
        .filter(|right| names.iter().any(|(name,)| Right::from_name(name) == Some(*right)))
        .collect();
    Ok(Membership { node, rights, revocation_ids })
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    use fmt::Write as _;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    #[test]
    fn a_token_grants_its_rights_to_its_device_only() {
        let cluster = ClusterKey::generate();
        let device = SecretKey::generate().public();
        let other = SecretKey::generate().public();
        let token = cluster.issue(device, &[Right::Node, Right::User], 1_760_000_000).unwrap();

        let membership = verify(cluster.id(), device, &token, |_| false).unwrap();
        assert_eq!(membership.node, device);
        assert_eq!(membership.rights, vec![Right::Node, Right::User]);
        assert!(!membership.has(Right::Admin));

        assert!(verify(cluster.id(), other, &token, |_| false).is_err(), "another device presents the token");
        let stranger = ClusterKey::generate();
        assert!(verify(stranger.id(), device, &token, |_| false).is_err(), "another cluster checks the token");
    }

    #[test]
    fn a_revoked_token_is_refused() {
        let cluster = ClusterKey::generate();
        let device = SecretKey::generate().public();
        let token = cluster.issue(device, &[Right::Node], 0).unwrap();
        let ids = verify(cluster.id(), device, &token, |_| false).unwrap().revocation_ids;
        assert!(verify(cluster.id(), device, &token, |id| ids.iter().any(|r| r == id)).is_err());
    }

    #[test]
    fn a_cluster_key_survives_its_bytes() {
        let key = ClusterKey::generate();
        let again = ClusterKey::from_bytes(&key.to_bytes()).unwrap();
        assert_eq!(key.id(), again.id());
        assert_eq!(ClusterId::from_bytes(key.id().as_bytes()).unwrap(), key.id());
        assert!(ClusterId::from_bytes(&[0u8; 31]).is_err());
    }

    #[test]
    fn a_changed_token_is_refused() {
        let cluster = ClusterKey::generate();
        let device = SecretKey::generate().public();
        let mut token = cluster.issue(device, &[Right::Admin], 0).unwrap();
        let last = token.len() - 1;
        token[last] ^= 1;
        assert!(verify(cluster.id(), device, &token, |_| false).is_err());
    }
}
