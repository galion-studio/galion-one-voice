// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Compiler: Chatterbox worker PCM → a Galion `.gqvoice` packet.
//!
//! The worker's stdout is raw s16le. That buffer is not the runtime format.
//! [`compile`] builds a packet the speech stack can load. [`handoff_at`] writes
//! it under `voices/gq/<persona>.gqvoice` and the caller plays the file it
//! just read back. Samples are copied verbatim (Jarvis texture stays).
//! Sectors are one heartbeat: 80 ms, 1920 samples, 24 kHz.
//!
//! PyTorch stays in the worker process. This module does not link a foreign
//! runtime and does not read a reference clip.

use super::sha256::Sha256;
use crate::{Audio, VoiceError};
use std::io::Write;
use std::path::{Path, PathBuf};

/// File magic. Not `RIFF`, and not the `.gq` weight magic `GALIONGQ`.
pub const MAGIC: [u8; 8] = *b"GQVOICE\0";
pub const VERSION: u16 = 1;
pub const RATE_HZ: u32 = 24_000;
/// Same clock as `LOCKS.toml` `[clock] frame_ms` and `galion-heartbeat`.
pub const SECTOR_MS: u32 = 80;
pub const SECTOR_SAMPLES: u32 = RATE_HZ * SECTOR_MS / 1000;
pub const HEADER_LEN: usize = 64;
/// Samples were copied. Nothing was stripped.
const FLAG_VERBATIM: u32 = 1;
/// One utterance, not an archive.
const MAX_SAMPLES: usize = (RATE_HZ as usize) * 120;

const PERSONA_SAMANTHA: u8 = 1;
const PERSONA_JARVIS: u8 = 2;
const MODEL_NANO: u8 = 1;
const MODEL_TURBO: u8 = 2;

/// Worker PCM after it has been proven to be a `.gqvoice` packet.
pub struct Accepted {
    pub audio: Audio,
    pub playable: Vec<i16>,
}

/// One landed voice packet. Playback uses [`playable`](Self::playable), which
/// omits the zero pad on the last sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GqVoice {
    bytes: Vec<u8>,
    persona: u8,
    model: u8,
    sample_count: u32,
    sector_count: u32,
}

impl GqVoice {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn persona_name(&self) -> &'static str {
        persona_slug(self.persona).expect("validated persona")
    }
    pub fn model_name(&self) -> &'static str {
        model_slug(self.model).expect("validated model")
    }
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }
    pub fn sector_count(&self) -> u32 {
        self.sector_count
    }

    /// One heartbeat frame. The last frame is zero-padded out to
    /// [`SECTOR_SAMPLES`]. Samples past [`sample_count`](Self::sample_count)
    /// are pad, not voice.
    pub fn sector(&self, index: usize) -> Option<Vec<i16>> {
        if index >= self.sector_count as usize {
            return None;
        }
        let n = SECTOR_SAMPLES as usize;
        let start = HEADER_LEN + index * n * 2;
        Some(decode_i16(&self.bytes[start..start + n * 2]))
    }

    pub fn playable(&self) -> Vec<i16> {
        let n = self.sample_count as usize * 2;
        decode_i16(&self.bytes[HEADER_LEN..HEADER_LEN + n])
    }

    pub fn to_audio(&self) -> Audio {
        Audio {
            sample_rate: RATE_HZ,
            samples: self
                .playable()
                .into_iter()
                .map(|s| s as f32 / 32768.0)
                .collect(),
        }
    }

    pub fn open(path: &Path) -> Result<Self, VoiceError> {
        let bytes =
            std::fs::read(path).map_err(|e| VoiceError::Backend(format!("gqvoice: {e}")))?;
        Self::open_bytes(&bytes)
    }

    /// Reject raw worker stdout, RIFF/WAVE, and `.gq` weights.
    pub fn open_bytes(bytes: &[u8]) -> Result<Self, VoiceError> {
        if bytes.starts_with(b"RIFF") {
            return Err(VoiceError::Backend(
                "gqvoice: RIFF/WAVE is not the runtime format".into(),
            ));
        }
        if bytes.len() >= 8 && &bytes[..8] == b"GALIONGQ" {
            return Err(VoiceError::Backend(
                "gqvoice: a .gq weight is not a voice packet".into(),
            ));
        }
        if bytes.len() < HEADER_LEN || &bytes[..8] != MAGIC {
            return Err(VoiceError::Backend(
                "gqvoice: raw chatterbox pcm is not the runtime format".into(),
            ));
        }
        let version = get_u16(bytes, 8);
        if version != VERSION {
            return Err(VoiceError::Backend(format!("gqvoice: version {version}")));
        }
        let persona = bytes[10];
        let model = bytes[11];
        if persona_slug(persona).is_none() {
            return Err(VoiceError::Backend("gqvoice: unknown persona".into()));
        }
        if model_slug(model).is_none() {
            return Err(VoiceError::Backend("gqvoice: unknown model".into()));
        }
        let rate = get_u32(bytes, 12);
        let sector_samples = get_u32(bytes, 16);
        let sample_count = get_u32(bytes, 20);
        let sector_count = get_u32(bytes, 24);
        let flags = get_u32(bytes, 28);
        if rate != RATE_HZ || sector_samples != SECTOR_SAMPLES {
            return Err(VoiceError::Backend(
                "gqvoice: rate or sector size is not the heartbeat".into(),
            ));
        }
        if flags != FLAG_VERBATIM {
            return Err(VoiceError::Backend(
                "gqvoice: samples must be stored verbatim".into(),
            ));
        }
        if sample_count == 0 || sample_count as usize > MAX_SAMPLES {
            return Err(VoiceError::Backend("gqvoice: bad sample count".into()));
        }
        let expect = (sample_count as usize).div_ceil(SECTOR_SAMPLES as usize) as u32;
        if sector_count != expect {
            return Err(VoiceError::Backend(
                "gqvoice: sector count does not match".into(),
            ));
        }
        let payload_bytes = sector_count as usize * SECTOR_SAMPLES as usize * 2;
        if bytes.len() != HEADER_LEN + payload_bytes {
            return Err(VoiceError::Backend("gqvoice: truncated packet".into()));
        }
        let mut hasher = Sha256::new();
        hasher.update(&bytes[HEADER_LEN..]);
        if hasher.digest().as_slice() != &bytes[32..64] {
            return Err(VoiceError::Backend("gqvoice: sha256 mismatch".into()));
        }
        let playable_bytes = sample_count as usize * 2;
        if bytes[HEADER_LEN + playable_bytes..].iter().any(|&b| b != 0) {
            return Err(VoiceError::Backend(
                "gqvoice: sector pad is not silence".into(),
            ));
        }
        Ok(Self {
            bytes: bytes.to_vec(),
            persona,
            model,
            sample_count,
            sector_count,
        })
    }
}

/// Worker stdout is not a file the stack is allowed to play.
pub const CHATTERBOX_IS_RUNTIME: bool = false;

/// Load a landed packet. The suffix must be `.gqvoice`.
/// Raw captures (`.s16le`, `.pcm`), WAV, and `.gq` weights are refused.
pub fn open_runtime(path: &Path) -> Result<GqVoice, VoiceError> {
    if CHATTERBOX_IS_RUNTIME {
        return Err(VoiceError::Backend(
            "gqvoice: chatterbox bytes are not the runtime format".into(),
        ));
    }
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    if ext != "gqvoice" {
        return Err(VoiceError::Backend(format!(
            "gqvoice: .{ext} is not the runtime format"
        )));
    }
    GqVoice::open(path)
}

/// Directory the stack loads. `GALION_GQVOICE` overrides it.
pub fn gqvoice_dir() -> PathBuf {
    if let Ok(p) = std::env::var("GALION_GQVOICE") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../voices/gq")
}

/// `dir/samantha.gqvoice` or `dir/jarvis.gqvoice`. Unknown names land as Samantha.
pub fn asset_path(dir: &Path, persona: &str) -> PathBuf {
    let name = if super::clone_worker::persona_name(persona) == "jarvis" {
        "jarvis"
    } else {
        "samantha"
    };
    dir.join(format!("{name}.gqvoice"))
}

/// Build a packet. Empty input is an error. Samples are not filtered.
pub fn compile(samples: &[i16], persona: &str, model: &str) -> Result<GqVoice, VoiceError> {
    if samples.is_empty() {
        return Err(VoiceError::EmptyInput);
    }
    if samples.len() > MAX_SAMPLES {
        return Err(VoiceError::Backend(
            "gqvoice: utterance longer than 120s".into(),
        ));
    }
    let model_id = parse_model(model)?;
    let sector_count = samples.len().div_ceil(SECTOR_SAMPLES as usize);
    let mut payload = Vec::with_capacity(sector_count * SECTOR_SAMPLES as usize * 2);
    for s in samples {
        payload.extend_from_slice(&s.to_le_bytes());
    }
    payload.resize(sector_count * SECTOR_SAMPLES as usize * 2, 0);
    let mut hasher = Sha256::new();
    hasher.update(&payload);
    let digest = hasher.digest();
    let mut bytes = vec![0u8; HEADER_LEN + payload.len()];
    bytes[..8].copy_from_slice(&MAGIC);
    put_u16(&mut bytes, 8, VERSION);
    bytes[10] = parse_persona(persona);
    bytes[11] = model_id;
    put_u32(&mut bytes, 12, RATE_HZ);
    put_u32(&mut bytes, 16, SECTOR_SAMPLES);
    put_u32(&mut bytes, 20, samples.len() as u32);
    put_u32(&mut bytes, 24, sector_count as u32);
    put_u32(&mut bytes, 28, FLAG_VERBATIM);
    bytes[32..64].copy_from_slice(&digest);
    bytes[HEADER_LEN..].copy_from_slice(&payload);
    GqVoice::open_bytes(&bytes)
}

/// Prove `samples` are inside a packet, and return that packet's audio.
/// Does not touch disk. The returned buffer is not the worker stdout.
pub fn accept_worker_pcm(
    samples: &[i16],
    persona: &str,
    model: &str,
) -> Result<Accepted, VoiceError> {
    let voice = compile(samples, persona, model)?;
    guard_not_chatterbox(&voice, samples)?;
    let playable = voice.playable();
    let audio = voice.to_audio();
    Ok(Accepted { audio, playable })
}

/// Compile, write `dir/<persona>.gqvoice`, read it back, return that audio.
pub fn handoff_at(
    samples: &[i16],
    persona: &str,
    model: &str,
    dir: &Path,
) -> Result<Audio, VoiceError> {
    let voice = compile(samples, persona, model)?;
    guard_not_chatterbox(&voice, samples)?;
    let path = land(&voice, dir)?;
    let loaded = open_runtime(&path)?;
    if loaded.as_bytes() != voice.as_bytes() {
        return Err(VoiceError::Backend(
            "gqvoice: landed bytes differ from the packet".into(),
        ));
    }
    Ok(loaded.to_audio())
}

/// Atomic replace: `.<persona>.gqvoice.tmp` → `<persona>.gqvoice`.
pub fn land(voice: &GqVoice, dir: &Path) -> Result<PathBuf, VoiceError> {
    std::fs::create_dir_all(dir).map_err(|e| VoiceError::Backend(format!("gqvoice: {e}")))?;
    let path = asset_path(dir, voice.persona_name());
    let tmp = dir.join(format!(".{}.gqvoice.tmp", voice.persona_name()));
    let write = (|| -> Result<(), VoiceError> {
        let mut f = std::fs::File::create(&tmp)
            .map_err(|e| VoiceError::Backend(format!("gqvoice: {e}")))?;
        f.write_all(voice.as_bytes())
            .map_err(|e| VoiceError::Backend(format!("gqvoice: {e}")))?;
        f.sync_all()
            .map_err(|e| VoiceError::Backend(format!("gqvoice: {e}")))?;
        Ok(())
    })();
    if let Err(e) = write {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(VoiceError::Backend(format!("gqvoice: {e}")));
    }
    Ok(path)
}

fn guard_not_chatterbox(voice: &GqVoice, samples: &[i16]) -> Result<(), VoiceError> {
    let raw = encode_i16(samples);
    if voice.as_bytes() == raw.as_slice() || voice.as_bytes().starts_with(b"RIFF") {
        return Err(VoiceError::Backend(
            "gqvoice: compiler kept the chatterbox bytes".into(),
        ));
    }
    if GqVoice::open_bytes(&raw).is_ok() {
        return Err(VoiceError::Backend(
            "gqvoice: loader accepted raw chatterbox pcm".into(),
        ));
    }
    if voice.playable() != samples {
        return Err(VoiceError::Backend(
            "gqvoice: packet samples differ from the worker pcm".into(),
        ));
    }
    Ok(())
}

fn parse_persona(s: &str) -> u8 {
    if super::clone_worker::persona_name(s) == "jarvis" {
        PERSONA_JARVIS
    } else {
        PERSONA_SAMANTHA
    }
}

fn parse_model(s: &str) -> Result<u8, VoiceError> {
    match s.trim().to_ascii_lowercase().as_str() {
        "nano" => Ok(MODEL_NANO),
        "turbo" => Ok(MODEL_TURBO),
        _ => Err(VoiceError::Backend(
            "gqvoice: model must be nano or turbo".into(),
        )),
    }
}

fn persona_slug(id: u8) -> Option<&'static str> {
    match id {
        PERSONA_SAMANTHA => Some("samantha"),
        PERSONA_JARVIS => Some("jarvis"),
        _ => None,
    }
}

fn model_slug(id: u8) -> Option<&'static str> {
    match id {
        MODEL_NANO => Some("nano"),
        MODEL_TURBO => Some("turbo"),
        _ => None,
    }
}

fn encode_i16(samples: &[i16]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        raw.extend_from_slice(&s.to_le_bytes());
    }
    raw
}

fn decode_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

fn put_u16(buf: &mut [u8], at: usize, v: u16) {
    buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn get_u16(buf: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([buf[at], buf[at + 1]])
}
fn get_u32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("galion-gqvoice-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn impulse() -> Vec<i16> {
        let mut s = vec![0i16; 100];
        s[0] = 32767;
        s[10] = -32768;
        s[40] = 20000;
        s[41] = 20000;
        s[42] = -18000;
        s.extend_from_slice(&[50i16; 1900]);
        s
    }

    #[test]
    fn chatterbox_suffix_is_refused_as_runtime() {
        assert!(!CHATTERBOX_IS_RUNTIME);
        let dir = tmp("refuse");
        let samples = vec![1i16, 2, 3, 4];
        let raw_path = dir.join("samantha.s16le");
        std::fs::write(&raw_path, encode_i16(&samples)).unwrap();
        let err = open_runtime(&raw_path).unwrap_err();
        assert!(err.to_string().contains("not the runtime format"), "{err}");
        std::fs::write(dir.join("samantha.wav"), b"RIFF\0\0\0\0WAVE").unwrap();
        assert!(open_runtime(&dir.join("samantha.wav")).is_err());
        std::fs::write(dir.join("samantha.gq"), b"GALIONGQ").unwrap();
        assert!(open_runtime(&dir.join("samantha.gq")).is_err());
        handoff_at(&samples, "samantha", "nano", &dir).unwrap();
        let loaded = open_runtime(&dir.join("samantha.gqvoice")).unwrap();
        assert_eq!(loaded.playable(), samples);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_chatterbox_stdout_is_not_the_runtime_format() {
        let samples: Vec<i16> = (0..400).map(|i| (i * 17 - 80) as i16).collect();
        let raw = encode_i16(&samples);
        let err = GqVoice::open_bytes(&raw).unwrap_err();
        assert!(err.to_string().contains("raw chatterbox"), "{err}");
        let voice = compile(&samples, "samantha", "nano").unwrap();
        assert!(voice.as_bytes().starts_with(&MAGIC));
        assert_ne!(voice.as_bytes(), raw.as_slice());
        assert!(!voice.as_bytes().starts_with(b"RIFF"));
        assert!(!voice.as_bytes().windows(8).any(|w| w == b"GALIONGQ"));
        assert_eq!(voice.playable(), samples);
        let accepted = accept_worker_pcm(&samples, "Samantha", "NANO").unwrap();
        assert_eq!(accepted.playable, samples);
        assert_eq!(accepted.audio.sample_rate, RATE_HZ);
        assert_eq!(accepted.audio.samples.len(), samples.len());
    }

    #[test]
    fn wav_and_weight_magic_are_refused() {
        let wav = GqVoice::open_bytes(b"RIFF\x24\x00\x00\x00WAVEfmt ").unwrap_err();
        assert!(wav.to_string().contains("RIFF"), "{wav}");
        let mut weight = b"GALIONGQ".to_vec();
        weight.extend_from_slice(&[0u8; 64]);
        let err = GqVoice::open_bytes(&weight).unwrap_err();
        assert!(err.to_string().contains(".gq weight"), "{err}");
    }

    #[test]
    fn sectors_match_the_heartbeat_and_pad_is_not_played() {
        let locks = include_str!("../../../../LOCKS.toml");
        assert!(
            locks.contains("frame_ms = 80"),
            "sector clock is the lock, not a second clock"
        );
        let heartbeat = include_str!("../../../galion-heartbeat/src/lib.rs");
        assert!(heartbeat.contains("pub const FRAME_MS: u32 = 80;"));
        assert!(heartbeat.contains("pub const SAMPLE_RATE: u32 = 24_000;"));
        assert_eq!(SECTOR_MS, 80);
        assert_eq!(SECTOR_SAMPLES, 1920);
        assert_eq!(RATE_HZ * SECTOR_MS / 1000, SECTOR_SAMPLES);
        let exact = vec![7i16; SECTOR_SAMPLES as usize];
        let one = compile(&exact, "samantha", "nano").unwrap();
        assert_eq!(one.sector_count(), 1);
        assert_eq!(one.sample_count(), SECTOR_SAMPLES);
        assert_eq!(
            one.as_bytes().len(),
            HEADER_LEN + SECTOR_SAMPLES as usize * 2
        );
        assert_eq!(one.sector(0).unwrap(), exact);
        assert!(one.sector(1).is_none());
        let mut long = vec![1i16; SECTOR_SAMPLES as usize + 1];
        long[SECTOR_SAMPLES as usize] = 9;
        let two = compile(&long, "jarvis", "turbo").unwrap();
        assert_eq!(two.sector_count(), 2);
        assert_eq!(two.playable().len(), long.len());
        assert_eq!(two.to_audio().samples.len(), long.len());
        let last = two.sector(1).unwrap();
        assert_eq!(last.len(), SECTOR_SAMPLES as usize);
        assert_eq!(last[0], 9);
        assert!(last[1..].iter().all(|&s| s == 0));
        assert_eq!(
            &two.as_bytes()[HEADER_LEN..HEADER_LEN + 2],
            &1i16.to_le_bytes()
        );
    }

    #[test]
    fn jarvis_texture_clicks_survive_verbatim() {
        let samples = impulse();
        let voice = compile(&samples, "jarvis", "turbo").unwrap();
        assert_eq!(voice.persona_name(), "jarvis");
        assert_eq!(voice.model_name(), "turbo");
        assert_eq!(voice.playable(), samples);
        assert_eq!(voice.playable()[0], 32767);
        assert_eq!(voice.playable()[10], -32768);
        assert_eq!(&voice.playable()[40..43], &[20000, 20000, -18000]);
        let dir = tmp("texture");
        let audio = handoff_at(&samples, "JARVIS", "turbo", &dir).unwrap();
        let loaded = GqVoice::open(&dir.join("jarvis.gqvoice")).unwrap();
        assert_eq!(loaded.playable(), samples);
        assert_eq!(audio.samples.len(), samples.len());
        assert!((audio.samples[0] - 32767.0 / 32768.0).abs() < 1e-7);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn samantha_and_jarvis_land_on_distinct_paths_and_replace() {
        let dir = tmp("land");
        let a = vec![1i16, 2, 3];
        let b = vec![4i16, 5, 6, 7];
        handoff_at(&a, "samantha", "nano", &dir).unwrap();
        handoff_at(&b, "other", "nano", &dir).unwrap();
        assert_eq!(
            GqVoice::open(&asset_path(&dir, "samantha"))
                .unwrap()
                .playable(),
            b
        );
        handoff_at(&a, "jarvis", "nano", &dir).unwrap();
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"samantha.gqvoice".to_string()));
        assert!(names.contains(&"jarvis.gqvoice".to_string()));
        assert!(names
            .iter()
            .all(|n| !n.ends_with(".wav") && !n.ends_with(".pt") && !n.ends_with(".tmp")));
        let bad = compile(&a, "samantha", "nano").unwrap();
        let mut corrupt = bad.as_bytes().to_vec();
        corrupt[HEADER_LEN] ^= 0xff;
        assert!(GqVoice::open_bytes(&corrupt)
            .unwrap_err()
            .to_string()
            .contains("sha256"));
        assert!(compile(&[], "samantha", "nano").is_err());
        assert!(compile(&[1], "samantha", "nope").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn little_endian_payload_and_exact_sector_file() {
        let voice = compile(&[0x1234], "samantha", "nano").unwrap();
        assert_eq!(&voice.as_bytes()[HEADER_LEN..HEADER_LEN + 2], &[0x34, 0x12]);
        assert_eq!(voice.playable(), vec![0x1234]);
        assert_eq!(voice.sector(0).unwrap()[0], 0x1234);
        assert_eq!(voice.sector(0).unwrap()[1], 0);
    }

    #[test]
    fn piper_slot_does_not_compile_and_the_fallback_stays() {
        let weights = include_str!("weights.rs");
        assert!(!weights.contains("gqvoice"));
        let speech = include_str!("mod.rs");
        let head = speech.split("mod tests").next().unwrap();
        assert!(head.contains("fn fallback_piper"));
        assert!(head.contains("A21WeightTts"));
        let cargo = include_str!("../../Cargo.toml");
        assert!(!cargo.contains("torch"));
        assert!(!cargo.contains("\nort "));
    }

    #[test]
    fn default_dir_is_voices_gq_when_unset() {
        if std::env::var_os("GALION_GQVOICE").is_none() {
            assert!(gqvoice_dir().ends_with("voices/gq"));
        }
        assert_eq!(
            asset_path(Path::new("voices/gq"), "Jarvis"),
            PathBuf::from("voices/gq/jarvis.gqvoice")
        );
    }
}
