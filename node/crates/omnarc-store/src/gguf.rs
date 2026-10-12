//! A reader of GGUF headers (format versions 2 and 3): the metadata, the tensors, and where each tensor's bytes lie
//! in the file.
//!
//! It reads untrusted bytes — a header fetched from another device — so it never panics, allocates only in step
//! with the bytes it was given, checks every size for overflow and refuses what llama.cpp's `gguf.cpp` refuses. When
//! the bytes end before the header does, it says how many it needs, so a caller can fetch the header in pieces.

use std::collections::HashSet;
use std::fmt;
use std::ops::Range;

/// The first four bytes of every GGUF file.
pub const MAGIC: [u8; 4] = *b"GGUF";

/// The alignment of tensor data when `general.alignment` is not given.
pub const DEFAULT_ALIGNMENT: u64 = 32;

/// Headers longer than this are refused; the largest vocabularies take a few tens of MiB.
pub const MAX_HEADER: u64 = 256 << 20;

/// llama.cpp's `GGML_MAX_DIMS`.
const MAX_DIMS: u32 = 4;

/// llama.cpp's `GGML_MAX_NAME` less the terminating zero.
const MAX_NAME: usize = 63;

// the value types of metadata
const TYPE_U8: u32 = 0;
const TYPE_I8: u32 = 1;
const TYPE_U16: u32 = 2;
const TYPE_I16: u32 = 3;
const TYPE_U32: u32 = 4;
const TYPE_I32: u32 = 5;
const TYPE_F32: u32 = 6;
const TYPE_BOOL: u32 = 7;
const TYPE_STRING: u32 = 8;
const TYPE_ARRAY: u32 = 9;
const TYPE_U64: u32 = 10;
const TYPE_I64: u32 = 11;
const TYPE_F64: u32 = 12;

/// A metadata value. Arrays — the vocabulary, mostly — are skipped and only described.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    String(String),
    U64(u64),
    I64(i64),
    F64(f64),
    /// An array of `len` values of the type `kind` (a GGUF value type).
    Array {
        kind: u32,
        len: u64,
    },
}

impl Value {
    /// The value as an unsigned integer, if it is an integer that is not negative.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Self::U8(v) => Some(v.into()),
            Self::U16(v) => Some(v.into()),
            Self::U32(v) => Some(v.into()),
            Self::U64(v) => Some(v),
            Self::I8(v) => u64::try_from(v).ok(),
            Self::I16(v) => u64::try_from(v).ok(),
            Self::I32(v) => u64::try_from(v).ok(),
            Self::I64(v) => u64::try_from(v).ok(),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
}

/// The element type of a tensor (`ggml_type`), with the size of its blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TensorType {
    pub id: u32,
    pub name: &'static str,
    /// Elements in one block.
    pub block: u64,
    /// Bytes in one block.
    pub bytes: u64,
}

/// The tensor types llama.cpp b11565 reads, from `ggml_type_size` and `ggml_blck_size`. Removed types are absent.
const TENSOR_TYPES: [TensorType; 35] = [
    TensorType { id: 0, name: "f32", block: 1, bytes: 4 },
    TensorType { id: 1, name: "f16", block: 1, bytes: 2 },
    TensorType { id: 2, name: "q4_0", block: 32, bytes: 18 },
    TensorType { id: 3, name: "q4_1", block: 32, bytes: 20 },
    TensorType { id: 6, name: "q5_0", block: 32, bytes: 22 },
    TensorType { id: 7, name: "q5_1", block: 32, bytes: 24 },
    TensorType { id: 8, name: "q8_0", block: 32, bytes: 34 },
    TensorType { id: 9, name: "q8_1", block: 32, bytes: 36 },
    TensorType { id: 10, name: "q2_K", block: 256, bytes: 84 },
    TensorType { id: 11, name: "q3_K", block: 256, bytes: 110 },
    TensorType { id: 12, name: "q4_K", block: 256, bytes: 144 },
    TensorType { id: 13, name: "q5_K", block: 256, bytes: 176 },
    TensorType { id: 14, name: "q6_K", block: 256, bytes: 210 },
    TensorType { id: 15, name: "q8_K", block: 256, bytes: 292 },
    TensorType { id: 16, name: "iq2_xxs", block: 256, bytes: 66 },
    TensorType { id: 17, name: "iq2_xs", block: 256, bytes: 74 },
    TensorType { id: 18, name: "iq3_xxs", block: 256, bytes: 98 },
    TensorType { id: 19, name: "iq1_s", block: 256, bytes: 50 },
    TensorType { id: 20, name: "iq4_nl", block: 32, bytes: 18 },
    TensorType { id: 21, name: "iq3_s", block: 256, bytes: 110 },
    TensorType { id: 22, name: "iq2_s", block: 256, bytes: 82 },
    TensorType { id: 23, name: "iq4_xs", block: 256, bytes: 136 },
    TensorType { id: 24, name: "i8", block: 1, bytes: 1 },
    TensorType { id: 25, name: "i16", block: 1, bytes: 2 },
    TensorType { id: 26, name: "i32", block: 1, bytes: 4 },
    TensorType { id: 27, name: "i64", block: 1, bytes: 8 },
    TensorType { id: 28, name: "f64", block: 1, bytes: 8 },
    TensorType { id: 29, name: "iq1_m", block: 256, bytes: 56 },
    TensorType { id: 30, name: "bf16", block: 1, bytes: 2 },
    TensorType { id: 34, name: "tq1_0", block: 256, bytes: 54 },
    TensorType { id: 35, name: "tq2_0", block: 256, bytes: 66 },
    TensorType { id: 39, name: "mxfp4", block: 32, bytes: 17 },
    TensorType { id: 40, name: "nvfp4", block: 64, bytes: 36 },
    TensorType { id: 41, name: "q1_0", block: 128, bytes: 18 },
    TensorType { id: 42, name: "q2_0", block: 64, bytes: 18 },
];

impl TensorType {
    #[must_use]
    pub fn from_id(id: u32) -> Option<Self> {
        TENSOR_TYPES.iter().copied().find(|t| t.id == id)
    }
}

/// A tensor of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tensor {
    pub name: String,
    /// Elements along each dimension, the first one varying fastest.
    pub dims: Vec<u64>,
    pub kind: TensorType,
    /// Where the tensor's bytes lie, counted from the start of the file.
    pub range: Range<u64>,
}

/// The header of a GGUF file.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u32,
    pub alignment: u64,
    /// The metadata, in the order of the file.
    pub metadata: Vec<(String, Value)>,
    /// The tensors, in the order of their data.
    pub tensors: Vec<Tensor>,
    /// Where the tensor data begins: the header's own length, padded to the alignment.
    pub data_offset: u64,
    /// The length of the whole file: the header, then every tensor padded to the alignment.
    pub file_len: u64,
}

/// Why a header could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The bytes end before the header does; at least `needed` bytes from the start of the file are needed.
    Incomplete { needed: u64 },
    /// The bytes are not a GGUF header that llama.cpp would read.
    Invalid(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete { needed } => write!(f, "the GGUF header needs at least {needed} bytes"),
            Self::Invalid(why) => write!(f, "not a valid GGUF file: {why}"),
        }
    }
}

impl std::error::Error for Error {}

fn invalid<T>(why: impl Into<String>) -> Result<T, Error> {
    Err(Error::Invalid(why.into()))
}

/// The bytes of a header, read from the front.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: u64) -> Result<&'a [u8], Error> {
        let end = (self.at as u64).checked_add(n).ok_or_else(|| Error::Invalid("a length overflows".into()))?;
        if end > MAX_HEADER {
            return invalid(format!("the header is longer than {MAX_HEADER} bytes"));
        }
        // end ≤ MAX_HEADER, so it fits in usize
        let end_index = usize::try_from(end).map_err(|_| Error::Invalid("a length overflows".into()))?;
        let Some(bytes) = self.bytes.get(self.at..end_index) else {
            return Err(Error::Incomplete { needed: end });
        };
        self.at = end_index;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let bytes = self.take(N as u64)?;
        let mut out = [0; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    /// A count or a length: a signed 64-bit number in llama.cpp, so it must not be negative.
    fn count(&mut self, what: &str) -> Result<u64, Error> {
        let n = self.u64()?;
        if i64::try_from(n).is_err() {
            return invalid(format!("{what} is negative"));
        }
        Ok(n)
    }

    fn bytes_of_string(&mut self) -> Result<&'a [u8], Error> {
        let len = self.count("a string length")?;
        self.take(len)
    }

    fn string(&mut self, what: &str) -> Result<String, Error> {
        let bytes = self.bytes_of_string()?;
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok(s.to_owned()),
            Err(_) => invalid(format!("{what} is not UTF-8")),
        }
    }

    fn value(&mut self, kind: u32) -> Result<Value, Error> {
        Ok(match kind {
            TYPE_U8 => Value::U8(self.u8()?),
            TYPE_I8 => Value::I8(i8::from_le_bytes(self.array()?)),
            TYPE_U16 => Value::U16(self.u16()?),
            TYPE_I16 => Value::I16(i16::from_le_bytes(self.array()?)),
            TYPE_U32 => Value::U32(self.u32()?),
            TYPE_I32 => Value::I32(i32::from_le_bytes(self.array()?)),
            TYPE_F32 => Value::F32(f32::from_le_bytes(self.array()?)),
            TYPE_BOOL => Value::Bool(self.u8()? != 0),
            // a value such as a chat template is kept even with a stray byte that is not UTF-8
            TYPE_STRING => Value::String(String::from_utf8_lossy(self.bytes_of_string()?).into_owned()),
            TYPE_U64 => Value::U64(self.u64()?),
            TYPE_I64 => Value::I64(i64::from_le_bytes(self.array()?)),
            TYPE_F64 => Value::F64(f64::from_le_bytes(self.array()?)),
            _ => return invalid(format!("unknown value type {kind}")),
        })
    }

    /// One tensor's name, dimensions, type, offset in the data and size in bytes.
    fn tensor_info(&mut self, names: &mut HashSet<String>) -> Result<TensorInfo, Error> {
        let name = self.string("a tensor name")?;
        if name.len() > MAX_NAME {
            return invalid(format!("the tensor name {name} is longer than {MAX_NAME} bytes"));
        }
        if !names.insert(name.clone()) {
            return invalid(format!("the tensor {name} appears twice"));
        }
        let n_dims = self.u32()?;
        if n_dims > MAX_DIMS {
            return invalid(format!("the tensor {name} has {n_dims} dimensions (at most {MAX_DIMS})"));
        }
        let mut dims = Vec::with_capacity(MAX_DIMS as usize);
        for _ in 0..n_dims {
            dims.push(self.count("a tensor dimension")?);
        }
        let id = self.u32()?;
        let Some(kind) = TensorType::from_id(id) else {
            return invalid(format!("the tensor {name} has the unknown type {id}"));
        };
        let row = dims.first().copied().unwrap_or(1);
        if row % kind.block != 0 {
            return invalid(format!("the rows of {name} are not whole blocks of {}", kind.name));
        }
        // the element count must stay below 2^63 (llama.cpp's int64_t), and so must the byte count
        let mut elements: u64 = 1;
        for &d in &dims {
            elements = elements
                .checked_mul(d)
                .filter(|&n| i64::try_from(n).is_ok())
                .ok_or_else(|| Error::Invalid(format!("the shape of {name} overflows")))?;
        }
        let size = (elements / kind.block)
            .checked_mul(kind.bytes)
            .filter(|&n| i64::try_from(n).is_ok())
            .ok_or_else(|| Error::Invalid(format!("the size of {name} overflows")))?;
        let offset = self.u64()?;
        Ok((name, dims, kind, offset, size))
    }

    /// Steps over an array of `len` values of the type `kind` without keeping them.
    fn skip_array(&mut self, kind: u32, len: u64) -> Result<(), Error> {
        let size = match kind {
            TYPE_U8 | TYPE_I8 | TYPE_BOOL => 1,
            TYPE_U16 | TYPE_I16 => 2,
            TYPE_U32 | TYPE_I32 | TYPE_F32 => 4,
            TYPE_U64 | TYPE_I64 | TYPE_F64 => 8,
            TYPE_STRING => {
                for _ in 0..len {
                    self.bytes_of_string()?;
                }
                return Ok(());
            }
            TYPE_ARRAY => return invalid("arrays of arrays are not supported"),
            _ => return invalid(format!("unknown array type {kind}")),
        };
        let bytes = len.checked_mul(size).ok_or_else(|| Error::Invalid("an array length overflows".into()))?;
        self.take(bytes)?;
        Ok(())
    }
}

/// A tensor as the header describes it: name, dimensions, type, offset in the data and size in bytes.
type TensorInfo = (String, Vec<u64>, TensorType, u64, u64);

/// `n` rounded up to a multiple of `alignment`, a power of two.
fn pad(n: u64, alignment: u64) -> Option<u64> {
    n.checked_add(alignment - 1).map(|n| n & !(alignment - 1))
}

impl Header {
    /// Reads the header at the start of `bytes`, which may hold more of the file or all of it.
    ///
    /// # Errors
    /// [`Error::Incomplete`] when `bytes` ends inside the header, [`Error::Invalid`] when it is not a GGUF header.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let mut r = Reader { bytes, at: 0 };
        if r.array::<4>()? != MAGIC {
            return invalid("the file does not start with GGUF");
        }
        let version = r.u32()?;
        if version != 0 && version.trailing_zeros() >= 16 {
            return invalid("big-endian files are not supported");
        }
        if !(2..=3).contains(&version) {
            return invalid(format!("version {version} is not supported (2 and 3 are)"));
        }
        let n_tensors = r.count("the tensor count")?;
        let n_kv = r.count("the metadata count")?;

        let mut metadata = Vec::new();
        let mut keys = HashSet::new();
        for _ in 0..n_kv {
            let key = r.string("a metadata key")?;
            if !keys.insert(key.clone()) {
                return invalid(format!("the metadata key {key} appears twice"));
            }
            let kind = r.u32()?;
            let value = if kind == TYPE_ARRAY {
                let element = r.u32()?;
                let len = r.count("an array length")?;
                r.skip_array(element, len)?;
                Value::Array { kind: element, len }
            } else {
                r.value(kind)?
            };
            metadata.push((key, value));
        }

        let alignment = match metadata.iter().find(|(k, _)| k == "general.alignment") {
            None => DEFAULT_ALIGNMENT,
            Some((_, Value::U32(a))) if a.is_power_of_two() => u64::from(*a),
            Some((_, Value::U32(a))) => return invalid(format!("the alignment {a} is not a power of two")),
            Some(_) => return invalid("general.alignment is not a u32"),
        };

        // name, dimensions, type and offset of every tensor; the data offsets come once the header's length is known
        let mut infos = Vec::new();
        let mut names = HashSet::new();
        for _ in 0..n_tensors {
            infos.push(r.tensor_info(&mut names)?);
        }

        let header_len = r.at as u64;
        let data_offset = if n_tensors > 0 {
            pad(header_len, alignment).ok_or_else(|| Error::Invalid("the header length overflows".into()))?
        } else {
            header_len
        };

        // tensors follow each other, each padded to the alignment, as llama.cpp requires
        let mut tensors = Vec::with_capacity(infos.len());
        let mut next: u64 = 0;
        for (name, dims, kind, offset, size) in infos {
            if offset != next {
                return invalid(format!("the tensor {name} lies at {offset}, not at {next}"));
            }
            let start = data_offset.checked_add(offset);
            let end = start.and_then(|s| s.checked_add(size));
            let (Some(start), Some(end)) = (start, end) else {
                return invalid(format!("the tensor {name} lies beyond 2^64"));
            };
            next = pad(size, alignment)
                .and_then(|padded| next.checked_add(padded))
                .ok_or_else(|| Error::Invalid("the data length overflows".into()))?;
            tensors.push(Tensor { name, dims, kind, range: start..end });
        }
        let file_len =
            data_offset.checked_add(next).ok_or_else(|| Error::Invalid("the file length overflows".into()))?;

        Ok(Self { version, alignment, metadata, tensors, data_offset, file_len })
    }

    /// The metadata value under `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// `general.architecture`, such as `llama` or `qwen3`.
    #[must_use]
    pub fn architecture(&self) -> Option<&str> {
        self.get("general.architecture").and_then(Value::as_str)
    }

    /// `general.name`, the model's own name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.get("general.name").and_then(Value::as_str)
    }

    /// The number of repeating layers, `<architecture>.block_count`.
    #[must_use]
    pub fn block_count(&self) -> Option<u32> {
        let arch = self.architecture()?;
        self.get(&format!("{arch}.block_count")).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok())
    }

    #[must_use]
    pub fn tensor(&self, name: &str) -> Option<&Tensor> {
        self.tensors.iter().find(|t| t.name == name)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes GGUF files for tests: metadata of a few types and tensors whose bytes are given.
    #[derive(Default)]
    pub(crate) struct Builder {
        metadata: Vec<(String, u32, Vec<u8>)>,
        tensors: Vec<(String, Vec<u64>, u32, Vec<u8>)>,
        pub(crate) version: Option<u32>,
    }

    fn string(s: &str) -> Vec<u8> {
        let mut out = (s.len() as u64).to_le_bytes().to_vec();
        out.extend_from_slice(s.as_bytes());
        out
    }

    impl Builder {
        pub(crate) fn string(mut self, key: &str, value: &str) -> Self {
            self.metadata.push((key.into(), TYPE_STRING, string(value)));
            self
        }

        pub(crate) fn u32(mut self, key: &str, value: u32) -> Self {
            self.metadata.push((key.into(), TYPE_U32, value.to_le_bytes().to_vec()));
            self
        }

        /// An array of strings, as the vocabulary is.
        pub(crate) fn strings(mut self, key: &str, values: &[&str]) -> Self {
            let mut bytes = TYPE_STRING.to_le_bytes().to_vec();
            bytes.extend_from_slice(&(values.len() as u64).to_le_bytes());
            for v in values {
                bytes.extend(string(v));
            }
            self.metadata.push((key.into(), TYPE_ARRAY, bytes));
            self
        }

        /// An f32 tensor of the given shape filled with `fill`.
        pub(crate) fn tensor(mut self, name: &str, dims: &[u64], fill: u8) -> Self {
            let n: u64 = dims.iter().product();
            self.tensors.push((name.into(), dims.to_vec(), 0, vec![fill; usize::try_from(n * 4).unwrap()]));
            self
        }

        pub(crate) fn build(self) -> Vec<u8> {
            let mut out = MAGIC.to_vec();
            out.extend_from_slice(&self.version.unwrap_or(3).to_le_bytes());
            out.extend_from_slice(&(self.tensors.len() as u64).to_le_bytes());
            out.extend_from_slice(&(self.metadata.len() as u64).to_le_bytes());
            for (key, kind, bytes) in &self.metadata {
                out.extend(string(key));
                out.extend_from_slice(&kind.to_le_bytes());
                out.extend_from_slice(bytes);
            }
            let mut offset = 0u64;
            for (name, dims, kind, data) in &self.tensors {
                out.extend(string(name));
                out.extend_from_slice(&u32::try_from(dims.len()).unwrap().to_le_bytes());
                for d in dims {
                    out.extend_from_slice(&d.to_le_bytes());
                }
                out.extend_from_slice(&kind.to_le_bytes());
                out.extend_from_slice(&offset.to_le_bytes());
                offset = pad(offset + data.len() as u64, DEFAULT_ALIGNMENT).unwrap();
            }
            if !self.tensors.is_empty() {
                out.resize(usize::try_from(pad(out.len() as u64, DEFAULT_ALIGNMENT).unwrap()).unwrap(), 0);
            }
            for (_, _, _, data) in &self.tensors {
                out.extend_from_slice(data);
                out.resize(usize::try_from(pad(out.len() as u64, DEFAULT_ALIGNMENT).unwrap()).unwrap(), 0);
            }
            out
        }
    }

    /// A model of the llama architecture with `layers` layers of two tensors each.
    pub(crate) fn llama(layers: u32, tied: bool) -> Vec<u8> {
        let mut b = Builder::default()
            .string("general.architecture", "llama")
            .string("general.name", "Tiny Llama")
            .u32("llama.block_count", layers)
            .strings("tokenizer.ggml.tokens", &["<s>", "</s>", "hello"])
            .tensor("token_embd.weight", &[8, 3], 1)
            .tensor("rope_freqs.weight", &[4], 2);
        for i in 0..layers {
            let fill = 10 + u8::try_from(i).unwrap();
            b = b.tensor(&format!("blk.{i}.attn_q.weight"), &[8, 8], fill).tensor(
                &format!("blk.{i}.ffn_down.weight"),
                &[8, 16],
                fill,
            );
        }
        b = b.tensor("output_norm.weight", &[8], 3);
        if !tied {
            b = b.tensor("output.weight", &[8, 3], 4);
        }
        b.build()
    }

    #[test]
    fn reads_a_header() {
        let file = llama(4, false);
        let header = Header::parse(&file).unwrap();
        assert_eq!(header.version, 3);
        assert_eq!(header.architecture(), Some("llama"));
        assert_eq!(header.name(), Some("Tiny Llama"));
        assert_eq!(header.block_count(), Some(4));
        assert_eq!(header.get("tokenizer.ggml.tokens"), Some(&Value::Array { kind: TYPE_STRING, len: 3 }));
        assert_eq!(header.tensors.len(), 2 + 4 * 2 + 2);
        assert_eq!(header.file_len, file.len() as u64);
        assert_eq!(header.data_offset % DEFAULT_ALIGNMENT, 0);
        // every tensor's bytes are where the header says
        let q = header.tensor("blk.2.attn_q.weight").unwrap();
        assert_eq!(q.range.end - q.range.start, 8 * 8 * 4);
        let start = usize::try_from(q.range.start).unwrap();
        assert!(file[start..start + 256].iter().all(|&b| b == 12));
    }

    #[test]
    fn says_how_much_more_it_needs() {
        let file = llama(2, true);
        let header = Header::parse(&file).unwrap();
        for cut in [0, 3, 10, 40, usize::try_from(header.data_offset).unwrap() - 40] {
            match Header::parse(&file[..cut]) {
                Err(Error::Incomplete { needed }) => assert!(needed > cut as u64 && needed <= header.data_offset),
                other => panic!("{cut} bytes: {other:?}"),
            }
        }
        // the header alone is enough
        let alone = usize::try_from(header.data_offset).unwrap();
        assert_eq!(Header::parse(&file[..alone]).unwrap(), header);
    }

    #[test]
    fn refuses_what_llama_cpp_refuses() {
        let mut file = llama(1, true);
        file[0] = b'X';
        assert!(matches!(Header::parse(&file), Err(Error::Invalid(_))));

        let old = Builder { version: Some(1), ..Builder::default() }.build();
        assert!(matches!(Header::parse(&old), Err(Error::Invalid(_))));

        let twice = Builder::default().u32("a", 1).u32("a", 2).build();
        assert!(matches!(Header::parse(&twice), Err(Error::Invalid(why)) if why.contains("twice")));

        let alignment = Builder::default().u32("general.alignment", 24).build();
        assert!(matches!(Header::parse(&alignment), Err(Error::Invalid(why)) if why.contains("power of two")));

        // a vocabulary that claims 2^62 strings ends with the bytes, not with an allocation
        let mut huge = Builder::default().strings("tokenizer.ggml.tokens", &[]).build();
        let at = huge.len() - 8;
        huge[at..].copy_from_slice(&(1u64 << 62).to_le_bytes());
        assert!(matches!(Header::parse(&huge), Err(Error::Incomplete { .. })));
    }

    proptest::proptest! {
        #[test]
        fn random_bytes_never_panic(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..1024)) {
            let _ = Header::parse(&bytes);
        }

        // a damaged or cut header is refused or read whole; what is read lies inside the file, in order
        #[test]
        fn damaged_headers_never_panic(
            flips in proptest::collection::vec(
                (proptest::prelude::any::<proptest::sample::Index>(), proptest::prelude::any::<u8>()), 0..8),
            cut in proptest::prelude::any::<proptest::sample::Index>(),
        ) {
            let mut file = llama(3, false);
            for (at, byte) in flips {
                let at = at.index(file.len());
                file[at] = byte;
            }
            let cut = cut.index(file.len() + 1);
            if let Ok(header) = Header::parse(&file[..cut]) {
                let mut end = header.data_offset;
                for t in &header.tensors {
                    proptest::prop_assert!(t.range.start >= end && t.range.start <= t.range.end);
                    end = t.range.end;
                }
                proptest::prop_assert!(end <= header.file_len);
            }
        }
    }

    #[test]
    fn reads_a_real_model() {
        // stories15M (`just models`); CI sets OMNARC_TEST_MODEL in the engine job, where the model is
        let Some(path) = std::env::var_os("OMNARC_TEST_MODEL") else {
            eprintln!("skipped: OMNARC_TEST_MODEL is not set");
            return;
        };
        let file = std::fs::read(path).unwrap();
        let header = Header::parse(&file).unwrap();
        assert_eq!(header.architecture(), Some("llama"));
        assert_eq!(header.block_count(), Some(6));
        assert_eq!(header.file_len, file.len() as u64);
        assert!(header.tensors.iter().any(|t| t.kind.name == "q8_0"));
    }
}
