//! The text of a ticket: what a QR code or a pasted command gives.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = omnarc_network::ticket::Ticket::decode(&String::from_utf8_lossy(data));
});
