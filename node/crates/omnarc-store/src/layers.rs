//! Which bytes of a model file a window of layers needs.
//!
//! The engine loads a window of layers `[begin, end)` (`engine/patches/0001-llama-add-layer-windows.patch`): the
//! repeating layers are numbered `0 .. n_layer - 1` and the output layer is `n_layer`. It never reads the tensors
//! outside its window, so a device needs only the header, its own tensors and the few that every layer shares — the
//! rest of the file can stay a hole.

use std::fmt;
use std::ops::Range;

use crate::gguf::{Header, Tensor};

/// The part of the model a tensor belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The token embeddings and what goes with them: loaded by the window that starts at layer 0.
    Input,
    /// A repeating layer.
    Layer(u32),
    /// The output norm and head: the window that ends after the last repeating layer.
    Output,
    /// The token embeddings of a model without its own output head: both the input and the output use them.
    Embeddings,
    /// Anything else, such as `rope_freqs.weight`: every window loads it.
    Shared,
}

/// A window of layers `[begin, end)`, the output layer being `layers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    pub begin: u32,
    pub end: u32,
}

impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.begin, self.end)
    }
}

impl std::str::FromStr for Window {
    type Err = String;

    /// `10:20`, as `omnarc-node ring --layers`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (begin, end) = s.split_once(':').ok_or_else(|| format!("{s} is not a window such as 0:40"))?;
        let begin = begin.trim().parse().map_err(|_| format!("{begin} is not a layer"))?;
        let end = end.trim().parse().map_err(|_| format!("{end} is not a layer"))?;
        Ok(Self { begin, end })
    }
}

/// A model file as the parts of its layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The repeating layers; the output layer is number `layers`.
    pub layers: u32,
    /// The header: metadata and tensor descriptions, padded to the alignment.
    pub header: Range<u64>,
    pub file_len: u64,
    /// Tensors start at multiples of this; the padding between them goes with them.
    pub alignment: u64,
    /// Every tensor's part and bytes, in file order.
    pub tensors: Vec<(Part, Range<u64>)>,
}

/// The part a tensor belongs to, by its name; `tied` when the model has no `output.weight`.
fn part(tensor: &Tensor, layers: u32, tied: bool) -> Part {
    let name = tensor.name.as_str();
    if let Some(rest) = name.strip_prefix("blk.") {
        return match rest.split_once('.').and_then(|(n, _)| n.parse::<u32>().ok()) {
            Some(n) if n < layers => Part::Layer(n),
            // blocks after the repeating ones (such as a prediction head) go with the output, as in the engine
            Some(_) => Part::Output,
            None => Part::Shared,
        };
    }
    let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
    match stem {
        "token_embd" if tied => Part::Embeddings,
        "token_embd" | "token_embd_norm" | "token_types" | "pos_embd" => Part::Input,
        "output" | "output_norm" => Part::Output,
        _ => Part::Shared,
    }
}

impl Layout {
    /// The layout of a model whose header says how many layers it has.
    ///
    /// # Errors
    /// The header does not give `general.architecture` and `<architecture>.block_count`.
    pub fn new(header: &Header) -> Result<Self, String> {
        let layers = header.block_count().ok_or("the model does not say how many layers it has")?;
        let tied = header.tensor("output.weight").is_none();
        let tensors = header.tensors.iter().map(|t| (part(t, layers, tied), t.range.clone())).collect();
        Ok(Self {
            layers,
            header: 0..header.data_offset,
            file_len: header.file_len,
            alignment: header.alignment,
            tensors,
        })
    }

    /// The whole model: every repeating layer and the output layer.
    #[must_use]
    pub fn all(&self) -> Window {
        Window { begin: 0, end: self.layers + 1 }
    }

    /// Checks that `window` is a window of this model.
    ///
    /// # Errors
    /// The window is empty or reaches past the output layer.
    pub fn check(&self, window: Window) -> Result<(), String> {
        if window.begin >= window.end || window.end > self.layers + 1 {
            return Err(format!(
                "{window} is not a window of a model with {} layers and the output layer ({})",
                self.layers,
                self.all()
            ));
        }
        Ok(())
    }

    /// The byte ranges `window` needs, sorted and merged across padding: the header, its layers and the shared
    /// tensors.
    ///
    /// # Errors
    /// The window is not a window of this model.
    pub fn ranges(&self, window: Window) -> Result<Vec<Range<u64>>, String> {
        self.check(window)?;
        let input = window.begin == 0;
        let output = window.end > self.layers;
        let mut ranges = vec![self.header.clone()];
        for (part, range) in &self.tensors {
            let needed = match *part {
                Part::Input => input,
                Part::Layer(n) => n >= window.begin && n < window.end,
                Part::Output => output,
                Part::Embeddings => input || output,
                Part::Shared => true,
            };
            if needed && range.start < range.end {
                ranges.push(range.clone());
            }
        }
        ranges.sort_by_key(|r| r.start);
        let mut merged: Vec<Range<u64>> = Vec::with_capacity(ranges.len());
        for range in ranges {
            match merged.last_mut() {
                Some(last) if range.start <= last.end.saturating_add(self.alignment) => {
                    last.end = last.end.max(range.end);
                }
                _ => merged.push(range),
            }
        }
        Ok(merged)
    }

    /// How many bytes `window` needs.
    ///
    /// # Errors
    /// The window is not a window of this model.
    pub fn bytes(&self, window: Window) -> Result<u64, String> {
        Ok(self.ranges(window)?.iter().map(|r| r.end - r.start).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gguf::tests::llama;

    fn layout(tied: bool) -> (Header, Layout) {
        let header = Header::parse(&llama(4, tied)).unwrap();
        let layout = Layout::new(&header).unwrap();
        (header, layout)
    }

    fn covers(ranges: &[Range<u64>], tensor: &Tensor) -> bool {
        ranges.iter().any(|r| r.start <= tensor.range.start && tensor.range.end <= r.end)
    }

    #[test]
    fn a_window_needs_its_layers_and_what_is_shared() {
        let (header, layout) = layout(false);
        let middle = layout.ranges(Window { begin: 1, end: 3 }).unwrap();
        assert_eq!(middle[0].start, 0, "the header comes first");
        for t in &header.tensors {
            let wanted = t.name.starts_with("blk.1.") || t.name.starts_with("blk.2.") || t.name == "rope_freqs.weight";
            assert_eq!(covers(&middle, t), wanted, "{}", t.name);
        }

        let first = layout.ranges(Window { begin: 0, end: 2 }).unwrap();
        assert!(covers(&first, header.tensor("token_embd.weight").unwrap()));
        assert!(!covers(&first, header.tensor("output.weight").unwrap()));

        let last = layout.ranges(Window { begin: 3, end: 5 }).unwrap();
        assert!(covers(&last, header.tensor("output.weight").unwrap()));
        assert!(covers(&last, header.tensor("output_norm.weight").unwrap()));
        assert!(!covers(&last, header.tensor("token_embd.weight").unwrap()));

        // the whole model is the whole file, less the padding after the last tensor
        let all = layout.ranges(layout.all()).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].end <= layout.file_len && layout.file_len - all[0].end < header.alignment);
    }

    #[test]
    fn tied_embeddings_go_with_the_output_too() {
        let (header, layout) = layout(true);
        let last = layout.ranges(Window { begin: 4, end: 5 }).unwrap();
        assert!(covers(&last, header.tensor("token_embd.weight").unwrap()));
        let middle = layout.ranges(Window { begin: 2, end: 3 }).unwrap();
        assert!(!covers(&middle, header.tensor("token_embd.weight").unwrap()));
    }

    #[test]
    fn windows_split_the_model() {
        let (_, layout) = layout(false);
        let two: u64 =
            [Window { begin: 0, end: 2 }, Window { begin: 2, end: 5 }].iter().map(|&w| layout.bytes(w).unwrap()).sum();
        let whole = layout.bytes(layout.all()).unwrap();
        // the header and the shared tensors are needed twice, the layers once
        assert!(two > whole && two < whole + whole / 2);
    }

    #[test]
    fn refuses_windows_outside_the_model() {
        let (_, layout) = layout(false);
        for w in ["0:0", "3:2", "0:6", "5:6"] {
            assert!(layout.ranges(w.parse().unwrap()).is_err(), "{w}");
        }
        assert!("0-4".parse::<Window>().is_err());
        assert_eq!("2:5".parse::<Window>().unwrap(), Window { begin: 2, end: 5 });
        assert_eq!(Window { begin: 2, end: 5 }.to_string(), "2:5");
    }
}
