//! Every Protobuf message another device can send: gossip, joining, tickets and device descriptions.
#![no_main]

use libfuzzer_sys::fuzz_target;
use omnarc_protocol::v1;
use prost::Message;

fuzz_target!(|data: &[u8]| {
    let Some((&which, bytes)) = data.split_first() else { return };
    match which % 7 {
        0 => drop(v1::SignedGossip::decode(bytes)),
        1 => drop(v1::GossipPayload::decode(bytes)),
        2 => drop(v1::JoinRequest::decode(bytes)),
        3 => drop(v1::JoinResponse::decode(bytes)),
        4 => {
            if let Ok(ticket) = v1::ClusterTicket::decode(bytes) {
                let _ = omnarc_network::ticket::Ticket::from_proto(&ticket);
            }
        }
        5 => drop(v1::DeviceInfo::decode(bytes)),
        _ => drop(v1::Telemetry::decode(bytes)),
    }
});
