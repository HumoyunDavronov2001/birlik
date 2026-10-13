//! The model store of an Omnarc node (PLAN.md §3.8, `protocol/CLUSTER.md`).
//!
//! A model is downloaded from the internet once, by one device; the cluster spreads it peer to peer. [`gguf`] reads
//! the header of a model file, [`layers`] turns a window of layers into the byte ranges it needs, and [`store`]
//! keeps models in a BLAKE3-verified store (iroh-blobs) from which other devices fetch only their own layers.

pub mod gguf;
pub mod layers;
pub mod sparse;
pub mod store;
#[cfg(feature = "testing")]
pub mod testing;

pub use layers::{Layout, Window};
pub use store::{BLOBS_ALPN, Error, Hash, Model, ModelStore, Result};
