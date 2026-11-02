// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! INTERIM system TTS: distro `libflite1` (CMU Flite 2.2) opened at RUNTIME via
//! `dlopen`. Nothing is linked at build time and nothing is vendored; if the
//! library is absent, [`FliteTts::open_default`] returns an error and the
//! caller falls through. Real English words, 16 kHz from flite, resampled to
//! 24 kHz. NOT the A21 voice, NOT neural Galion weights.
//!
//! LOCKS note: `formats.foreign_runtime_allowed = false` targets inference
//! runtimes (candle/ort/tract/burn/llama/ggml). Flite is a classic C speech
//! synthesizer, not on that list, but its use as an interim path is an
//! explicit A21 counsel item (see docs/agent-v1-demo/A21-BLOCKERS.md).
#![allow(unsafe_code)]

use super::{resample, SPEECH_RATE};
use crate::{split_lang_token, Audio, LanguageId, TtsEngine, VoiceError};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::{Mutex, OnceLock};

extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
    fn srand(seed: u32);
}
const RTLD_NOW: c_int = 2;
const RTLD_GLOBAL: c_int = 0x100;

/// Flite `cst_wave` (flite/include/cst_wave.h).
#[repr(C)]
struct CstWave { kind: *const c_char, sample_rate: c_int, num_samples: c_int, num_channels: c_int, samples: *mut i16 }

type InitFn = unsafe extern "C" fn() -> c_int;
type RegisterFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type TextToWaveFn = unsafe extern "C" fn(*const c_char, *mut c_void) -> *mut CstWave;
type DeleteWaveFn = unsafe extern "C" fn(*mut CstWave);

struct Lib { text_to_wave: TextToWaveFn, delete_wave: DeleteWaveFn, voice: usize }
// Flite is not thread-safe: every call goes through LIBS' mutex.
static LIBS: OnceLock<Mutex<Vec<(String, Result<Lib, String>)>>> = OnceLock::new();

/// Voices shipped by Debian/Ubuntu `libflite1`.
pub const VOICES: &[&str] = &["slt", "kal16", "awb", "rms", "kal"];

fn last_dl_error() -> String {
    // SAFETY: dlerror returns NULL or a NUL-terminated thread-local string.
    unsafe { let p = dlerror(); if p.is_null() { "unknown dlopen error".into() } else { CStr::from_ptr(p).to_string_lossy().into_owned() } }
}

fn open_lib(name: &str) -> Result<*mut c_void, String> {
    let c = CString::new(name).unwrap();
    // SAFETY: valid C string; flags are standard glibc values.
    let h = unsafe { dlopen(c.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
    if h.is_null() { Err(format!("{name}: {}", last_dl_error())) } else { Ok(h) }
}

fn sym(h: *mut c_void, name: &str) -> Result<*mut c_void, String> {
    let c = CString::new(name).unwrap();
    // SAFETY: handle came from dlopen; name is a valid C string.
    let p = unsafe { dlsym(h, c.as_ptr()) };
    if p.is_null() { Err(format!("symbol {name}: {}", last_dl_error())) } else { Ok(p) }
}

fn load(voice: &str) -> Result<Lib, String> {
    let core = open_lib("libflite.so.1")?;
    // Language + lexicon first (voice libs reference them).
    open_lib("libflite_usenglish.so.1")?;
    open_lib("libflite_cmulex.so.1")?;
    let vlib = open_lib(&format!("libflite_cmu_us_{voice}.so.1"))?;
    // SAFETY: symbol signatures match flite 2.2's public C API.
    unsafe {
        let init: InitFn = std::mem::transmute(sym(core, "flite_init")?);
        let text_to_wave: TextToWaveFn = std::mem::transmute(sym(core, "flite_text_to_wave")?);
        let delete_wave: DeleteWaveFn = std::mem::transmute(sym(core, "delete_wave")?);
        let register: RegisterFn = std::mem::transmute(sym(vlib, &format!("register_cmu_us_{voice}"))?);
        init();
        let v = register(std::ptr::null());
        if v.is_null() { return Err(format!("register_cmu_us_{voice} returned NULL")); }
        Ok(Lib { text_to_wave, delete_wave, voice: v as usize })
    }
}

fn with_lib<R>(voice: &str, f: impl FnOnce(&Lib) -> R) -> Result<R, String> {
    let m = LIBS.get_or_init(|| Mutex::new(Vec::new()));
    let mut g = m.lock().unwrap_or_else(|p| p.into_inner());
    if !g.iter().any(|(v, _)| v == voice) { g.push((voice.to_string(), load(voice))); }
    let (_, r) = g.iter().find(|(v, _)| v == voice).unwrap();
    match r { Ok(l) => Ok(f(l)), Err(e) => Err(e.clone()) }
}

/// Interim English TTS on the system libflite.
pub struct FliteTts { voice: String }

impl FliteTts {
    /// Open `voice` (one of [`VOICES`]).
    pub fn open(voice: &str) -> Result<Self, VoiceError> {
        if !VOICES.contains(&voice) { return Err(VoiceError::Backend(format!("unknown flite voice {voice}"))); }
        with_lib(voice, |_| ()).map_err(|e| VoiceError::Backend(format!("libflite unavailable ({e}); install the distro package libflite1")))?;
        Ok(Self { voice: voice.to_string() })
    }
    /// Open `GALION_FLITE_VOICE` or `slt`.
    pub fn open_default() -> Result<Self, VoiceError> {
        Self::open(&std::env::var("GALION_FLITE_VOICE").unwrap_or_else(|_| "slt".into()))
    }
    pub fn voice(&self) -> &str { &self.voice }
}

impl TtsEngine for FliteTts {
    fn languages(&self) -> &[LanguageId] { LanguageId::ALL }
    fn synthesize(&mut self, text: &str, lang: LanguageId) -> Result<Audio, VoiceError> {
        let _ = lang; // English only.
        let (_, body) = split_lang_token(text);
        let body = body.trim();
        if body.is_empty() { return Err(VoiceError::EmptyInput); }
        let c = CString::new(body.replace('\0', " ")).unwrap();
        let (rate, pcm) = with_lib(&self.voice, |l| {
            // SAFETY: voice pointer from register_*; wave freed below; access serialized by mutex.
            unsafe {
                // Flite's excitation noise uses libc rand(); reseed under the mutex so
                // the same text always yields the same samples (demo determinism).
                srand(0x6a1_0001);
                let w = (l.text_to_wave)(c.as_ptr(), l.voice as *mut c_void);
                if w.is_null() { return (0, Vec::new()); }
                let wv = &*w;
                let n = (wv.num_samples.max(0) as usize) * (wv.num_channels.max(1) as usize);
                let pcm: Vec<i16> = if wv.samples.is_null() { Vec::new() } else { std::slice::from_raw_parts(wv.samples, n).to_vec() };
                let ch = wv.num_channels.max(1) as usize;
                let rate = wv.sample_rate as u32;
                (l.delete_wave)(w);
                (rate, pcm.chunks(ch).map(|c| c[0]).collect())
            }
        }).map_err(VoiceError::Backend)?;
        if pcm.is_empty() || rate == 0 { return Err(VoiceError::Backend("flite produced no audio".into())); }
        let f: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
        Ok(Audio { sample_rate: SPEECH_RATE, samples: resample(&f, rate, SPEECH_RATE) })
    }
}
