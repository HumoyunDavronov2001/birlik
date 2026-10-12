//! The 32-byte header of a tensor frame, as it arrives from the previous device of a ring.
#![no_main]

use libfuzzer_sys::fuzz_target;
use omnarc_protocol::frame::FrameHeader;

fuzz_target!(|data: &[u8]| {
    if let Ok(header) = FrameHeader::decode(data) {
        // what is accepted encodes back to the same bytes
        assert_eq!(header.encode()[..], data[..32]);
    }
});
