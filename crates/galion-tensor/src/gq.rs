// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! `.gq` — Galion's own packed weight container (format v1). **Reader only.**
//!
//! Spec: `galion-voice-20/v4/agents/A07-gq-format.md`. Zero llama.cpp/GGML-family containers, zero third-party deps.
//!
//! Layout (all integers little-endian):
//! ```text
//! [0..64)            header (fixed 64 B)
//! [table_offset..)   tensor table: n_tensors variable-length entries
//! [data_offset..)    data section; every tensor starts at a multiple of `alignment`
//! ```
//! The writer is deferred until a real weight needs packing (02-OPEN-DECISIONS G);
//! a test-only builder in `tests` produces synthetic files.
//!
//! Safety posture: every offset/length is bounds- and overflow-checked before any slice is
//! taken; the parser never panics on arbitrary bytes (fuzz test below).

use crate::{Tensor, TensorError};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

pub const MAGIC: [u8; 8] = *b"GALIONGQ";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 64;
pub const MIN_ALIGN: u32 = 64;
pub const MAX_ALIGN: u32 = 1 << 16;
pub const MAX_RANK: usize = 8;
pub const MAX_NAME_LEN: usize = 1024;
pub const MAX_TENSORS: u32 = 1 << 20;

// ---------------------------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GqError {
    Io(String),
    BadMagic,
    UnsupportedVersion(u16),
    UnknownFlags(u16),
    BadAlignment(u32),
    Truncated { what: &'static str, need: u64, have: u64 },
    OutOfBounds(&'static str),
    BadName,
    DuplicateName(String),
    UnknownDType(u8),
    BadRank(u8),
    SizeMismatch { name: String, expected: u64, got: u64 },
    Misaligned(String),
    Overlap(String),
    TrailingTable,
    ChecksumMismatch(String),
    NotFound(String),
    BadManifest(String),
}

impl fmt::Display for GqError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gq: {self:?}")
    }
}
impl std::error::Error for GqError {}

impl From<GqError> for TensorError {
    fn from(e: GqError) -> Self { TensorError::InvalidShape(e.to_string()) }
}

pub type GqResult<T> = std::result::Result<T, GqError>;

// ---------------------------------------------------------------------------------------------
// DType codes
// ---------------------------------------------------------------------------------------------

/// On-disk dtype code. Codes are frozen once assigned; new quant schemes get new codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum GqDType {
    F32 = 0,
    BF16 = 1,
    /// Symmetric per-tensor int8: real = q * scale (entry `scale` field).
    I8 = 2,
}

impl GqDType {
    pub fn from_code(c: u8) -> GqResult<Self> {
        match c {
            0 => Ok(GqDType::F32),
            1 => Ok(GqDType::BF16),
            2 => Ok(GqDType::I8),
            x => Err(GqError::UnknownDType(x)),
        }
    }
    pub const fn size_of(self) -> u64 {
        match self { GqDType::F32 => 4, GqDType::BF16 => 2, GqDType::I8 => 1 }
    }
}

// ---------------------------------------------------------------------------------------------
// Header + table
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GqHeader {
    pub version: u16,
    pub flags: u16,
    pub alignment: u32,
    pub n_tensors: u32,
    pub table_offset: u64,
    pub table_len: u64,
    pub data_offset: u64,
    pub data_len: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GqEntry {
    pub name: String,
    pub dtype: GqDType,
    pub dims: Vec<usize>,
    pub scale: f32,
    /// Relative to `data_offset`.
    pub offset: u64,
    pub len: u64,
    pub sha256: [u8; 32],
}

impl GqEntry {
    pub fn numel(&self) -> u64 { self.dims.iter().map(|&d| d as u64).product() }
}

struct Cur<'a> { b: &'a [u8], p: usize }
impl<'a> Cur<'a> {
    fn take(&mut self, n: usize, what: &'static str) -> GqResult<&'a [u8]> {
        let end = self.p.checked_add(n).ok_or(GqError::OutOfBounds(what))?;
        if end > self.b.len() {
            return Err(GqError::Truncated { what, need: end as u64, have: self.b.len() as u64 });
        }
        let s = &self.b[self.p..end];
        self.p = end;
        Ok(s)
    }
    fn u8(&mut self, w: &'static str) -> GqResult<u8> { Ok(self.take(1, w)?[0]) }
    fn u16(&mut self, w: &'static str) -> GqResult<u16> { Ok(u16::from_le_bytes(self.take(2, w)?.try_into().unwrap())) }
    fn u32(&mut self, w: &'static str) -> GqResult<u32> { Ok(u32::from_le_bytes(self.take(4, w)?.try_into().unwrap())) }
    fn u64(&mut self, w: &'static str) -> GqResult<u64> { Ok(u64::from_le_bytes(self.take(8, w)?.try_into().unwrap())) }
    fn f32(&mut self, w: &'static str) -> GqResult<f32> { Ok(f32::from_bits(self.u32(w)?)) }
}

fn parse_header(b: &[u8]) -> GqResult<GqHeader> {
    let mut c = Cur { b, p: 0 };
    let magic = c.take(8, "magic")?;
    if magic != MAGIC { return Err(GqError::BadMagic); }
    let version = c.u16("version")?;
    if version != VERSION { return Err(GqError::UnsupportedVersion(version)); }
    let flags = c.u16("flags")?;
    if flags != 0 { return Err(GqError::UnknownFlags(flags)); }
    let alignment = c.u32("alignment")?;
    if !alignment.is_power_of_two() || !(MIN_ALIGN..=MAX_ALIGN).contains(&alignment) {
        return Err(GqError::BadAlignment(alignment));
    }
    let n_tensors = c.u32("n_tensors")?;
    if n_tensors > MAX_TENSORS { return Err(GqError::OutOfBounds("n_tensors")); }
    let _reserved = c.u32("reserved0")?;
    let table_offset = c.u64("table_offset")?;
    let table_len = c.u64("table_len")?;
    let data_offset = c.u64("data_offset")?;
    let data_len = c.u64("data_len")?;
    // bytes 56..64 reserved (must be zero for v1).
    if c.take(8, "reserved1")?.iter().any(|&x| x != 0) { return Err(GqError::UnknownFlags(0xffff)); }
    let h = GqHeader { version, flags, alignment, n_tensors, table_offset, table_len, data_offset, data_len };
    if h.table_offset != HEADER_LEN as u64 { return Err(GqError::OutOfBounds("table_offset")); }
    let table_end = h.table_offset.checked_add(h.table_len).ok_or(GqError::OutOfBounds("table_len"))?;
    if h.data_offset < table_end { return Err(GqError::OutOfBounds("data_offset<table_end")); }
    if h.data_offset % h.alignment as u64 != 0 { return Err(GqError::Misaligned("data_offset".into())); }
    let data_end = h.data_offset.checked_add(h.data_len).ok_or(GqError::OutOfBounds("data_len"))?;
    if data_end != b.len() as u64 {
        return Err(GqError::Truncated { what: "file size", need: data_end, have: b.len() as u64 });
    }
    Ok(h)
}

fn parse_table(b: &[u8], h: &GqHeader) -> GqResult<Vec<GqEntry>> {
    let t0 = h.table_offset as usize;
    let t1 = t0 + h.table_len as usize; // bounded by parse_header (≤ file len)
    let mut c = Cur { b: &b[t0..t1], p: 0 };
    let mut out = Vec::with_capacity(h.n_tensors.min(4096) as usize);
    let mut seen = HashMap::new();
    for _ in 0..h.n_tensors {
        let nl = c.u16("name_len")? as usize;
        if nl == 0 || nl > MAX_NAME_LEN { return Err(GqError::BadName); }
        let name = std::str::from_utf8(c.take(nl, "name")?).map_err(|_| GqError::BadName)?.to_owned();
        let dtype = GqDType::from_code(c.u8("dtype")?)?;
        let rank = c.u8("rank")?;
        if rank as usize > MAX_RANK { return Err(GqError::BadRank(rank)); }
        let _pad = c.u16("pad")?;
        let mut dims = Vec::with_capacity(rank as usize);
        let mut numel: u64 = 1;
        for _ in 0..rank {
            let d = c.u64("dim")?;
            numel = numel.checked_mul(d).ok_or(GqError::OutOfBounds("numel"))?;
            dims.push(usize::try_from(d).map_err(|_| GqError::OutOfBounds("dim"))?);
        }
        let scale = c.f32("scale")?;
        let offset = c.u64("offset")?;
        let len = c.u64("len")?;
        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(c.take(32, "sha256")?);
        let expected = numel.checked_mul(dtype.size_of()).ok_or(GqError::OutOfBounds("nbytes"))?;
        if len != expected { return Err(GqError::SizeMismatch { name, expected, got: len }); }
        if offset % h.alignment as u64 != 0 { return Err(GqError::Misaligned(name)); }
        let end = offset.checked_add(len).ok_or(GqError::OutOfBounds("tensor end"))?;
        if end > h.data_len { return Err(GqError::OutOfBounds("tensor beyond data section")); }
        if dtype == GqDType::I8 && !(scale.is_finite() && scale > 0.0) {
            return Err(GqError::OutOfBounds("i8 scale"));
        }
        if seen.insert(name.clone(), out.len()).is_some() { return Err(GqError::DuplicateName(name)); }
        out.push(GqEntry { name, dtype, dims, scale, offset, len, sha256 });
    }
    if c.p != c.b.len() { return Err(GqError::TrailingTable); }
    // No overlap between tensors.
    let mut spans: Vec<(u64, u64, usize)> = out.iter().enumerate().map(|(i, e)| (e.offset, e.offset + e.len, i)).collect();
    spans.sort_unstable();
    for w in spans.windows(2) {
        if w[1].0 < w[0].1 { return Err(GqError::Overlap(out[w[1].2].name.clone())); }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Backing bytes: mmap (unix) or aligned heap buffer
// ---------------------------------------------------------------------------------------------

/// How to warm pages after mapping. Latency-critical paths (80 ms frame budget) want weights
/// resident before the first `step()`; a page fault mid-frame is a missed deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefault {
    /// Lazy: pages fault in on first touch.
    None,
    /// `madvise(MADV_WILLNEED)` — async readahead hint.
    WillNeed,
    /// Linux `MAP_POPULATE` (synchronous prefault at map time); elsewhere falls back to `Touch`.
    Populate,
    /// Read one byte per 4 KiB page (portable, synchronous).
    Touch,
}

enum Backing {
    #[cfg(unix)]
    Mmap { ptr: *mut u8, len: usize },
    /// u64 words give 8-byte alignment so f32/bf16 views can be zero-copy.
    Heap { words: Vec<u64>, len: usize },
}
// SAFETY: the mapping is PROT_READ + MAP_PRIVATE and never mutated after creation.
unsafe impl Send for Backing {}
unsafe impl Sync for Backing {}

impl Backing {
    fn bytes(&self) -> &[u8] {
        match self {
            #[cfg(unix)]
            Backing::Mmap { ptr, len } => unsafe { std::slice::from_raw_parts(*ptr, *len) },
            Backing::Heap { words, len } => unsafe { std::slice::from_raw_parts(words.as_ptr() as *const u8, *len) },
        }
    }
    fn from_bytes(b: &[u8]) -> Self {
        let mut words = vec![0u64; b.len().div_ceil(8)];
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), words.as_mut_ptr() as *mut u8, b.len()) };
        Backing::Heap { words, len: b.len() }
    }
}

#[cfg(unix)]
impl Drop for Backing {
    fn drop(&mut self) {
        if let Backing::Mmap { ptr, len } = self {
            unsafe { sys::munmap(*ptr as *mut std::ffi::c_void, *len) };
        }
    }
}

#[cfg(unix)]
mod sys {
    use std::ffi::c_void;
    extern "C" {
        pub fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut c_void;
        pub fn munmap(addr: *mut c_void, len: usize) -> i32;
        pub fn madvise(addr: *mut c_void, len: usize, advice: i32) -> i32;
    }
    pub const PROT_READ: i32 = 1;
    pub const MAP_PRIVATE: i32 = 2;
    #[cfg(target_os = "linux")]
    pub const MAP_POPULATE: i32 = 0x8000;
    pub const MADV_WILLNEED: i32 = 3;
}

#[cfg(unix)]
fn map_file(file: &std::fs::File, len: usize, pf: Prefault) -> GqResult<Backing> {
    use std::os::unix::io::AsRawFd;
    if len == 0 { return Err(GqError::Truncated { what: "file", need: HEADER_LEN as u64, have: 0 }); }
    #[allow(unused_mut)]
    let mut flags = sys::MAP_PRIVATE;
    #[cfg(target_os = "linux")]
    if pf == Prefault::Populate { flags |= sys::MAP_POPULATE; }
    let p = unsafe { sys::mmap(std::ptr::null_mut(), len, sys::PROT_READ, flags, file.as_raw_fd(), 0) };
    if p as isize == -1 || p.is_null() {
        return Err(GqError::Io(format!("mmap failed: {}", std::io::Error::last_os_error())));
    }
    if pf == Prefault::WillNeed {
        unsafe { sys::madvise(p, len, sys::MADV_WILLNEED) }; // advisory; ignore result
    }
    Ok(Backing::Mmap { ptr: p as *mut u8, len })
}

fn touch_pages(b: &[u8]) -> u8 {
    let mut acc = 0u8;
    for i in (0..b.len()).step_by(4096) { acc ^= unsafe { std::ptr::read_volatile(b.as_ptr().add(i)) }; }
    acc
}

// ---------------------------------------------------------------------------------------------
// GqFile
// ---------------------------------------------------------------------------------------------

pub struct GqFile {
    backing: Backing,
    header: GqHeader,
    entries: Vec<GqEntry>,
    index: HashMap<String, usize>,
}

impl fmt::Debug for GqFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GqFile").field("header", &self.header).field("n", &self.entries.len()).finish()
    }
}

impl GqFile {
    /// Open via mmap (unix) with the given prefault policy; heap read elsewhere.
    pub fn open(path: impl AsRef<Path>, pf: Prefault) -> GqResult<Self> {
        let path = path.as_ref();
        let file = std::fs::File::open(path).map_err(|e| GqError::Io(e.to_string()))?;
        let len = file.metadata().map_err(|e| GqError::Io(e.to_string()))?.len();
        let len = usize::try_from(len).map_err(|_| GqError::OutOfBounds("file len"))?;
        #[cfg(unix)]
        let backing = map_file(&file, len, pf)?;
        #[cfg(not(unix))]
        let backing = {
            let _ = file;
            Backing::from_bytes(&std::fs::read(path).map_err(|e| GqError::Io(e.to_string()))?)
        };
        let g = Self::from_backing(backing)?;
        let needs_touch = pf == Prefault::Touch || (pf == Prefault::Populate && !cfg!(target_os = "linux"));
        if needs_touch { std::hint::black_box(touch_pages(g.backing.bytes())); }
        Ok(g)
    }

    /// Parse from an in-memory buffer (copied into an 8-byte-aligned heap buffer).
    pub fn from_bytes(b: &[u8]) -> GqResult<Self> { Self::from_backing(Backing::from_bytes(b)) }

    fn from_backing(backing: Backing) -> GqResult<Self> {
        let b = backing.bytes();
        let header = parse_header(b)?;
        let entries = parse_table(b, &header)?;
        let index = entries.iter().enumerate().map(|(i, e)| (e.name.clone(), i)).collect();
        Ok(GqFile { backing, header, entries, index })
    }

    pub fn header(&self) -> &GqHeader { &self.header }
    pub fn entries(&self) -> &[GqEntry] { &self.entries }
    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn entry(&self, name: &str) -> Option<&GqEntry> { self.index.get(name).map(|&i| &self.entries[i]) }

    /// Raw little-endian bytes of a tensor (zero-copy into the mapping).
    pub fn raw(&self, name: &str) -> GqResult<&[u8]> {
        let e = self.entry(name).ok_or_else(|| GqError::NotFound(name.into()))?;
        Ok(self.raw_entry(e))
    }
    fn raw_entry(&self, e: &GqEntry) -> &[u8] {
        let s = (self.header.data_offset + e.offset) as usize;
        &self.backing.bytes()[s..s + e.len as usize]
    }

    /// Zero-copy `&[f32]` view (little-endian hosts; data is ≥64-byte aligned in-file and the
    /// mapping/heap base is ≥8-byte aligned).
    pub fn f32_view(&self, name: &str) -> GqResult<Option<&[f32]>> {
        let e = self.entry(name).ok_or_else(|| GqError::NotFound(name.into()))?;
        if e.dtype != GqDType::F32 || cfg!(target_endian = "big") { return Ok(None); }
        let r = self.raw_entry(e);
        // SAFETY: f32 has no invalid bit patterns; alignment checked by align_to.
        let (pre, mid, post) = unsafe { r.align_to::<f32>() };
        Ok(if pre.is_empty() && post.is_empty() { Some(mid) } else { None })
    }

    /// Materialize as a `Tensor` (copies; F32→F32, BF16→BF16 storage, I8→dequantized F32).
    pub fn tensor(&self, name: &str) -> GqResult<Tensor> {
        let e = self.entry(name).ok_or_else(|| GqError::NotFound(name.into()))?;
        let r = self.raw_entry(e);
        let v: Vec<f32> = match e.dtype {
            GqDType::F32 => r.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
            GqDType::BF16 => r.chunks_exact(2).map(|c| f32::from_bits((u16::from_le_bytes(c.try_into().unwrap()) as u32) << 16)).collect(),
            GqDType::I8 => r.iter().map(|&q| q as i8 as f32 * e.scale).collect(),
        };
        let t = Tensor::from_vec(v, e.dims.as_slice()).map_err(|x| GqError::BadManifest(x.to_string()))?;
        Ok(if e.dtype == GqDType::BF16 { t.to_bf16() } else { t })
    }

    /// Verify one tensor's SHA-256 against its table entry.
    pub fn verify(&self, name: &str) -> GqResult<()> {
        let e = self.entry(name).ok_or_else(|| GqError::NotFound(name.into()))?;
        if sha256(self.raw_entry(e)) != e.sha256 { return Err(GqError::ChecksumMismatch(e.name.clone())); }
        Ok(())
    }
    pub fn verify_all(&self) -> GqResult<()> {
        for e in &self.entries { self.verify(&e.name)?; }
        Ok(())
    }
    /// SHA-256 of the whole file (what `SHA256SUMS` records).
    pub fn file_sha256(&self) -> [u8; 32] { sha256(self.backing.bytes()) }
}

// ---------------------------------------------------------------------------------------------
// SHA-256 (FIPS 180-4), dependency-free
// ---------------------------------------------------------------------------------------------

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn compress(st: &mut [u32; 8], blk: &[u8]) {
    let mut w = [0u32; 64];
    for i in 0..16 { w[i] = u32::from_be_bytes(blk[i * 4..i * 4 + 4].try_into().unwrap()); }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *st;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        h = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
    }
    for (s, v) in st.iter_mut().zip([a, b, c, d, e, f, g, h]) { *s = s.wrapping_add(v); }
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut st: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut chunks = data.chunks_exact(64);
    for blk in &mut chunks { compress(&mut st, blk); }
    let rem = chunks.remainder();
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let tl = if rem.len() < 56 { 64 } else { 128 };
    tail[tl - 8..tl].copy_from_slice(&((data.len() as u64).wrapping_mul(8)).to_be_bytes());
    for blk in tail[..tl].chunks_exact(64) { compress(&mut st, blk); }
    let mut out = [0u8; 32];
    for (i, v) in st.iter().enumerate() { out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes()); }
    out
}

pub fn to_hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

// ---------------------------------------------------------------------------------------------
// SHA256SUMS manifest (coreutils `sha256sum` format: "<64 hex>  <path>", '*' = binary mode)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestLine { pub sha256: [u8; 32], pub path: String }

pub fn parse_sha256sums(text: &str) -> GqResult<Vec<ManifestLine>> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') { continue; }
        let bad = || GqError::BadManifest(format!("line {}", n + 1));
        if line.len() < 66 || !line.is_char_boundary(64) { return Err(bad()); }
        let (hex, rest) = line.split_at(64);
        let path = rest.strip_prefix("  ").or_else(|| rest.strip_prefix(" *")).ok_or_else(bad)?;
        if path.is_empty() || path.starts_with('/') || path.split('/').any(|c| c == "..") { return Err(bad()); }
        let mut sha = [0u8; 32];
        for i in 0..32 { sha[i] = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| bad())?; }
        out.push(ManifestLine { sha256: sha, path: path.to_owned() });
    }
    Ok(out)
}

/// Verify every manifest line against files under `root`. Reads files fully (offline tool path).
pub fn verify_sha256sums(root: impl AsRef<Path>, text: &str) -> GqResult<()> {
    for l in parse_sha256sums(text)? {
        let b = std::fs::read(root.as_ref().join(&l.path)).map_err(|e| GqError::Io(format!("{}: {e}", l.path)))?;
        if sha256(&b) != l.sha256 { return Err(GqError::ChecksumMismatch(l.path)); }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tests — synthetic tensors only, no real weights.
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only builder (the production writer is deferred; see A07 doc §6).
    struct Syn { name: String, dtype: GqDType, dims: Vec<usize>, scale: f32, bytes: Vec<u8> }

    fn build(ts: &[Syn], align: u32) -> Vec<u8> {
        let mut table = Vec::new();
        let mut data: Vec<u8> = Vec::new();
        for t in ts {
            while data.len() % align as usize != 0 { data.push(0); }
            let off = data.len() as u64;
            data.extend_from_slice(&t.bytes);
            table.extend_from_slice(&(t.name.len() as u16).to_le_bytes());
            table.extend_from_slice(t.name.as_bytes());
            table.push(t.dtype as u8);
            table.push(t.dims.len() as u8);
            table.extend_from_slice(&0u16.to_le_bytes());
            for &d in &t.dims { table.extend_from_slice(&(d as u64).to_le_bytes()); }
            table.extend_from_slice(&t.scale.to_le_bytes());
            table.extend_from_slice(&off.to_le_bytes());
            table.extend_from_slice(&(t.bytes.len() as u64).to_le_bytes());
            table.extend_from_slice(&sha256(&t.bytes));
        }
        let table_end = HEADER_LEN as u64 + table.len() as u64;
        let data_off = table_end.div_ceil(align as u64) * align as u64;
        let mut f = Vec::new();
        f.extend_from_slice(&MAGIC);
        f.extend_from_slice(&VERSION.to_le_bytes());
        f.extend_from_slice(&0u16.to_le_bytes());
        f.extend_from_slice(&align.to_le_bytes());
        f.extend_from_slice(&(ts.len() as u32).to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&(HEADER_LEN as u64).to_le_bytes());
        f.extend_from_slice(&(table.len() as u64).to_le_bytes());
        f.extend_from_slice(&data_off.to_le_bytes());
        f.extend_from_slice(&(data.len() as u64).to_le_bytes());
        f.extend_from_slice(&[0u8; 8]);
        assert_eq!(f.len(), HEADER_LEN);
        f.extend_from_slice(&table);
        f.resize(data_off as usize, 0);
        f.extend_from_slice(&data);
        f
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
        fn f32(&mut self) -> f32 { (self.next() % 20001) as f32 / 1000.0 - 10.0 }
    }

    fn f32_syn(name: &str, dims: &[usize], r: &mut Rng) -> (Syn, Vec<f32>) {
        let n: usize = dims.iter().product();
        let v: Vec<f32> = (0..n).map(|_| r.f32()).collect();
        let bytes = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        (Syn { name: name.into(), dtype: GqDType::F32, dims: dims.to_vec(), scale: 0.0, bytes }, v)
    }

    fn sample(r: &mut Rng) -> (Vec<u8>, Vec<f32>, Vec<f32>, Vec<i8>) {
        let (a, av) = f32_syn("enc.w", &[3, 5], r);
        let bv: Vec<f32> = (0..7).map(|_| crate::Bf16::from_f32(r.f32()).to_f32()).collect();
        let b = Syn { name: "enc.b".into(), dtype: GqDType::BF16, dims: vec![7], scale: 0.0,
            bytes: bv.iter().flat_map(|x| ((x.to_bits() >> 16) as u16).to_le_bytes()).collect() };
        let q: Vec<i8> = (0..12).map(|i| (i as i8 - 6) * 10).collect();
        let c = Syn { name: "dec.q".into(), dtype: GqDType::I8, dims: vec![2, 2, 3], scale: 0.05, bytes: q.iter().map(|&x| x as u8).collect() };
        let s = Syn { name: "scalar".into(), dtype: GqDType::F32, dims: vec![], scale: 0.0, bytes: 1.5f32.to_le_bytes().to_vec() };
        (build(&[a, b, c, s], 64), av, bv, q)
    }

    #[test]
    fn sha256_vectors() {
        assert_eq!(to_hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(to_hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(to_hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1");
        assert_eq!(to_hex(&sha256(&vec![b'a'; 1_000_000])),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn roundtrip_all_dtypes() {
        let mut r = Rng(0x9E3779B97F4A7C15);
        let (bytes, av, bv, q) = sample(&mut r);
        let g = GqFile::from_bytes(&bytes).unwrap();
        assert_eq!(g.len(), 4);
        g.verify_all().unwrap();
        let a = g.tensor("enc.w").unwrap();
        assert_eq!(a.dims(), &[3, 5]);
        assert_eq!(a.to_vec_f32(), av);
        assert_eq!(g.f32_view("enc.w").unwrap().unwrap(), av.as_slice());
        let b = g.tensor("enc.b").unwrap();
        assert_eq!(b.dtype(), crate::DType::BF16);
        assert_eq!(b.to_vec_f32(), bv);
        let c = g.tensor("dec.q").unwrap();
        assert_eq!(c.dims(), &[2, 2, 3]);
        let want: Vec<f32> = q.iter().map(|&x| x as f32 * 0.05).collect();
        assert_eq!(c.to_vec_f32(), want);
        assert_eq!(g.tensor("scalar").unwrap().to_vec_f32(), vec![1.5]);
        for e in g.entries() { assert_eq!(e.offset % 64, 0); }
        assert!(matches!(g.tensor("nope"), Err(GqError::NotFound(_))));
    }

    #[test]
    fn mmap_open_with_every_prefault() {
        let mut r = Rng(42);
        let (bytes, av, _, _) = sample(&mut r);
        let dir = std::env::temp_dir().join(format!("gq-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("syn.gq");
        std::fs::write(&p, &bytes).unwrap();
        for pf in [Prefault::None, Prefault::WillNeed, Prefault::Populate, Prefault::Touch] {
            let g = GqFile::open(&p, pf).unwrap();
            g.verify_all().unwrap();
            assert_eq!(g.f32_view("enc.w").unwrap().unwrap(), av.as_slice());
            assert_eq!(g.file_sha256(), sha256(&bytes));
        }
        // SHA256SUMS manifest round trip.
        let sums = format!("# galion\n{}  syn.gq\n", to_hex(&sha256(&bytes)));
        verify_sha256sums(&dir, &sums).unwrap();
        let bad = format!("{}  syn.gq\n", to_hex(&[0u8; 32]));
        assert!(matches!(verify_sha256sums(&dir, &bad), Err(GqError::ChecksumMismatch(_))));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn manifest_rejects_traversal_and_garbage() {
        let h = "0".repeat(64);
        assert!(parse_sha256sums(&format!("{h}  ../etc/passwd")).is_err());
        assert!(parse_sha256sums(&format!("{h}  /abs")).is_err());
        assert!(parse_sha256sums("zz  x").is_err());
        assert!(parse_sha256sums(&format!("{}  x", "g".repeat(64))).is_err());
        assert_eq!(parse_sha256sums(&format!("{h} *w.gq\n")).unwrap()[0].path, "w.gq");
    }

    #[test]
    fn detects_corruption() {
        let mut r = Rng(7);
        let (mut bytes, _, _, _) = sample(&mut r);
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff; // flip a byte inside the last tensor's data
        let g = GqFile::from_bytes(&bytes).unwrap();
        assert!(matches!(g.verify_all(), Err(GqError::ChecksumMismatch(_))));
    }

    #[test]
    fn rejects_structural_errors() {
        let mut r = Rng(11);
        let (good, _, _, _) = sample(&mut r);
        let mut b = good.clone(); b[0] = b'X';
        assert_eq!(GqFile::from_bytes(&b).unwrap_err(), GqError::BadMagic);
        let mut b = good.clone(); b[8] = 2;
        assert_eq!(GqFile::from_bytes(&b).unwrap_err(), GqError::UnsupportedVersion(2));
        let mut b = good.clone(); b[12..16].copy_from_slice(&48u32.to_le_bytes());
        assert_eq!(GqFile::from_bytes(&b).unwrap_err(), GqError::BadAlignment(48));
        let b = &good[..good.len() - 1];
        assert!(matches!(GqFile::from_bytes(b), Err(GqError::Truncated { .. })));
        assert!(GqFile::from_bytes(&[]).is_err());
        // Duplicate names.
        let (a, _) = f32_syn("x", &[2], &mut r);
        let (a2, _) = f32_syn("x", &[2], &mut r);
        assert!(matches!(GqFile::from_bytes(&build(&[a, a2], 64)), Err(GqError::DuplicateName(_))));
        // Size mismatch: dims say 4 elems, 3 present.
        let s = Syn { name: "s".into(), dtype: GqDType::F32, dims: vec![4], scale: 0.0, bytes: vec![0; 12] };
        assert!(matches!(GqFile::from_bytes(&build(&[s], 64)), Err(GqError::SizeMismatch { .. })));
        // I8 with non-positive scale.
        let s = Syn { name: "q".into(), dtype: GqDType::I8, dims: vec![1], scale: 0.0, bytes: vec![1] };
        assert!(GqFile::from_bytes(&build(&[s], 64)).is_err());
        // Empty file with zero tensors is valid.
        let e = GqFile::from_bytes(&build(&[], 64)).unwrap();
        assert!(e.is_empty());
    }

    /// Fuzz: random byte flips, truncations, and pure noise must never panic.
    #[test]
    fn fuzz_never_panics() {
        let mut r = Rng(0xDEADBEEF);
        let (good, _, _, _) = sample(&mut r);
        let mut ok = 0;
        for i in 0..4000 {
            let mut b = good.clone();
            match i % 4 {
                0 => { let k = 1 + r.next() as usize % 8; for _ in 0..k { let p = r.next() as usize % b.len(); b[p] = r.next() as u8; } }
                1 => { let p = r.next() as usize % (HEADER_LEN + 200).min(b.len()); b[p] = r.next() as u8; }
                2 => { let n = r.next() as usize % b.len(); b.truncate(n); }
                _ => { let n = r.next() as usize % 512; b = (0..n).map(|_| r.next() as u8).collect(); if n >= 8 && i % 8 == 3 { b[..8].copy_from_slice(&MAGIC); } }
            }
            if let Ok(g) = GqFile::from_bytes(&b) {
                ok += 1;
                for e in g.entries().to_vec() {
                    let _ = g.tensor(&e.name);
                    let _ = g.verify(&e.name);
                    let _ = g.f32_view(&e.name);
                }
            }
        }
        assert!(ok > 0, "some mutations (data-only flips) should still parse");
    }

    /// Random synthetic files of many shapes/dtypes round-trip exactly.
    #[test]
    fn fuzz_random_roundtrip() {
        let mut r = Rng(0xC0FFEE);
        for _ in 0..50 {
            let n = r.next() as usize % 6;
            let mut ts = Vec::new();
            let mut want = Vec::new();
            for j in 0..n {
                let rank = r.next() as usize % 4;
                let dims: Vec<usize> = (0..rank).map(|_| 1 + r.next() as usize % 9).collect();
                let (s, v) = f32_syn(&format!("t{j}"), &dims, &mut r);
                ts.push(s); want.push((dims, v));
            }
            let align = [64u32, 128, 4096][r.next() as usize % 3];
            let g = GqFile::from_bytes(&build(&ts, align)).unwrap();
            g.verify_all().unwrap();
            for (j, (dims, v)) in want.iter().enumerate() {
                let t = g.tensor(&format!("t{j}")).unwrap();
                assert_eq!(t.dims(), dims.as_slice());
                assert_eq!(&t.to_vec_f32(), v);
            }
        }
    }
}
