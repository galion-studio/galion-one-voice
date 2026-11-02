// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! W20-14 — synthetic-only `.gq` writer prototype + G5 round-trip gate sketch.
//!
//! SYNTHETIC ONLY. This file never reads a real weight, never downloads anything, and does NOT
//! mean the weights path is clear (A21 counsel / HF URIs / sha256 still open; see
//! docs/agent-wave20/W20-14-STATUS.md). The production `gq-pack` stays deferred per
//! A07-gq-format.md §6/§8 ("no writer in the runtime crate until a real weight exists"), so this
//! writer lives in an integration test only and exercises nothing but the public reader API.
//!
//! What is covered (A07 §6 design, steps 2/3/5/6/7/8/9):
//! * `SynWriter`: header placeholder → table → align pad → aligned tensor data → back-patch header;
//!   per-tensor SHA-256; deterministic order (explicit order list else lexicographic);
//!   atomic `*.gq.tmp` → fsync → rename.
//! * Minimal synthetic safetensors emitter + parser (in-tree, test-only; our own emitted subset).
//! * G5 sketch: `pack → open (mmap, Populate) → verify_all → compare every tensor to source`
//!   bit-exact for F32/BF16, within quant tolerance for I8 (|x − q·s| ≤ s/2), plus
//!   SHA256SUMS + DATA_MANIFEST-style output and negative cases (corruption must fail G5).

use galion_tensor::gq::{self, GqDType, GqError, GqFile, Prefault, HEADER_LEN, MAGIC, VERSION};
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------------------------
// Deterministic synthetic data
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 { self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17; self.0 }
    fn f32(&mut self) -> f32 { (self.next() % 200_001) as f32 / 10_000.0 - 10.0 }
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("w20-14-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------------------------------------
// Source tensors (safetensors-like interchange, synthetic)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum StDType { F32, BF16 }

#[derive(Clone, Debug, PartialEq)]
struct SrcTensor { name: String, dtype: StDType, shape: Vec<usize>, data: Vec<u8> }

impl SrcTensor {
    fn f32(name: &str, shape: &[usize], r: &mut Rng) -> Self {
        let n: usize = shape.iter().product();
        let data = (0..n).flat_map(|_| r.f32().to_le_bytes()).collect();
        SrcTensor { name: name.into(), dtype: StDType::F32, shape: shape.to_vec(), data }
    }
    fn bf16(name: &str, shape: &[usize], r: &mut Rng) -> Self {
        let n: usize = shape.iter().product();
        let data = (0..n).flat_map(|_| ((r.f32().to_bits() >> 16) as u16).to_le_bytes()).collect();
        SrcTensor { name: name.into(), dtype: StDType::BF16, shape: shape.to_vec(), data }
    }
    fn as_f32(&self) -> Vec<f32> {
        match self.dtype {
            StDType::F32 => self.data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
            StDType::BF16 => self.data.chunks_exact(2)
                .map(|c| f32::from_bits((u16::from_le_bytes(c.try_into().unwrap()) as u32) << 16)).collect(),
        }
    }
}

/// Emit a safetensors-format blob: u64 LE header len + JSON header + raw data.
fn st_emit(ts: &[SrcTensor]) -> Vec<u8> {
    let mut sorted: Vec<&SrcTensor> = ts.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut json = String::from("{\"__metadata__\":{\"format\":\"pt\",\"note\":\"synthetic\"}");
    let mut data = Vec::new();
    for t in sorted {
        let b = data.len();
        data.extend_from_slice(&t.data);
        let dt = match t.dtype { StDType::F32 => "F32", StDType::BF16 => "BF16" };
        let shape: Vec<String> = t.shape.iter().map(|d| d.to_string()).collect();
        json.push_str(&format!(",\"{}\":{{\"dtype\":\"{dt}\",\"shape\":[{}],\"data_offsets\":[{b},{}]}}",
            t.name, shape.join(","), data.len()));
    }
    json.push('}');
    while (8 + json.len()) % 8 != 0 { json.push(' '); }
    let mut out = (json.len() as u64).to_le_bytes().to_vec();
    out.extend_from_slice(json.as_bytes());
    out.extend_from_slice(&data);
    out
}

/// Tiny parser for the subset `st_emit` produces (A07 §6.1 rejection rules applied).
fn st_parse(b: &[u8]) -> Result<Vec<SrcTensor>, String> {
    const MAX_HDR: u64 = 100 << 20;
    if b.len() < 8 { return Err("short".into()); }
    let hl = u64::from_le_bytes(b[..8].try_into().unwrap());
    if hl > MAX_HDR || 8 + hl > b.len() as u64 { return Err("header len".into()); }
    let hdr = std::str::from_utf8(&b[8..8 + hl as usize]).map_err(|_| "utf8")?.trim_end();
    let data = &b[8 + hl as usize..];
    let body = hdr.strip_prefix('{').and_then(|s| s.strip_suffix('}')).ok_or("not object")?;
    // Split top-level entries on `},"` boundaries (subset emitted above has no nested strings with braces).
    let mut out = Vec::new();
    let mut spans: Vec<(u64, u64, String)> = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let rest2 = rest.strip_prefix(',').unwrap_or(rest);
        let k0 = rest2.find('"').ok_or("key")? + 1;
        let k1 = k0 + rest2[k0..].find('"').ok_or("key end")?;
        let key = &rest2[k0..k1];
        let v0 = k1 + rest2[k1..].find('{').ok_or("val")?;
        let v1 = v0 + rest2[v0..].find('}').ok_or("val end")? + 1;
        let val = &rest2[v0..v1];
        rest = &rest2[v1..];
        if key == "__metadata__" { continue; }
        let field = |f: &str| -> Result<&str, String> {
            let i = val.find(&format!("\"{f}\":")).ok_or(format!("missing {f}"))? + f.len() + 3;
            Ok(&val[i..])
        };
        let dt = field("dtype")?;
        let dtype = if dt.starts_with("\"F32\"") { StDType::F32 }
            else if dt.starts_with("\"BF16\"") { StDType::BF16 }
            else { return Err(format!("unsupported dtype for {key} (no silent casts)")); };
        let arr = |s: &str| -> Result<Vec<u64>, String> {
            let s = s.strip_prefix('[').ok_or("arr")?;
            let s = &s[..s.find(']').ok_or("arr end")?];
            if s.is_empty() { return Ok(vec![]); }
            s.split(',').map(|x| x.trim().parse::<u64>().map_err(|e| e.to_string())).collect()
        };
        let shape: Vec<usize> = arr(field("shape")?)?.into_iter().map(|x| x as usize).collect();
        let off = arr(field("data_offsets")?)?;
        if off.len() != 2 || off[0] > off[1] || off[1] > data.len() as u64 { return Err(format!("offsets {key}")); }
        let esz = match dtype { StDType::F32 => 4, StDType::BF16 => 2 };
        if (off[1] - off[0]) != shape.iter().product::<usize>() as u64 * esz { return Err(format!("size {key}")); }
        spans.push((off[0], off[1], key.to_string()));
        out.push(SrcTensor { name: key.into(), dtype, shape, data: data[off[0] as usize..off[1] as usize].to_vec() });
    }
    spans.sort();
    for w in spans.windows(2) { if w[1].0 < w[0].1 { return Err(format!("overlap {}", w[1].2)); } }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Synthetic writer (A07 §6 layout) — test-only prototype of `gq-pack`
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Quant { None, I8Sym }

struct PackOpts<'a> { align: u32, quant: Quant, keep_float: &'a [&'a str], order: Option<&'a [&'a str]> }
impl Default for PackOpts<'_> {
    fn default() -> Self { PackOpts { align: 64, quant: Quant::None, keep_float: &[], order: None } }
}

struct Packed { name: String, dtype: GqDType, dims: Vec<usize>, scale: f32, bytes: Vec<u8> }

#[derive(Debug, PartialEq)]
enum PackError { BadAlign(u32), BadName(String), Duplicate(String), OrderUnknown(String), Io(String) }

fn quant_i8_sym(v: &[f32]) -> (f32, Vec<u8>) {
    let amax = v.iter().fold(0f32, |m, x| m.max(x.abs()));
    let scale = if amax > 0.0 { amax / 127.0 } else { 1.0 }; // all-zero tensor: any positive scale
    let q = v.iter().map(|&x| (x / scale).round().clamp(-127.0, 127.0) as i8 as u8).collect();
    (scale, q)
}

fn to_packed(src: &[SrcTensor], o: &PackOpts) -> Result<Vec<Packed>, PackError> {
    let mut seen = std::collections::HashSet::new();
    let mut v = Vec::with_capacity(src.len());
    for t in src {
        if t.name.is_empty() || t.name.len() > gq::MAX_NAME_LEN { return Err(PackError::BadName(t.name.clone())); }
        if !seen.insert(t.name.clone()) { return Err(PackError::Duplicate(t.name.clone())); }
        let keep = o.keep_float.iter().any(|k| t.name.contains(k));
        let p = if o.quant == Quant::I8Sym && !keep {
            let (scale, bytes) = quant_i8_sym(&t.as_f32());
            Packed { name: t.name.clone(), dtype: GqDType::I8, dims: t.shape.clone(), scale, bytes }
        } else {
            let dtype = match t.dtype { StDType::F32 => GqDType::F32, StDType::BF16 => GqDType::BF16 };
            Packed { name: t.name.clone(), dtype, dims: t.shape.clone(), scale: 0.0, bytes: t.data.clone() }
        };
        v.push(p);
    }
    // Deterministic order: explicit first-use order (every name must be listed or it is appended
    // lexicographically after the listed ones), else lexicographic.
    match o.order {
        Some(ord) => {
            for n in ord { if !seen.contains(*n) { return Err(PackError::OrderUnknown(n.to_string())); } }
            let rank = |n: &str| ord.iter().position(|x| *x == n).unwrap_or(usize::MAX);
            v.sort_by(|a, b| rank(&a.name).cmp(&rank(&b.name)).then(a.name.cmp(&b.name)));
        }
        None => v.sort_by(|a, b| a.name.cmp(&b.name)),
    }
    Ok(v)
}

fn pack_bytes(src: &[SrcTensor], o: &PackOpts) -> Result<Vec<u8>, PackError> {
    if !o.align.is_power_of_two() || !(gq::MIN_ALIGN..=gq::MAX_ALIGN).contains(&o.align) {
        return Err(PackError::BadAlign(o.align));
    }
    let a = o.align as u64;
    let ts = to_packed(src, o)?;
    let mut table = Vec::new();
    let mut off = 0u64;
    let mut offsets = Vec::with_capacity(ts.len());
    for t in &ts {
        off = off.div_ceil(a) * a;
        offsets.push(off);
        table.extend_from_slice(&(t.name.len() as u16).to_le_bytes());
        table.extend_from_slice(t.name.as_bytes());
        table.push(t.dtype as u8);
        table.push(t.dims.len() as u8);
        table.extend_from_slice(&0u16.to_le_bytes());
        for &d in &t.dims { table.extend_from_slice(&(d as u64).to_le_bytes()); }
        table.extend_from_slice(&t.scale.to_le_bytes());
        table.extend_from_slice(&off.to_le_bytes());
        table.extend_from_slice(&(t.bytes.len() as u64).to_le_bytes());
        table.extend_from_slice(&gq::sha256(&t.bytes));
        off += t.bytes.len() as u64;
    }
    let data_len = off;
    let data_off = (HEADER_LEN as u64 + table.len() as u64).div_ceil(a) * a;
    // Header placeholder, then back-patch once sizes are known (streaming writer shape).
    let mut f = vec![0u8; HEADER_LEN];
    f.extend_from_slice(&table);
    f.resize(data_off as usize, 0);
    for (t, &o) in ts.iter().zip(&offsets) {
        f.resize((data_off + o) as usize, 0);
        f.extend_from_slice(&t.bytes);
    }
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(&MAGIC);
    h.extend_from_slice(&VERSION.to_le_bytes());
    h.extend_from_slice(&0u16.to_le_bytes());
    h.extend_from_slice(&o.align.to_le_bytes());
    h.extend_from_slice(&(ts.len() as u32).to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&(HEADER_LEN as u64).to_le_bytes());
    h.extend_from_slice(&(table.len() as u64).to_le_bytes());
    h.extend_from_slice(&data_off.to_le_bytes());
    h.extend_from_slice(&data_len.to_le_bytes());
    h.extend_from_slice(&[0u8; 8]);
    assert_eq!(h.len(), HEADER_LEN);
    f[..HEADER_LEN].copy_from_slice(&h);
    Ok(f)
}

/// Atomic write: `<out>.tmp` → fsync → rename. No partially written `.gq` is ever visible.
fn pack_to_file(src: &[SrcTensor], o: &PackOpts, out: &Path) -> Result<[u8; 32], PackError> {
    let bytes = pack_bytes(src, o)?;
    let tmp = out.with_extension("gq.tmp");
    let io = |e: std::io::Error| PackError::Io(e.to_string());
    {
        let mut fh = std::fs::File::create(&tmp).map_err(io)?;
        fh.write_all(&bytes).map_err(io)?;
        fh.sync_all().map_err(io)?;
    }
    std::fs::rename(&tmp, out).map_err(io)?;
    Ok(gq::sha256(&bytes))
}

// ---------------------------------------------------------------------------------------------
// G5 gate sketch (synthetic): pack → open → verify_all → compare to source
// ---------------------------------------------------------------------------------------------

#[derive(Debug)]
struct G5Report { tensors: usize, exact: usize, quant: usize, max_quant_err_over_half_scale: f32 }

fn g5_check(src: &[SrcTensor], g: &GqFile) -> Result<G5Report, String> {
    g.verify_all().map_err(|e| format!("verify_all: {e}"))?;
    if g.len() != src.len() { return Err(format!("count {} != {}", g.len(), src.len())); }
    let mut rep = G5Report { tensors: src.len(), exact: 0, quant: 0, max_quant_err_over_half_scale: 0.0 };
    for s in src {
        let e = g.entry(&s.name).ok_or(format!("missing {}", s.name))?;
        if e.dims != s.shape { return Err(format!("dims {}", s.name)); }
        match e.dtype {
            GqDType::F32 | GqDType::BF16 => {
                // Bit-exact on raw bytes (catches NaN payloads / -0.0 that float compare would miss).
                if g.raw(&s.name).map_err(|e| e.to_string())? != s.data.as_slice() {
                    return Err(format!("not bit-exact: {}", s.name));
                }
                rep.exact += 1;
            }
            GqDType::I8 => {
                let got = g.tensor(&s.name).map_err(|e| e.to_string())?.to_vec_f32();
                let want = s.as_f32();
                for (x, y) in want.iter().zip(&got) {
                    let r = (x - y).abs() / (e.scale * 0.5);
                    // 1e-3 slack for f32 rounding in x/scale and q*scale.
                    if r > 1.0 + 1e-3 { return Err(format!("quant tol {}: |{x}-{y}| > s/2", s.name)); }
                    rep.max_quant_err_over_half_scale = rep.max_quant_err_over_half_scale.max(r);
                }
                rep.quant += 1;
            }
        }
    }
    Ok(rep)
}

/// Full G5 sketch on disk: safetensors blob → parse → pack (atomic) → mmap Populate → check →
/// SHA256SUMS + DATA_MANIFEST.toml text.
fn g5_run(dir: &Path, src: &[SrcTensor], o: &PackOpts) -> Result<(G5Report, String, String), String> {
    let st = st_emit(src);
    std::fs::write(dir.join("synthetic.safetensors"), &st).map_err(|e| e.to_string())?;
    let parsed = st_parse(&std::fs::read(dir.join("synthetic.safetensors")).unwrap())?;
    let out = dir.join("synthetic.gq");
    let digest = pack_to_file(&parsed, o, &out).map_err(|e| format!("{e:?}"))?;
    let g = GqFile::open(&out, Prefault::Populate).map_err(|e| e.to_string())?;
    if g.file_sha256() != digest { return Err("file digest drift".into()); }
    let rep = g5_check(&parsed, &g)?;
    let sums = format!("# Galion Voice — SYNTHETIC G5 fixture, not a weight slot\n{}  synthetic.gq\n", gq::to_hex(&digest));
    gq::verify_sha256sums(dir, &sums).map_err(|e| e.to_string())?;
    let manifest = format!(
        "# SYNTHETIC — no upstream checkpoint, no licence claim\n[[source]]\nfile = \"synthetic.safetensors\"\nsha256 = \"{}\"\nlicence = \"n/a (synthetic)\"\n[[output]]\nfile = \"synthetic.gq\"\nsha256 = \"{}\"\nconverter = \"w20-14 synthetic writer (test-only)\"\n",
        gq::to_hex(&gq::sha256(&st)), gq::to_hex(&digest));
    Ok((rep, sums, manifest))
}

fn fixture(r: &mut Rng) -> Vec<SrcTensor> {
    let mut v = vec![
        SrcTensor::f32("enc.layers.0.attn.q.weight", &[8, 16], r),
        SrcTensor::f32("enc.layers.0.norm.weight", &[16], r),
        SrcTensor::bf16("dec.out.weight", &[4, 3, 5], r),
        SrcTensor::f32("embed.tokens", &[11, 6], r),
        SrcTensor::f32("scalar.gain", &[], r),
    ];
    // Edge bit patterns must survive bit-exactly.
    let mut edge = SrcTensor::f32("edge.bits", &[6], r);
    edge.data = [-0.0f32, f32::MIN_POSITIVE, f32::MAX, f32::INFINITY, f32::from_bits(0x7fc0_1234), 1e-45]
        .iter().flat_map(|x| x.to_le_bytes()).collect();
    v.push(edge);
    v
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn w20_14_safetensors_subset_roundtrip() {
    let mut r = Rng(0x5EED_0014);
    let src = fixture(&mut r);
    let mut got = st_parse(&st_emit(&src)).unwrap();
    let mut want = src.clone();
    got.sort_by(|a, b| a.name.cmp(&b.name));
    want.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(got, want);
    // Rejections: unknown dtype, out-of-range offsets, oversized header.
    let mk = |json: &str, data: &[u8]| { let mut b = (json.len() as u64).to_le_bytes().to_vec(); b.extend_from_slice(json.as_bytes()); b.extend_from_slice(data); b };
    assert!(st_parse(&mk(r#"{"x":{"dtype":"F16","shape":[1],"data_offsets":[0,2]}}"#, &[0, 0])).is_err());
    assert!(st_parse(&mk(r#"{"x":{"dtype":"F32","shape":[2],"data_offsets":[0,8]}}"#, &[0; 4])).is_err());
    assert!(st_parse(&mk(r#"{"x":{"dtype":"F32","shape":[2],"data_offsets":[0,4]}}"#, &[0; 8])).is_err());
    assert!(st_parse(&mk(r#"{"a":{"dtype":"F32","shape":[2],"data_offsets":[0,8]},"b":{"dtype":"F32","shape":[2],"data_offsets":[4,12]}}"#, &[0; 12])).is_err());
    let mut huge = (200u64 << 20).to_le_bytes().to_vec(); huge.extend_from_slice(b"{}");
    assert!(st_parse(&huge).is_err());
}

#[test]
fn w20_14_writer_output_is_valid_gq_every_alignment() {
    let mut r = Rng(1);
    let src = fixture(&mut r);
    for align in [64u32, 128, 4096, 65536] {
        let b = pack_bytes(&src, &PackOpts { align, ..Default::default() }).unwrap();
        let g = GqFile::from_bytes(&b).unwrap();
        assert_eq!(g.header().alignment, align);
        assert_eq!(g.header().data_offset % align as u64, 0);
        for e in g.entries() { assert_eq!(e.offset % align as u64, 0, "{}", e.name); }
        let rep = g5_check(&src, &g).unwrap();
        assert_eq!(rep.exact, src.len());
        // Zero-copy f32 view still works on writer output.
        let w = g.f32_view("enc.layers.0.attn.q.weight").unwrap().unwrap();
        assert_eq!(w, src[0].as_f32().as_slice());
    }
    for bad in [0u32, 32, 96, 1 << 17] {
        assert_eq!(pack_bytes(&src, &PackOpts { align: bad, ..Default::default() }).unwrap_err(), PackError::BadAlign(bad));
    }
}

#[test]
fn w20_14_writer_is_deterministic_and_order_aware() {
    let mut r = Rng(2);
    let src = fixture(&mut r);
    let mut rev = src.clone();
    rev.reverse();
    let a = pack_bytes(&src, &PackOpts::default()).unwrap();
    let b = pack_bytes(&rev, &PackOpts::default()).unwrap();
    assert_eq!(a, b, "input order must not change lexicographic output");
    assert_eq!(gq::sha256(&a), gq::sha256(&pack_bytes(&src, &PackOpts::default()).unwrap()));
    let lex: Vec<String> = GqFile::from_bytes(&a).unwrap().entries().iter().map(|e| e.name.clone()).collect();
    let mut sorted = lex.clone(); sorted.sort();
    assert_eq!(lex, sorted);
    // First-use order: listed names first in list order, rest lexicographic after.
    let ord = ["enc.layers.0.norm.weight", "enc.layers.0.attn.q.weight", "dec.out.weight"];
    let c = pack_bytes(&src, &PackOpts { order: Some(&ord), ..Default::default() }).unwrap();
    let g = GqFile::from_bytes(&c).unwrap();
    let names: Vec<&str> = g.entries().iter().map(|e| e.name.as_str()).collect();
    assert_eq!(&names[..3], &ord);
    assert_eq!(&names[3..], &["edge.bits", "embed.tokens", "scalar.gain"]);
    // Offsets are increasing in table order (sequential readahead matches first-use order).
    assert!(g.entries().windows(2).all(|w| w[0].offset < w[1].offset));
    assert_eq!(pack_bytes(&src, &PackOpts { order: Some(&["nope"]), ..Default::default() }).unwrap_err(),
        PackError::OrderUnknown("nope".into()));
}

#[test]
fn w20_14_writer_rejects_bad_inputs() {
    let mut r = Rng(3);
    let a = SrcTensor::f32("x", &[2], &mut r);
    assert_eq!(pack_bytes(&[a.clone(), a.clone()], &PackOpts::default()).unwrap_err(), PackError::Duplicate("x".into()));
    let mut e = a.clone(); e.name.clear();
    assert!(matches!(pack_bytes(&[e], &PackOpts::default()), Err(PackError::BadName(_))));
    let mut long = a.clone(); long.name = "n".repeat(gq::MAX_NAME_LEN + 1);
    assert!(matches!(pack_bytes(&[long], &PackOpts::default()), Err(PackError::BadName(_))));
    // Zero tensors is a valid (empty) file.
    let g = GqFile::from_bytes(&pack_bytes(&[], &PackOpts::default()).unwrap()).unwrap();
    assert!(g.is_empty());
}

#[test]
fn w20_14_i8_quant_within_tolerance_and_keep_float() {
    let mut r = Rng(4);
    let mut src = fixture(&mut r);
    src.retain(|t| t.name != "edge.bits"); // inf/NaN are not quantizable; real recipe keeps such tensors float
    src.push(SrcTensor { name: "zeros".into(), dtype: StDType::F32, shape: vec![3], data: vec![0; 12] });
    let o = PackOpts { quant: Quant::I8Sym, keep_float: &["norm", "embed"], ..Default::default() };
    let g = GqFile::from_bytes(&pack_bytes(&src, &o).unwrap()).unwrap();
    for e in g.entries() {
        let keep = e.name.contains("norm") || e.name.contains("embed");
        assert_eq!(e.dtype != GqDType::I8, keep, "{}", e.name);
        if e.dtype == GqDType::I8 { assert!(e.scale.is_finite() && e.scale > 0.0); }
    }
    let rep = g5_check(&src, &g).unwrap();
    assert_eq!(rep.exact, 2);
    assert_eq!(rep.quant, src.len() - 2);
    assert!(rep.max_quant_err_over_half_scale <= 1.0 + 1e-3);
}

#[test]
fn w20_14_g5_sketch_end_to_end_on_disk() {
    let dir = tmpdir("g5");
    let mut r = Rng(0x6_5);
    let src = fixture(&mut r);
    let (rep, sums, manifest) = g5_run(&dir, &src, &PackOpts { align: 4096, ..Default::default() }).unwrap();
    assert_eq!(rep.tensors, src.len());
    assert_eq!(rep.exact, src.len());
    assert!(sums.contains("synthetic.gq") && sums.starts_with("# Galion Voice — SYNTHETIC"));
    assert!(manifest.contains("SYNTHETIC") && manifest.contains("[[output]]"));
    assert!(!dir.join("synthetic.gq.tmp").exists(), "tmp file must be renamed away");
    // Every prefault mode opens the same bytes.
    for pf in [Prefault::None, Prefault::WillNeed, Prefault::Touch] {
        let g = GqFile::open(dir.join("synthetic.gq"), pf).unwrap();
        g5_check(&src, &g).unwrap();
    }
    // Re-run is byte-identical (determinism half of G5).
    let first = std::fs::read(dir.join("synthetic.gq")).unwrap();
    g5_run(&dir, &src, &PackOpts { align: 4096, ..Default::default() }).unwrap();
    assert_eq!(first, std::fs::read(dir.join("synthetic.gq")).unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn w20_14_g5_fails_on_corruption_and_source_drift() {
    let mut r = Rng(5);
    let src = fixture(&mut r);
    let good = pack_bytes(&src, &PackOpts::default()).unwrap();
    // 1) Data-byte flip → reader parses, G5 must fail at verify_all.
    let mut b = good.clone();
    let last = b.len() - 1; b[last] ^= 1;
    let g = GqFile::from_bytes(&b).unwrap();
    assert!(g5_check(&src, &g).unwrap_err().starts_with("verify_all"));
    assert!(matches!(g.verify_all(), Err(GqError::ChecksumMismatch(_))));
    // 2) Source drifted after packing → G5 must fail on bit-exact compare.
    let g = GqFile::from_bytes(&good).unwrap();
    let mut drift = src.clone(); drift[0].data[0] ^= 1;
    assert!(g5_check(&drift, &g).unwrap_err().starts_with("not bit-exact"));
    // 3) Missing tensor in pack → count mismatch.
    assert!(g5_check(&src, &GqFile::from_bytes(&pack_bytes(&src[1..], &PackOpts::default()).unwrap()).unwrap()).is_err());
    // 4) SHA256SUMS mismatch.
    let dir = tmpdir("neg");
    std::fs::write(dir.join("synthetic.gq"), &good).unwrap();
    let bad = format!("{}  synthetic.gq\n", gq::to_hex(&[0u8; 32]));
    assert!(matches!(gq::verify_sha256sums(&dir, &bad), Err(GqError::ChecksumMismatch(_))));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn w20_14_random_synthetic_roundtrip_fuzz() {
    let mut r = Rng(0xC0DE_0014);
    for i in 0..60 {
        let n = r.next() as usize % 7;
        let src: Vec<SrcTensor> = (0..n).map(|j| {
            let rank = r.next() as usize % 4;
            let dims: Vec<usize> = (0..rank).map(|_| 1 + r.next() as usize % 9).collect();
            if r.next() % 2 == 0 { SrcTensor::f32(&format!("t{j}"), &dims, &mut r) } else { SrcTensor::bf16(&format!("t{j}"), &dims, &mut r) }
        }).collect();
        let align = [64u32, 128, 256, 4096][i % 4];
        let parsed = st_parse(&st_emit(&src)).unwrap();
        let b = pack_bytes(&parsed, &PackOpts { align, ..Default::default() }).unwrap();
        let g = GqFile::from_bytes(&b).unwrap();
        let rep = g5_check(&src, &g).unwrap();
        assert_eq!(rep.exact, n);
    }
}
