// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Symphony voice clip. Chatterbox clone PCM goes in. A Galion `.gq` comes out.
//!
//! The runtime file is not a Chatterbox weight, not a WAV, and not raw s16le.
//! [`galion_tensor::GqFile`] is the reader (mmap, prefault). This module is the
//! writer. `galion-tensor` stays reader-only.
//!
//! Layout, all little-endian, alignment 64, magic `GALIONGQ` (format v1):
//!
//! | Tensor | Shape | Payload |
//! |---|---|---|
//! | `voice.clock` | `[4]` | `u32` bits: rate, frame_ms, n_samples, channels |
//! | `voice.flags` | `[4]` | `u32` bits: cpu_only=1, gpu=0, chatterbox_runtime=0, galion_clip=1 |
//! | `voice.id` | `[16]` | persona bytes, one byte per `u32`, then zeros |
//! | `voice.pcm` | `[n_frames, 1920]` | f32 mono in [-1, 1], 24 kHz, tail padded with silence |
//!
//! Integers ride in `f32::from_bits`, because `.gq` v1 has no u32 dtype and the
//! reader must not gain one for a clip. `24000` is not the float `24000.0`.
//!
//! Hot path is [`VoiceClip::fill_frame`]: one 80 ms frame, no alloc, no I/O.
//! A late frame or a frame past the end is silence. The index is not skipped.
//!
//! Load path may allocate and read disk. It is not inside `Heartbeat::step`.
//! Target hosts are CPU-only: EPYC 9655P-class, Plus CX23, Ultra CX43.

use galion_tensor::gq::{self, GqDType, GqFile, Prefault};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Phone / heartbeat rate. Same number as `galion_heartbeat::SAMPLE_RATE`.
pub const SAMPLE_RATE: u32 = 24_000;
/// Locked frame. Same number as `LOCKS.toml` `frame_ms` and `galion_heartbeat::FRAME_MS`.
pub const FRAME_MS: u32 = 80;
/// Samples in one frame at [`SAMPLE_RATE`].
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;
/// One percent of an 80 ms frame. Non-inference tick budget (CPU foundation).
pub const TICK_BUDGET: Duration = Duration::from_micros(800);
/// Longest utterance this compiler will pack (15 min at 24 kHz).
pub const MAX_SAMPLES: u32 = 15 * 60 * SAMPLE_RATE;
/// Input cap so a mis-labelled weight file cannot allocate a session's RAM.
const MAX_INPUT_BYTES: usize = 256 << 20;

/// Extension the symphony opens. Not `safetensors`.
pub const PRIMARY_RUNTIME_EXT: &str = "gq";
/// Policy pin. A test fails if this becomes true.
pub const CHATTERBOX_IS_RUNTIME: bool = false;
/// Voice crates do not carry a GGML-family container.
pub const GGML_FAMILY_IN_VOICE: bool = false;

/// Flagship app/voice CPU in the datacenter note. Capacity reference, not a claim
/// that this process measured that chip.
pub const HOST_FLAGSHIP: &str = "AMD EPYC 9655P-class (96c/192t, CPU only)";
/// Plus freemium class.
pub const HOST_PLUS: &str = "Plus CX23";
/// Ultra freemium class.
pub const HOST_ULTRA: &str = "Ultra CX43";

const ALIGN: u32 = 64;
const CLOCK: &str = "voice.clock";
const FLAGS: &str = "voice.flags";
const ID: &str = "voice.id";
const PCM: &str = "voice.pcm";
const ID_LEN: usize = 16;
const GCI_MAGIC: &[u8; 8] = b"GCLONINT";
const GCI_VERSION: u16 = 1;

/// What [`VoiceClip::fill_frame`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePull {
    /// `out` holds this frame's samples.
    Sound,
    /// Silence written. The caller keeps the same index (no catch-up).
    Paused,
}

/// A clip the heartbeat can pull frames from. Built at load, then only read.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceClip {
    pub persona: String,
    pub sample_rate: u32,
    pub frame_ms: u32,
    pub n_samples: u32,
    pub n_frames: u32,
    pcm: Vec<f32>,
}

impl VoiceClip {
    /// Valid samples, then silence that pads the last frame.
    pub fn pcm(&self) -> &[f32] {
        &self.pcm
    }

    /// Copy frame `index` into `out`.
    ///
    /// `out` must be [`FRAME_SAMPLES`] long. A short buffer, a late flag, or an
    /// index past the end writes silence and returns [`FramePull::Paused`].
    /// Nothing is allocated. A late frame does not advance `index`; the caller
    /// passes the same index again when the clock has caught up.
    pub fn fill_frame(&self, index: u32, late: bool, out: &mut [f32]) -> FramePull {
        if out.len() != FRAME_SAMPLES || late || index >= self.n_frames {
            for s in out.iter_mut() {
                *s = 0.0;
            }
            return FramePull::Paused;
        }
        let start = index as usize * FRAME_SAMPLES;
        out.copy_from_slice(&self.pcm[start..start + FRAME_SAMPLES]);
        FramePull::Sound
    }
}

/// Result of a compile that landed on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileReport {
    pub path: PathBuf,
    pub persona: String,
    pub n_samples: u32,
    pub n_frames: u32,
    pub sha256_hex: String,
    /// Stored `voice.pcm` bytes (f32), including the silent tail.
    pub pcm_bytes: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArtError {
    BadPersona(String),
    BadInput(String),
    /// Chatterbox / foreign container, or a clip that claims to be one.
    Refused(String),
    BadClip(String),
    Sidecar(String),
    Io(String),
}

impl std::fmt::Display for ArtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArtError::BadPersona(m) => write!(f, "persona: {m}"),
            ArtError::BadInput(m) => write!(f, "input: {m}"),
            ArtError::Refused(m) => write!(f, "refused: {m}"),
            ArtError::BadClip(m) => write!(f, "clip: {m}"),
            ArtError::Sidecar(m) => write!(f, "sidecar: {m}"),
            ArtError::Io(m) => write!(f, "io: {m}"),
        }
    }
}

impl std::error::Error for ArtError {}

/// `voices/compiled` under the repo, or `$GALION_VOICE_DIR` when that is set.
pub fn artifact_dir() -> PathBuf {
    match std::env::var("GALION_VOICE_DIR") {
        Ok(p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../voices/compiled"),
    }
}

/// Documented runtime path: `voices/compiled/<persona>.gq`.
pub fn artifact_path(persona: &str) -> PathBuf {
    artifact_dir().join(format!("{persona}.{PRIMARY_RUNTIME_EXT}"))
}

/// Sidecar next to a clip. Coreutils shape: `<hex>  <file name>`.
pub fn sidecar_path(clip: &Path) -> PathBuf {
    PathBuf::from(format!("{}.sha256", clip.display()))
}

/// True only for a `.gq` path, and only while Chatterbox is not the runtime.
pub fn is_primary_runtime_path(path: &Path) -> bool {
    !CHATTERBOX_IS_RUNTIME && ext(path).as_deref() == Some(PRIMARY_RUNTIME_EXT)
}

/// Compile clone PCM and write the clip plus its sha256 sidecar.
///
/// `opts.rate` and `opts.channels` apply to raw `.pcm` / `.s16le` / `.raw`.
/// WAV and `.gci` carry their own rate and channel count.
pub fn compile_file(
    input: &Path,
    out: &Path,
    persona: &str,
    rate: u32,
    channels: u16,
) -> Result<CompileReport, ArtError> {
    check_persona(persona)?;
    if ext(out).as_deref() != Some(PRIMARY_RUNTIME_EXT) {
        return Err(ArtError::Refused(format!(
            "runtime voice is .{PRIMARY_RUNTIME_EXT} only (refusing {})",
            out.display()
        )));
    }
    let bytes = read_limited(input)?;
    refuse_foreign_bytes(&bytes)?;
    let (in_rate, in_ch, pcm) = decode_input(input, &bytes, rate, channels)?;
    let packed = pack_voice(persona, in_rate, in_ch, &pcm)?;
    atomic_write(out, &packed.bytes)?;
    let digest = gq::sha256(&packed.bytes);
    let hex = gq::to_hex(&digest);
    let name = out
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ArtError::Io(format!("bad output name {}", out.display())))?;
    let side = format!("{hex}  {name}\n");
    atomic_write(&sidecar_path(out), side.as_bytes())?;
    Ok(CompileReport {
        path: out.to_path_buf(),
        persona: persona.to_string(),
        n_samples: packed.n_samples,
        n_frames: packed.n_frames,
        sha256_hex: hex,
        pcm_bytes: packed.pcm_bytes,
    })
}

/// Pack samples in memory. Tests and the file writer share this.
pub fn compile_samples(
    persona: &str,
    rate: u32,
    channels: u16,
    interleaved: &[i16],
) -> Result<Vec<u8>, ArtError> {
    check_persona(persona)?;
    Ok(pack_voice(persona, rate, channels, interleaved)?.bytes)
}

/// Galion clone intermediate (compiler input only). Magic `GCLONINT`.
/// Not a runtime path: [`open_runtime`] refuses `.gci`.
pub fn encode_intermediate(
    rate: u32,
    channels: u16,
    interleaved: &[i16],
) -> Result<Vec<u8>, ArtError> {
    if channels == 0 || interleaved.len() % channels as usize != 0 {
        return Err(ArtError::BadInput(
            "intermediate channel count does not divide the samples".into(),
        ));
    }
    let n = (interleaved.len() / channels as usize) as u32;
    let mut out = Vec::with_capacity(24 + interleaved.len() * 2);
    out.extend_from_slice(GCI_MAGIC);
    out.extend_from_slice(&GCI_VERSION.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for s in interleaved {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(out)
}

/// Open a compiled clip. Requires `.gq`, a matching sha256 sidecar, and the
/// file stem equal to the persona stored inside.
pub fn open_runtime(path: &Path) -> Result<VoiceClip, ArtError> {
    if CHATTERBOX_IS_RUNTIME {
        return Err(ArtError::Refused(
            "Chatterbox-as-runtime is not a Galion voice path".into(),
        ));
    }
    if !is_primary_runtime_path(path) {
        let kind = ext(path).unwrap_or_else(|| "(none)".into());
        return Err(ArtError::Refused(format!(
            ".{kind} is not the Galion runtime voice (want .{PRIMARY_RUNTIME_EXT}; Chatterbox containers stay off this path)"
        )));
    }
    let bytes = read_limited(path)?;
    refuse_foreign_bytes(&bytes)?;
    check_sidecar(path, &bytes)?;
    let g = GqFile::open(path, Prefault::Populate).map_err(|e| ArtError::BadClip(e.to_string()))?;
    let clip = clip_from_file(&g)?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if stem != clip.persona {
        return Err(ArtError::BadClip(format!(
            "file stem `{stem}` does not match persona `{}`",
            clip.persona
        )));
    }
    Ok(clip)
}

/// Parse bytes already checked by the caller (unit tests, in-memory round trip).
pub fn parse(bytes: &[u8]) -> Result<VoiceClip, ArtError> {
    refuse_foreign_bytes(bytes)?;
    let g = GqFile::from_bytes(bytes).map_err(|e| ArtError::BadClip(e.to_string()))?;
    clip_from_file(&g)
}

fn clip_from_file(g: &GqFile) -> Result<VoiceClip, ArtError> {
    g.verify_all()
        .map_err(|e| ArtError::BadClip(e.to_string()))?;
    if g.len() != 4 {
        return Err(ArtError::BadClip(format!(
            "voice clip has 4 tensors (clock, flags, id, pcm); found {}",
            g.len()
        )));
    }
    let clock = take_u32s(g, CLOCK, &[4])?;
    let flags = take_u32s(g, FLAGS, &[4])?;
    let id = take_u32s(g, ID, &[ID_LEN])?;
    if flags[0] != 1 {
        return Err(ArtError::Refused(
            "voice clip is CPU-only; cpu_only flag is not set".into(),
        ));
    }
    if flags[1] != 0 {
        return Err(ArtError::Refused("voice clip refuses a GPU buffer".into()));
    }
    if flags[2] != 0 {
        return Err(ArtError::Refused(
            "Chatterbox is not the runtime voice".into(),
        ));
    }
    if flags[3] != 1 {
        return Err(ArtError::BadClip("galion_clip flag is not set".into()));
    }
    let sample_rate = clock[0];
    let frame_ms = clock[1];
    let n_samples = clock[2];
    let channels = clock[3];
    if sample_rate != SAMPLE_RATE || frame_ms != FRAME_MS || channels != 1 {
        return Err(ArtError::BadClip(format!(
            "want {SAMPLE_RATE} Hz / {FRAME_MS} ms / mono, got {sample_rate} Hz / {frame_ms} ms / {channels} ch"
        )));
    }
    let persona = persona_from_id(&id)?;
    let n_frames = n_samples.div_ceil(FRAME_SAMPLES as u32);
    let pcm_entry = g
        .entry(PCM)
        .ok_or_else(|| ArtError::BadClip("missing voice.pcm".into()))?;
    if pcm_entry.dtype != GqDType::F32 || pcm_entry.dims != [n_frames as usize, FRAME_SAMPLES] {
        return Err(ArtError::BadClip(format!(
            "voice.pcm dims {:?}, want [{n_frames}, {FRAME_SAMPLES}]",
            pcm_entry.dims
        )));
    }
    let pcm = f32_le(g, PCM)?;
    if pcm.len() != n_frames as usize * FRAME_SAMPLES {
        return Err(ArtError::BadClip(
            "voice.pcm length does not match the clock".into(),
        ));
    }
    let valid = n_samples as usize;
    if pcm[valid..].iter().any(|s| *s != 0.0) {
        return Err(ArtError::BadClip(
            "tail past n_samples must be silence".into(),
        ));
    }
    Ok(VoiceClip {
        persona,
        sample_rate,
        frame_ms,
        n_samples,
        n_frames,
        pcm,
    })
}

struct Packed {
    bytes: Vec<u8>,
    n_samples: u32,
    n_frames: u32,
    pcm_bytes: u64,
}

struct Tensor {
    name: &'static str,
    dims: Vec<usize>,
    bytes: Vec<u8>,
}

fn pack_voice(
    persona: &str,
    rate: u32,
    channels: u16,
    interleaved: &[i16],
) -> Result<Packed, ArtError> {
    if rate == 0 || channels == 0 {
        return Err(ArtError::BadInput(
            "rate and channels must be non-zero".into(),
        ));
    }
    if interleaved.is_empty() || interleaved.len() % channels as usize != 0 {
        return Err(ArtError::BadInput("no whole frames of PCM".into()));
    }
    let mono = downmix(interleaved, channels);
    let mono = resample_linear(&mono, rate, SAMPLE_RATE);
    if mono.is_empty() || mono.len() > MAX_SAMPLES as usize {
        return Err(ArtError::BadInput(format!(
            "utterance must be 1 sample .. {MAX_SAMPLES} samples at {SAMPLE_RATE} Hz"
        )));
    }
    let n_samples = mono.len() as u32;
    let n_frames = n_samples.div_ceil(FRAME_SAMPLES as u32);
    let mut pcm = Vec::with_capacity(n_frames as usize * FRAME_SAMPLES);
    for s in &mono {
        pcm.push(*s as f32 / 32768.0);
    }
    pcm.resize(n_frames as usize * FRAME_SAMPLES, 0.0);
    let clock = u32s_to_f32_bytes(&[SAMPLE_RATE, FRAME_MS, n_samples, 1]);
    // cpu_only, gpu, chatterbox_runtime, galion_clip
    let flags = u32s_to_f32_bytes(&[1, 0, 0, 1]);
    let mut id_u = [0u32; ID_LEN];
    for (i, b) in persona.bytes().enumerate() {
        id_u[i] = u32::from(b);
    }
    let id = u32s_to_f32_bytes(&id_u);
    let pcm_bytes = pcm.len() as u64 * 4;
    let pcm_raw = f32s_to_bytes(&pcm);
    let tensors = [
        Tensor {
            name: CLOCK,
            dims: vec![4],
            bytes: clock,
        },
        Tensor {
            name: FLAGS,
            dims: vec![4],
            bytes: flags,
        },
        Tensor {
            name: ID,
            dims: vec![ID_LEN],
            bytes: id,
        },
        Tensor {
            name: PCM,
            dims: vec![n_frames as usize, FRAME_SAMPLES],
            bytes: pcm_raw,
        },
    ];
    let bytes = pack_gq(&tensors)?;
    Ok(Packed {
        bytes,
        n_samples,
        n_frames,
        pcm_bytes,
    })
}

/// Test-only knob so a clip that claims Chatterbox-as-runtime is rejected.
#[cfg(test)]
fn pack_with_flags(persona: &str, pcm_i16: &[i16], flags: [u32; 4]) -> Result<Vec<u8>, ArtError> {
    check_persona(persona)?;
    let n_samples = pcm_i16.len() as u32;
    let n_frames = n_samples.div_ceil(FRAME_SAMPLES as u32);
    let mut pcm = pcm_i16
        .iter()
        .map(|s| *s as f32 / 32768.0)
        .collect::<Vec<_>>();
    pcm.resize(n_frames as usize * FRAME_SAMPLES, 0.0);
    let mut id_u = [0u32; ID_LEN];
    for (i, b) in persona.bytes().enumerate() {
        id_u[i] = u32::from(b);
    }
    let tensors = [
        Tensor {
            name: CLOCK,
            dims: vec![4],
            bytes: u32s_to_f32_bytes(&[SAMPLE_RATE, FRAME_MS, n_samples, 1]),
        },
        Tensor {
            name: FLAGS,
            dims: vec![4],
            bytes: u32s_to_f32_bytes(&flags),
        },
        Tensor {
            name: ID,
            dims: vec![ID_LEN],
            bytes: u32s_to_f32_bytes(&id_u),
        },
        Tensor {
            name: PCM,
            dims: vec![n_frames as usize, FRAME_SAMPLES],
            bytes: f32s_to_bytes(&pcm),
        },
    ];
    pack_gq(&tensors)
}

fn pack_gq(tensors: &[Tensor]) -> Result<Vec<u8>, ArtError> {
    let a = ALIGN as u64;
    let mut table = Vec::new();
    let mut off = 0u64;
    let mut offsets = Vec::with_capacity(tensors.len());
    for t in tensors {
        off = off.div_ceil(a) * a;
        offsets.push(off);
        if t.name.is_empty() || t.name.len() > gq::MAX_NAME_LEN {
            return Err(ArtError::BadClip(format!("bad tensor name {}", t.name)));
        }
        let numel: u64 = t.dims.iter().map(|&d| d as u64).product();
        let len = numel
            .checked_mul(4)
            .ok_or_else(|| ArtError::BadClip("tensor too large".into()))?;
        if len != t.bytes.len() as u64 {
            return Err(ArtError::BadClip(format!(
                "{} size {} != {len}",
                t.name,
                t.bytes.len()
            )));
        }
        table.extend_from_slice(&(t.name.len() as u16).to_le_bytes());
        table.extend_from_slice(t.name.as_bytes());
        table.push(GqDType::F32 as u8);
        table.push(t.dims.len() as u8);
        table.extend_from_slice(&0u16.to_le_bytes());
        for &d in &t.dims {
            table.extend_from_slice(&(d as u64).to_le_bytes());
        }
        table.extend_from_slice(&0f32.to_le_bytes());
        table.extend_from_slice(&off.to_le_bytes());
        table.extend_from_slice(&len.to_le_bytes());
        table.extend_from_slice(&gq::sha256(&t.bytes));
        off += len;
    }
    let data_len = off;
    let data_off = (gq::HEADER_LEN as u64 + table.len() as u64).div_ceil(a) * a;
    let mut file = vec![0u8; gq::HEADER_LEN];
    file.extend_from_slice(&table);
    file.resize(data_off as usize, 0);
    for (t, &o) in tensors.iter().zip(&offsets) {
        file.resize((data_off + o) as usize, 0);
        file.extend_from_slice(&t.bytes);
    }
    let mut header = Vec::with_capacity(gq::HEADER_LEN);
    header.extend_from_slice(&gq::MAGIC);
    header.extend_from_slice(&gq::VERSION.to_le_bytes());
    header.extend_from_slice(&0u16.to_le_bytes());
    header.extend_from_slice(&ALIGN.to_le_bytes());
    header.extend_from_slice(&(tensors.len() as u32).to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    header.extend_from_slice(&(gq::HEADER_LEN as u64).to_le_bytes());
    header.extend_from_slice(&(table.len() as u64).to_le_bytes());
    header.extend_from_slice(&data_off.to_le_bytes());
    header.extend_from_slice(&data_len.to_le_bytes());
    header.extend_from_slice(&[0u8; 8]);
    if header.len() != gq::HEADER_LEN {
        return Err(ArtError::BadClip("header length drifted".into()));
    }
    file[..gq::HEADER_LEN].copy_from_slice(&header);
    Ok(file)
}

fn check_persona(persona: &str) -> Result<(), ArtError> {
    let ok = !persona.is_empty()
        && persona.len() <= 15
        && persona
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        && persona.as_bytes()[0].is_ascii_lowercase();
    if ok {
        Ok(())
    } else {
        Err(ArtError::BadPersona(format!(
            "`{persona}` — want 1..=15 of [a-z0-9_-], starting with a letter (samantha, jarvis)"
        )))
    }
}

fn persona_from_id(words: &[u32]) -> Result<String, ArtError> {
    let mut bytes = Vec::new();
    for w in words {
        if *w == 0 {
            break;
        }
        if *w > 0xff {
            return Err(ArtError::BadClip("persona byte does not fit in u8".into()));
        }
        bytes.push(*w as u8);
    }
    let s =
        String::from_utf8(bytes).map_err(|_| ArtError::BadClip("persona is not utf-8".into()))?;
    check_persona(&s)?;
    Ok(s)
}

fn take_u32s(g: &GqFile, name: &str, shape: &[usize]) -> Result<Vec<u32>, ArtError> {
    let e = g
        .entry(name)
        .ok_or_else(|| ArtError::BadClip(format!("missing {name}")))?;
    if e.dtype != GqDType::F32 || e.dims != shape {
        return Err(ArtError::BadClip(format!(
            "{name} shape {:?} dtype {:?}",
            e.dims, e.dtype
        )));
    }
    let raw = g
        .raw(name)
        .map_err(|err| ArtError::BadClip(err.to_string()))?;
    Ok(raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]).to_bits())
        .collect())
}

fn f32_le(g: &GqFile, name: &str) -> Result<Vec<f32>, ArtError> {
    let raw = g
        .raw(name)
        .map_err(|err| ArtError::BadClip(err.to_string()))?;
    Ok(raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn u32s_to_f32_bytes(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for w in words {
        out.extend_from_slice(&f32::from_bits(*w).to_le_bytes());
    }
    out
}

fn f32s_to_bytes(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 4);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn downmix(interleaved: &[i16], channels: u16) -> Vec<i16> {
    let ch = channels as usize;
    if ch == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(ch)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|&s| i32::from(s)).sum();
            (sum / ch as i32) as i16
        })
        .collect()
}

/// Linear resample. No low-pass, no noise gate, no denoise — texture stays.
fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }
    let out_len =
        ((input.len() as u64 * u64::from(to) + u64::from(from) / 2) / u64::from(from)) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let num = i as u64 * u64::from(from);
        let idx = (num / u64::from(to)) as usize;
        let frac = (num % u64::from(to)) as f64 / f64::from(to);
        let a = f64::from(input[idx.min(last)]);
        let b = f64::from(input[(idx + 1).min(last)]);
        let v = a + (b - a) * frac;
        out.push(v.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16);
    }
    out
}

fn decode_input(
    path: &Path,
    bytes: &[u8],
    rate: u32,
    channels: u16,
) -> Result<(u32, u16, Vec<i16>), ArtError> {
    match ext(path).as_deref() {
        Some("wav") => decode_wav(bytes),
        Some("gci") => decode_gci(bytes),
        Some("pcm" | "raw" | "s16le") => {
            if rate == 0 || channels == 0 {
                return Err(ArtError::BadInput(
                    "raw PCM needs a non-zero --rate and --channels".into(),
                ));
            }
            if bytes.len() % (2 * channels as usize) != 0 {
                return Err(ArtError::BadInput(
                    "raw PCM length is not a whole number of frames".into(),
                ));
            }
            Ok((rate, channels, pcm16(bytes)))
        }
        Some("gq") => Err(ArtError::Refused(
            "a .gq clip is already the runtime voice; it is not clone output".into(),
        )),
        Some(other) if refused_input_ext(other) => Err(ArtError::Refused(format!(
            ".{other} is a Chatterbox or foreign container, not clone PCM"
        ))),
        Some(other) => Err(ArtError::BadInput(format!(
            ".{other} is not a compile input (want .wav, .gci, .pcm, .s16le, .raw)"
        ))),
        None => Err(ArtError::BadInput("input path needs an extension".into())),
    }
}

fn refused_input_ext(ext: &str) -> bool {
    matches!(
        ext,
        "safetensors"
            | "pt"
            | "pth"
            | "onnx"
            | "bin"
            | "ckpt"
            | "npz"
            | "npy"
            | "ggml"
            | "mp3"
            | "flac"
    )
}

fn decode_gci(bytes: &[u8]) -> Result<(u32, u16, Vec<i16>), ArtError> {
    if bytes.len() < 24 || &bytes[..8] != GCI_MAGIC {
        return Err(ArtError::BadInput(
            "not a Galion clone intermediate (GCLONINT)".into(),
        ));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != GCI_VERSION {
        return Err(ArtError::BadInput(format!(
            "intermediate version {version}"
        )));
    }
    let channels = u16::from_le_bytes([bytes[10], bytes[11]]);
    let rate = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    let n = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    if channels == 0 || rate == 0 {
        return Err(ArtError::BadInput(
            "intermediate rate or channels is zero".into(),
        ));
    }
    let pcm = pcm16(&bytes[24..]);
    if pcm.len() != n * channels as usize {
        return Err(ArtError::BadInput(
            "intermediate sample count does not match the header".into(),
        ));
    }
    Ok((rate, channels, pcm))
}

fn decode_wav(bytes: &[u8]) -> Result<(u32, u16, Vec<i16>), ArtError> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(ArtError::BadInput("not a RIFF/WAVE file".into()));
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    loop {
        if pos + 8 > bytes.len() {
            return Err(ArtError::BadInput("wav truncated before data".into()));
        }
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        if id == b"fmt " {
            if size < 16 || body + 16 > bytes.len() {
                return Err(ArtError::BadInput("wav fmt truncated".into()));
            }
            let mut tag = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
            let channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
            let rate = u32::from_le_bytes([
                bytes[body + 4],
                bytes[body + 5],
                bytes[body + 6],
                bytes[body + 7],
            ]);
            let bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
            if tag == 0xfffe {
                if body + 26 > bytes.len() {
                    return Err(ArtError::BadInput("wav extensible fmt truncated".into()));
                }
                tag = u16::from_le_bytes([bytes[body + 24], bytes[body + 25]]);
            }
            fmt = Some((tag, channels, rate, bits));
        } else if id == b"data" {
            let (tag, channels, rate, bits) =
                fmt.ok_or_else(|| ArtError::BadInput("wav missing fmt".into()))?;
            if tag != 1 {
                return Err(ArtError::BadInput(format!(
                    "wav format tag {tag} is not integer PCM"
                )));
            }
            if bits != 16 || channels == 0 || rate == 0 {
                return Err(ArtError::BadInput(format!(
                    "wav need 16-bit PCM, got {bits}-bit x{channels} @ {rate}"
                )));
            }
            let avail = bytes.len().saturating_sub(body);
            let data_len = if size == 0 || size == u32::MAX as usize {
                avail
            } else {
                size.min(avail)
            };
            let data = &bytes[body..body + data_len];
            return Ok((rate, channels, pcm16(data)));
        }
        pos = body
            .checked_add(size + (size & 1))
            .ok_or_else(|| ArtError::BadInput("wav chunk overflow".into()))?;
    }
}

fn pcm16(data: &[u8]) -> Vec<i16> {
    data.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn read_limited(path: &Path) -> Result<Vec<u8>, ArtError> {
    let meta =
        std::fs::metadata(path).map_err(|e| ArtError::Io(format!("{}: {e}", path.display())))?;
    if meta.len() as usize > MAX_INPUT_BYTES {
        return Err(ArtError::BadInput(format!(
            "{} exceeds the {MAX_INPUT_BYTES} byte compile cap",
            path.display()
        )));
    }
    std::fs::read(path).map_err(|e| ArtError::Io(format!("{}: {e}", path.display())))
}

fn check_sidecar(path: &Path, bytes: &[u8]) -> Result<(), ArtError> {
    let side = sidecar_path(path);
    let text = std::fs::read_to_string(&side).map_err(|_| {
        ArtError::Sidecar(format!(
            "missing {} (the runtime clip is not loaded without its sha256)",
            side.display()
        ))
    })?;
    let want = text
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let got = gq::to_hex(&gq::sha256(bytes));
    if want != got {
        return Err(ArtError::Sidecar(format!(
            "sha256 mismatch for {}",
            path.display()
        )));
    }
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), ArtError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ArtError::Io(format!("{}: {e}", parent.display())))?;
        }
    }
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    let io = |e: std::io::Error| ArtError::Io(format!("{}: {e}", tmp.display()));
    {
        let mut fh = File::create(&tmp).map_err(io)?;
        fh.write_all(bytes).map_err(io)?;
        fh.sync_all().map_err(io)?;
    }
    std::fs::rename(&tmp, path).map_err(|e| {
        ArtError::Io(format!(
            "rename {} -> {}: {e}",
            tmp.display(),
            path.display()
        ))
    })?;
    Ok(())
}

fn refuse_foreign_bytes(bytes: &[u8]) -> Result<(), ArtError> {
    if is_ggml_family(bytes) {
        return Err(ArtError::Refused(
            "GGML-family container is not a Galion voice file".into(),
        ));
    }
    if looks_like_safetensors(bytes) {
        return Err(ArtError::Refused(
            "safetensors interchange is Chatterbox weight input, not the runtime voice and not clone PCM".into(),
        ));
    }
    Ok(())
}

fn is_ggml_family(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let m = [bytes[0], bytes[1], bytes[2], bytes[3]];
    m == [b'G', b'G', b'U', b'F'] || m == [b'g', b'g', b'u', b'f'] || m == *b"GGML" || m == *b"ggml"
}

fn looks_like_safetensors(bytes: &[u8]) -> bool {
    if bytes.len() < 16
        || bytes.starts_with(&gq::MAGIC)
        || bytes.starts_with(b"RIFF")
        || bytes.starts_with(GCI_MAGIC)
    {
        return false;
    }
    let n = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    if n < 2 || n > 8 << 20 || 8 + n > bytes.len() {
        return false;
    }
    let hdr = &bytes[8..8 + n];
    hdr.first() == Some(&b'{') && hdr.windows(7).any(|w| w == b"\"dtype\"")
}

fn ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn tone(n: usize) -> Vec<i16> {
        let mut v = Vec::with_capacity(n);
        v.push(i16::MIN);
        v.push(i16::MAX);
        v.push(0);
        v.push(20_000);
        v.push(-20_000);
        while v.len() < n {
            let i = v.len() as i16;
            v.push(i.wrapping_mul(13).wrapping_sub(500));
        }
        v.truncate(n);
        v
    }

    fn back(s: f32) -> i16 {
        (s * 32768.0).round() as i16
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("galion-clip-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let block = u32::from(channels) * 2;
        let mut o = Vec::new();
        o.extend_from_slice(b"RIFF");
        o.extend_from_slice(&(36 + data_len).to_le_bytes());
        o.extend_from_slice(b"WAVE");
        o.extend_from_slice(b"fmt ");
        o.extend_from_slice(&16u32.to_le_bytes());
        o.extend_from_slice(&1u16.to_le_bytes());
        o.extend_from_slice(&channels.to_le_bytes());
        o.extend_from_slice(&rate.to_le_bytes());
        o.extend_from_slice(&(rate * block).to_le_bytes());
        o.extend_from_slice(&(block as u16).to_le_bytes());
        o.extend_from_slice(&16u16.to_le_bytes());
        o.extend_from_slice(b"data");
        o.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            o.extend_from_slice(&s.to_le_bytes());
        }
        o
    }

    fn fake_safetensors() -> Vec<u8> {
        let json = br#"{"w":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut b = (json.len() as u64).to_le_bytes().to_vec();
        b.extend_from_slice(json);
        b.extend_from_slice(&1f32.to_le_bytes());
        b
    }

    #[test]
    fn frame_matches_the_heartbeat() {
        assert_eq!(SAMPLE_RATE, galion_heartbeat::SAMPLE_RATE);
        assert_eq!(FRAME_MS, galion_heartbeat::FRAME_MS);
        assert_eq!(FRAME_SAMPLES, galion_heartbeat::FRAME_SAMPLES);
        assert_eq!(FRAME_SAMPLES, 1_920);
        assert!(!CHATTERBOX_IS_RUNTIME);
        assert!(!GGML_FAMILY_IN_VOICE);
        assert_eq!(PRIMARY_RUNTIME_EXT, "gq");
        assert_ne!(PRIMARY_RUNTIME_EXT, "safetensors");
    }

    #[test]
    fn clock_words_are_bit_patterns_not_decimal_floats() {
        assert_ne!(f32::from_bits(SAMPLE_RATE), SAMPLE_RATE as f32);
        assert_ne!(f32::from_bits(FRAME_MS), FRAME_MS as f32);
    }

    #[test]
    fn round_trip_is_bit_exact_and_not_raw_pcm() {
        let src = tone(FRAME_SAMPLES + 10);
        let bytes = compile_samples("samantha", SAMPLE_RATE, 1, &src).unwrap();
        assert!(bytes.starts_with(&gq::MAGIC));
        assert!(!bytes.starts_with(b"RIFF"));
        assert_ne!(
            &bytes[..src.len().min(bytes.len())],
            &src.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<_>>()[..]
        );
        let again = compile_samples("samantha", SAMPLE_RATE, 1, &src).unwrap();
        assert_eq!(bytes, again);
        let clip = parse(&bytes).unwrap();
        assert_eq!(clip.persona, "samantha");
        assert_eq!(clip.n_samples, src.len() as u32);
        assert_eq!(clip.n_frames, 2);
        assert_eq!(clip.pcm().len(), 2 * FRAME_SAMPLES);
        for (i, s) in src.iter().enumerate() {
            assert_eq!(back(clip.pcm()[i]), *s, "sample {i}");
        }
        assert!(clip.pcm()[src.len()..].iter().all(|s| *s == 0.0));
        let g = GqFile::from_bytes(&bytes).unwrap();
        g.verify_all().unwrap();
        assert_eq!(g.len(), 4);
        assert!(g.entry(PCM).is_some());
    }

    #[test]
    fn stereo_texture_is_mixed_not_stripped() {
        let stereo = vec![0, 20_000, 0, -20_000];
        let bytes = compile_samples("jarvis", SAMPLE_RATE, 2, &stereo).unwrap();
        let clip = parse(&bytes).unwrap();
        assert_eq!(clip.persona, "jarvis");
        assert_eq!(back(clip.pcm()[0]), 10_000);
        assert_eq!(back(clip.pcm()[1]), -10_000);
    }

    #[test]
    fn resample_keeps_a_click() {
        let mut src = vec![0i16; 480];
        src[10] = 30_000;
        let bytes = compile_samples("jarvis", 48_000, 1, &src).unwrap();
        let clip = parse(&bytes).unwrap();
        assert_eq!(clip.n_samples, 240);
        assert!(
            clip.pcm().iter().any(|s| s.abs() > 0.1),
            "impulse was stripped"
        );
    }

    #[test]
    fn late_frame_pauses_without_skipping() {
        let src = tone(FRAME_SAMPLES);
        let clip = parse(&compile_samples("samantha", SAMPLE_RATE, 1, &src).unwrap()).unwrap();
        let mut out = [1f32; FRAME_SAMPLES];
        assert_eq!(clip.fill_frame(0, true, &mut out), FramePull::Paused);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(clip.fill_frame(0, false, &mut out), FramePull::Sound);
        assert_eq!(back(out[3]), src[3]);
        assert_eq!(clip.fill_frame(1, false, &mut out), FramePull::Paused);
        assert_eq!(clip.fill_frame(0, false, &mut [0f32; 4]), FramePull::Paused);
    }

    #[test]
    fn chatterbox_containers_are_not_the_runtime() {
        for name in [
            "t3_nano_v1.safetensors",
            "voice.pt",
            "voice.pth",
            "voice.onnx",
            "voice.ckpt",
            "voice.bin",
            "out.wav",
            "out.pcm",
            "out.s16le",
            "out.gci",
            "model.ggml",
            "weights.npy",
        ] {
            let path = Path::new(name);
            assert!(!is_primary_runtime_path(path), "{name}");
            assert!(
                matches!(open_runtime(path), Err(ArtError::Refused(_))),
                "{name}"
            );
        }
        assert!(is_primary_runtime_path(Path::new(
            "voices/compiled/samantha.gq"
        )));
    }

    #[test]
    fn renamed_safetensors_and_ggml_family_fail_inside_gq() {
        let d = tmp("foreign");
        let st = d.join("samantha.gq");
        let body = fake_safetensors();
        std::fs::write(&st, &body).unwrap();
        std::fs::write(
            sidecar_path(&st),
            format!("{}  samantha.gq\n", gq::to_hex(&gq::sha256(&body))),
        )
        .unwrap();
        assert!(matches!(open_runtime(&st), Err(ArtError::Refused(_))));

        let gg = d.join("jarvis.gq");
        let mut magic = vec![b'G', b'G', b'U', b'F'];
        magic.extend_from_slice(&[0u8; 64]);
        std::fs::write(&gg, &magic).unwrap();
        assert!(matches!(open_runtime(&gg), Err(ArtError::Refused(_))));
    }

    #[test]
    fn chatterbox_runtime_flag_is_refused() {
        let pcm = tone(8);
        let bad = pack_with_flags("samantha", &pcm, [1, 0, 1, 1]).unwrap();
        assert!(matches!(parse(&bad), Err(ArtError::Refused(m)) if m.contains("Chatterbox")));
        let gpu = pack_with_flags("samantha", &pcm, [1, 1, 0, 1]).unwrap();
        assert!(matches!(parse(&gpu), Err(ArtError::Refused(m)) if m.contains("GPU")));
        let cpu = pack_with_flags("samantha", &pcm, [0, 0, 0, 1]).unwrap();
        assert!(matches!(parse(&cpu), Err(ArtError::Refused(_))));
    }

    #[test]
    fn disk_round_trip_checks_sidecar_and_stem() {
        let d = tmp("disk");
        let wav_path = d.join("clone.wav");
        let src = tone(100);
        std::fs::write(&wav_path, wav(SAMPLE_RATE, 1, &src)).unwrap();
        let out = d.join("samantha.gq");
        let rep = compile_file(&wav_path, &out, "samantha", 0, 0).unwrap();
        assert!(rep.sha256_hex.len() == 64);
        assert!(!out.with_extension("tmp").exists());
        assert!(!PathBuf::from(format!("{}.tmp", out.display())).exists());
        let clip = open_runtime(&out).unwrap();
        assert_eq!(clip.n_samples, 100);
        assert_eq!(back(clip.pcm()[1]), src[1]);
        let session_floor = (0.65 * 1024.0 * 1024.0 * 1024.0) as u64;
        assert!(rep.pcm_bytes < session_floor);

        std::fs::remove_file(sidecar_path(&out)).unwrap();
        assert!(matches!(open_runtime(&out), Err(ArtError::Sidecar(_))));
        std::fs::write(
            sidecar_path(&out),
            format!("{}  samantha.gq\n", "ab".repeat(32)),
        )
        .unwrap();
        assert!(matches!(open_runtime(&out), Err(ArtError::Sidecar(_))));
    }

    #[test]
    fn stem_must_match_the_persona_inside() {
        let d = tmp("stem");
        let src = tone(4);
        let bytes = compile_samples("samantha", SAMPLE_RATE, 1, &src).unwrap();
        let out = d.join("jarvis.gq");
        std::fs::write(&out, &bytes).unwrap();
        std::fs::write(
            sidecar_path(&out),
            format!("{}  jarvis.gq\n", gq::to_hex(&gq::sha256(&bytes))),
        )
        .unwrap();
        assert!(matches!(open_runtime(&out), Err(ArtError::BadClip(_))));
    }

    #[test]
    fn compiler_refuses_to_emit_or_eat_chatterbox_files() {
        let d = tmp("refuse");
        let st = d.join("t3_nano_v1.safetensors");
        std::fs::write(&st, fake_safetensors()).unwrap();
        let err =
            compile_file(&st, &d.join("samantha.gq"), "samantha", SAMPLE_RATE, 1).unwrap_err();
        assert!(matches!(err, ArtError::Refused(_)), "{err}");
        let pcm = d.join("in.pcm");
        std::fs::write(&pcm, 1i16.to_le_bytes()).unwrap();
        let err = compile_file(
            &pcm,
            &d.join("voice.safetensors"),
            "samantha",
            SAMPLE_RATE,
            1,
        )
        .unwrap_err();
        assert!(matches!(err, ArtError::Refused(_)), "{err}");
        let err = compile_file(&pcm, &d.join("voice.wav"), "jarvis", SAMPLE_RATE, 1).unwrap_err();
        assert!(matches!(err, ArtError::Refused(_)));
    }

    #[test]
    fn intermediate_and_wav_are_inputs_only() {
        let d = tmp("gci");
        let src = tone(32);
        let gci = encode_intermediate(SAMPLE_RATE, 1, &src).unwrap();
        assert!(gci.starts_with(b"GCLONINT"));
        let gci_path = d.join("clone.gci");
        std::fs::write(&gci_path, &gci).unwrap();
        let out = d.join("samantha.gq");
        compile_file(&gci_path, &out, "samantha", 0, 0).unwrap();
        let clip = open_runtime(&out).unwrap();
        assert_eq!(back(clip.pcm()[4]), src[4]);
        assert!(matches!(open_runtime(&gci_path), Err(ArtError::Refused(_))));

        let wav_path = d.join("clone.WAV");
        std::fs::write(&wav_path, wav(SAMPLE_RATE, 1, &src)).unwrap();
        let out2 = d.join("jarvis.gq");
        compile_file(&wav_path, &out2, "jarvis", 0, 0).unwrap();
        assert_eq!(open_runtime(&out2).unwrap().persona, "jarvis");
    }

    #[test]
    fn bad_persona_and_empty_pcm_fail() {
        assert!(compile_samples("Samantha", SAMPLE_RATE, 1, &[1]).is_err());
        assert!(compile_samples("", SAMPLE_RATE, 1, &[1]).is_err());
        assert!(compile_samples("samantha", SAMPLE_RATE, 1, &[]).is_err());
        assert!(compile_samples("a jarvis", SAMPLE_RATE, 1, &[1]).is_err());
    }

    #[test]
    fn documented_directory_ends_at_voices_compiled() {
        if std::env::var("GALION_VOICE_DIR")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .is_none()
        {
            assert!(artifact_path("samantha").ends_with("voices/compiled/samantha.gq"));
            assert!(artifact_path("jarvis").ends_with("voices/compiled/jarvis.gq"));
        }
    }

    #[test]
    fn a_weight_pack_is_not_a_voice_clip() {
        let mut bytes = compile_samples("samantha", SAMPLE_RATE, 1, &tone(8)).unwrap();
        bytes[16] = 1; // n_tensors
        assert!(parse(&bytes).is_err());
    }
}
