//! Model files for tests: small GGUF files of the llama architecture whose bytes say which layer they belong to.

use std::path::Path;

fn string(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// A GGUF file of the llama architecture: `layers` layers of one 64 KiB f32 tensor each, every byte of a tensor
/// set to its layer number + 1, then the output head.
///
/// # Panics
/// When the file cannot be written.
pub fn model(path: &Path, layers: u32) {
    model_with(path, layers, 64 << 10);
}

/// [`model`] with tensors of `tensor_bytes` bytes, a multiple of 4.
///
/// # Panics
/// When the file cannot be written.
pub fn model_with(path: &Path, layers: u32, tensor_bytes: u64) {
    let tensors: Vec<(String, u8)> = (0..layers)
        .map(|i| (format!("blk.{i}.ffn_down.weight"), u8::try_from(i + 1).unwrap()))
        .chain([("output.weight".to_owned(), 0xEE), ("token_embd.weight".to_owned(), 0xAA)])
        .collect();
    let mut out = b"GGUF".to_vec();
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&(tensors.len() as u64).to_le_bytes());
    out.extend_from_slice(&2u64.to_le_bytes());
    string(&mut out, "general.architecture");
    out.extend_from_slice(&8u32.to_le_bytes());
    string(&mut out, "llama");
    string(&mut out, "llama.block_count");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&layers.to_le_bytes());
    for (i, (name, _)) in tensors.iter().enumerate() {
        string(&mut out, name);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(tensor_bytes / 4).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(i as u64 * tensor_bytes).to_le_bytes());
    }
    out.resize(out.len().next_multiple_of(32), 0);
    for (_, fill) in &tensors {
        out.resize(out.len() + usize::try_from(tensor_bytes).unwrap(), *fill);
    }
    std::fs::write(path, out).unwrap();
}
