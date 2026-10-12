//! A GGUF header fetched from another device, and the byte ranges of the windows of its layers.
#![no_main]

use libfuzzer_sys::fuzz_target;
use omnarc_store::gguf::Header;
use omnarc_store::{Layout, Window};

fuzz_target!(|data: &[u8]| {
    let Ok(header) = Header::parse(data) else { return };
    let mut end = header.data_offset;
    for t in &header.tensors {
        assert!(t.range.start >= end && t.range.start <= t.range.end);
        end = t.range.end;
    }
    assert!(end <= header.file_len);
    if let Ok(layout) = Layout::new(&header) {
        for window in [layout.all(), Window { begin: 0, end: 1 }, Window { begin: layout.layers / 2, end: layout.layers + 1 }] {
            if let Ok(ranges) = layout.ranges(window) {
                assert!(ranges.windows(2).all(|w| w[0].end < w[1].start));
                assert!(ranges.iter().all(|r| r.end <= layout.file_len));
            }
        }
    }
});
