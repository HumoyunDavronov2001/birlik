//! Relays (`protocol/CLUSTER.md` §3.5): servers through which members on different networks reach each other.
//!
//! A relay helps two devices behind NATs punch a direct path between them and carries their encrypted traffic when
//! punching fails. A cluster names its relays; a relay that only admits its owner's clusters wants a token, written
//! into the relay's address as `https://relay.example.com/?token=SECRET`. Tickets carry the address with its token, so
//! a device on another network can reach the inviter; the token never travels to the relay in the address, only in
//! the `Authorization` header of the relay connection.

use std::fmt;
use std::str::FromStr;

use iroh::{RelayConfig, RelayMap, RelayMode, RelayUrl, TransportAddr};

use crate::{Error, Result};

/// A relay a cluster uses: its address and the token it wants, if any.
#[derive(Clone, PartialEq, Eq)]
pub struct Relay {
    pub url: RelayUrl,
    pub token: Option<String>,
}

impl fmt::Debug for Relay {
    // the token is a secret
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Relay")
            .field("url", &self.url.to_string())
            .field("token", &self.token.as_ref().map(|_| "…"))
            .finish()
    }
}

impl FromStr for Relay {
    type Err = Error;

    /// `https://relay.example.com`, or with its token: `https://relay.example.com/?token=SECRET`.
    fn from_str(s: &str) -> Result<Self> {
        let (address, token) = match s.split_once("?token=") {
            Some((address, token)) if !token.is_empty() && !token.contains('&') => (address, Some(token.to_owned())),
            Some(_) => return Err(Error(format!("{s}: a relay takes one token, ?token=SECRET"))),
            None => (s, None),
        };
        let url: RelayUrl = address.parse().map_err(|e| Error::new(&format!("the relay {address}"), e))?;
        if !matches!(url.scheme(), "https" | "http") {
            return Err(Error(format!("{address}: a relay address starts with https://")));
        }
        Ok(Self { url, token })
    }
}

impl fmt::Display for Relay {
    /// The relay's address with its token, as tickets carry it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.token {
            Some(token) => write!(f, "{}?token={token}", self.url),
            None => write!(f, "{}", self.url),
        }
    }
}

impl Relay {
    /// The address at which a device homed on this relay is dialed.
    #[must_use]
    pub fn transport_addr(&self) -> TransportAddr {
        TransportAddr::Relay(self.url.clone())
    }
}

/// Reads relay addresses, skipping none: one that does not parse is an error.
///
/// # Errors
/// An address is not a relay address.
pub fn parse_all<S: AsRef<str>>(addresses: &[S]) -> Result<Vec<Relay>> {
    addresses.iter().map(|a| a.as_ref().parse()).collect()
}

/// How an endpoint uses `relays`: not at all when there are none.
#[must_use]
pub fn relay_mode(relays: &[Relay]) -> RelayMode {
    if relays.is_empty() {
        return RelayMode::Disabled;
    }
    let configs = relays.iter().map(|relay| {
        let config = RelayConfig::from(relay.url.clone());
        match &relay.token {
            Some(token) => config.with_auth_token(token.clone()),
            None => config,
        }
    });
    RelayMode::Custom(RelayMap::from_iter(configs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relay_address_keeps_its_token_apart() {
        let relay: Relay = "https://relay.example.com/?token=s3cret".parse().unwrap();
        assert_eq!(relay.url.to_string(), "https://relay.example.com/");
        assert_eq!(relay.token.as_deref(), Some("s3cret"));
        assert_eq!(relay.to_string(), "https://relay.example.com/?token=s3cret");
        assert_eq!(relay.to_string().parse::<Relay>().unwrap(), relay);
        assert!(!format!("{relay:?}").contains("s3cret"), "the token stays out of logs");

        let open: Relay = "https://relay2.example.com".parse().unwrap();
        assert_eq!(open.token, None);
        for bad in [
            "relay.example.com",
            "ftp://relay.example.com",
            "https://r.example/?token=",
            "https://r.example/?token=a&b=c",
        ] {
            assert!(bad.parse::<Relay>().is_err(), "{bad}");
        }
        assert!(matches!(relay_mode(&[]), RelayMode::Disabled));
        assert!(matches!(relay_mode(&[relay, open]), RelayMode::Custom(map) if map.len() == 2));
    }
}
