//! Measuring the links between members (`protocol/CLUSTER.md` §3.3): the round-trip time of a one-byte ping, the
//! bandwidth of a short probe, and the kind of link a connection uses.
//!
//! Both run on short bidirectional streams of the ALPN [`PROBE_ALPN`]. A ping is the byte 1, echoed. A bandwidth probe
//! is the byte 2, a length (u64, little-endian) and that many bytes; the answer is how many microseconds passed between
//! the first and the last byte arriving, so the time to open the stream is not counted.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use iroh::endpoint::{Connection, RecvStream, SendStream};
use iroh::protocol::{AcceptError, ProtocolHandler};
use omnarc_protocol::v1::LinkKind;

use crate::{Error, Result};

/// ALPN of link probes between members.
pub const PROBE_ALPN: &[u8] = b"omnarc/probe/1";

const PING: u8 = 1;
const BANDWIDTH: u8 = 2;

/// The largest bandwidth probe a node answers.
pub const MAX_PROBE_BYTES: u64 = 64 << 20;

/// Answers the probes of other devices; put it behind [`crate::gossip::MembersOnly`].
#[derive(Debug, Clone, Default)]
pub struct ProbeHandler;

impl ProtocolHandler for ProbeHandler {
    async fn accept(&self, connection: Connection) -> std::result::Result<(), AcceptError> {
        // one stream per probe, for as long as the other device keeps the connection
        while let Ok((send, recv)) = connection.accept_bi().await {
            tokio::spawn(async move {
                let _ = answer(send, recv).await;
            });
        }
        Ok(())
    }
}

async fn answer(mut send: SendStream, mut recv: RecvStream) -> Result<()> {
    let mut kind = [0u8; 1];
    recv.read_exact(&mut kind).await.map_err(|e| Error::new("reading a probe", e))?;
    match kind[0] {
        PING => {
            send.write_all(&[PING]).await.map_err(|e| Error::new("answering a ping", e))?;
        }
        BANDWIDTH => {
            let mut len = [0u8; 8];
            recv.read_exact(&mut len).await.map_err(|e| Error::new("reading a probe", e))?;
            let len = u64::from_le_bytes(len);
            if len == 0 || len > MAX_PROBE_BYTES {
                return Err(Error(format!("a probe of {len} bytes")));
            }
            let mut buffer = vec![0u8; 64 << 10];
            let (mut got, mut first) = (0u64, None);
            while got < len {
                let n = recv
                    .read(&mut buffer)
                    .await
                    .map_err(|e| Error::new("reading a probe", e))?
                    .ok_or_else(|| Error("the probe ended early".into()))?;
                first.get_or_insert_with(Instant::now);
                got += n as u64;
            }
            let micros = first.map_or(0, |t| u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX));
            send.write_all(&micros.to_le_bytes()).await.map_err(|e| Error::new("answering a probe", e))?;
        }
        other => return Err(Error(format!("unknown probe {other}"))),
    }
    send.finish().map_err(|e| Error::new("answering a probe", e))
}

/// The round-trip time of a one-byte ping to the other end of `connection`.
///
/// # Errors
/// The connection is gone, or the other end does not answer probes.
pub async fn ping(connection: &Connection) -> Result<Duration> {
    let start = Instant::now();
    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| Error::new("opening a ping", e))?;
    send.write_all(&[PING]).await.map_err(|e| Error::new("sending a ping", e))?;
    send.finish().map_err(|e| Error::new("sending a ping", e))?;
    let mut echo = [0u8; 1];
    recv.read_exact(&mut echo).await.map_err(|e| Error::new("waiting for the ping's echo", e))?;
    Ok(start.elapsed())
}

/// A measured bandwidth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bandwidth {
    pub mbps: f64,
    /// How long the bytes took to arrive; a probe shorter than about 200 ms says little, so send more next time.
    pub took: Duration,
}

/// The bandwidth from this device to the other end of `connection`, measured with `bytes` bytes (at most
/// [`MAX_PROBE_BYTES`]).
///
/// # Errors
/// The connection is gone, or the other end does not answer probes.
pub async fn bandwidth(connection: &Connection, bytes: u64) -> Result<Bandwidth> {
    let bytes = bytes.clamp(1, MAX_PROBE_BYTES);
    let (mut send, mut recv) = connection.open_bi().await.map_err(|e| Error::new("opening a probe", e))?;
    send.write_all(&[BANDWIDTH]).await.map_err(|e| Error::new("sending a probe", e))?;
    send.write_all(&bytes.to_le_bytes()).await.map_err(|e| Error::new("sending a probe", e))?;
    let block = vec![0x5a; 64 << 10];
    let mut left = bytes;
    while left > 0 {
        let n = usize::try_from(left.min(block.len() as u64)).unwrap_or(block.len());
        send.write_all(&block[..n]).await.map_err(|e| Error::new("sending a probe", e))?;
        left -= n as u64;
    }
    send.finish().map_err(|e| Error::new("sending a probe", e))?;
    let mut micros = [0u8; 8];
    recv.read_exact(&mut micros).await.map_err(|e| Error::new("waiting for the probe's answer", e))?;
    let took = Duration::from_micros(u64::from_le_bytes(micros).max(1));
    #[allow(clippy::cast_precision_loss)] // a measurement
    let mbps = bytes as f64 * 8.0 / took.as_secs_f64() / 1e6;
    Ok(Bandwidth { mbps, took })
}

/// The kind of link `connection` uses now: a relay, or the kind of the local interface of its direct path.
#[must_use]
pub fn kind(connection: &Connection) -> LinkKind {
    let paths = connection.paths();
    let Some(path) = paths.iter().find(iroh::endpoint::Path::is_selected) else {
        return LinkKind::Unspecified;
    };
    if path.is_relay() {
        return LinkKind::Relay;
    }
    match path.local_addr() {
        iroh::endpoint::LocalTransportAddr::Ip(Some(ip)) => interface_kind(*ip),
        _ => LinkKind::Unspecified,
    }
}

/// The kind of the network interface that holds `ip`.
#[must_use]
pub fn interface_kind(ip: IpAddr) -> LinkKind {
    use netdev::interface::types::InterfaceType;
    let interfaces = netdev::get_interfaces();
    let Some(interface) = interfaces.iter().find(|i| {
        i.ipv4.iter().any(|net| IpAddr::V4(net.addr()) == ip) || i.ipv6.iter().any(|net| IpAddr::V6(net.addr()) == ip)
    }) else {
        return LinkKind::Unspecified;
    };
    let name = interface.name.to_ascii_lowercase();
    if name.contains("thunderbolt") || name.starts_with("bridge") && cfg!(target_os = "macos") {
        return LinkKind::Thunderbolt;
    }
    // a cable between two devices over USB; a USB adapter to an Ethernet cable stays Ethernet
    if name.starts_with("usb") || name.starts_with("rndis") {
        return LinkKind::Usb;
    }
    match interface.if_type {
        InterfaceType::Wireless80211 => LinkKind::Wifi,
        InterfaceType::Ethernet
        | InterfaceType::FastEthernetT
        | InterfaceType::FastEthernetFx
        | InterfaceType::GigabitEthernet => LinkKind::Ethernet,
        _ => LinkKind::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EndpointAddr, SecretKey, TransportAddr, bind_with};
    use iroh::protocol::Router;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pings_and_probes_are_answered() {
        let server = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
        let router = Router::builder(server.clone()).accept(PROBE_ALPN, ProbeHandler).spawn();
        let addr = EndpointAddr::from_parts(server.id(), server.bound_sockets().into_iter().map(TransportAddr::Ip));
        let client = bind_with(SecretKey::generate(), "127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
        let connection = client.connect(addr, PROBE_ALPN).await.unwrap();

        let rtt = ping(&connection).await.unwrap();
        assert!(rtt < Duration::from_secs(1), "{rtt:?} on this machine");
        let measured = bandwidth(&connection, 4 << 20).await.unwrap();
        assert!(measured.mbps > 10.0, "{measured:?} on this machine");
        // the loopback is no known kind of link
        assert_eq!(kind(&connection), LinkKind::Unspecified);
        // every interface of this machine that has an address is of some kind or none, without failing
        for interface in netdev::get_interfaces() {
            for net in &interface.ipv4 {
                let _ = interface_kind(IpAddr::V4(net.addr()));
            }
        }
        // too large a probe is refused, and the connection still answers pings
        assert!(bandwidth(&connection, u64::MAX).await.is_ok(), "clamped to the largest probe");
        assert!(ping(&connection).await.is_ok());

        client.close().await;
        router.shutdown().await.unwrap();
    }
}
