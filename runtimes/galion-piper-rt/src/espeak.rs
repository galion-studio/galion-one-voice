// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! eSpeak-ng phonemizer, loaded at runtime from the A21 Piper pack
//! (`libespeak-ng.so.1`, rhasspy fork with `espeak_TextToPhonemesWithTerminator`).
//! Mirrors piper-phonemize 1.2 `phonemize_eSpeak`: one IPA string per clause,
//! clause punctuation re-appended, a new sentence on every sentence terminator.
#![allow(unsafe_code)]

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

type InitFn = unsafe extern "C" fn(c_int, c_int, *const c_char, c_int) -> c_int;
type SetVoiceFn = unsafe extern "C" fn(*const c_char) -> c_int;
type ToPhonFn = unsafe extern "C" fn(*mut *const c_void, c_int, c_int, *mut c_int) -> *const c_char;

const AUDIO_OUTPUT_SYNCHRONOUS: c_int = 2;
const ESPEAK_CHARS_AUTO: c_int = 0;
const ESPEAK_PHONEMES_IPA: c_int = 0x02;

const CLAUSE_INTONATION_FULL_STOP: c_int = 0x0000_0000;
const CLAUSE_INTONATION_COMMA: c_int = 0x0000_1000;
const CLAUSE_INTONATION_QUESTION: c_int = 0x0000_2000;
const CLAUSE_INTONATION_EXCLAMATION: c_int = 0x0000_3000;
const CLAUSE_TYPE_CLAUSE: c_int = 0x0004_0000;
const CLAUSE_TYPE_SENTENCE: c_int = 0x0008_0000;
const CLAUSE_PERIOD: c_int = 40 | CLAUSE_INTONATION_FULL_STOP | CLAUSE_TYPE_SENTENCE;
const CLAUSE_COMMA: c_int = 20 | CLAUSE_INTONATION_COMMA | CLAUSE_TYPE_CLAUSE;
const CLAUSE_QUESTION: c_int = 40 | CLAUSE_INTONATION_QUESTION | CLAUSE_TYPE_SENTENCE;
const CLAUSE_EXCLAMATION: c_int = 45 | CLAUSE_INTONATION_EXCLAMATION | CLAUSE_TYPE_SENTENCE;
const CLAUSE_COLON: c_int = 30 | CLAUSE_INTONATION_FULL_STOP | CLAUSE_TYPE_CLAUSE;
const CLAUSE_SEMICOLON: c_int = 30 | CLAUSE_INTONATION_COMMA | CLAUSE_TYPE_CLAUSE;

pub struct Espeak {
    _lib: *mut c_void,
    to_phonemes: ToPhonFn,
}

// SAFETY: one Espeak per process; used from the single synthesis thread only.
unsafe impl Send for Espeak {}

impl Espeak {
    pub fn open(lib: &Path, data_dir: &Path, voice: &str) -> Result<Self, String> {
        let clib = CString::new(lib.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
        // SAFETY: dlopen/dlsym on a path we were given; symbols are checked for null before use
        // and cast to the documented espeak-ng 1.52 signatures.
        unsafe {
            let h = libc::dlopen(clib.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if h.is_null() {
                return Err(format!("dlopen {}: {}", lib.display(), CStr::from_ptr(libc::dlerror()).to_string_lossy()));
            }
            let sym = |name: &[u8]| -> Result<*mut c_void, String> {
                let p = libc::dlsym(h, name.as_ptr() as *const c_char);
                if p.is_null() { Err(format!("missing symbol {}", String::from_utf8_lossy(&name[..name.len() - 1]))) } else { Ok(p) }
            };
            let init: InitFn = std::mem::transmute(sym(b"espeak_Initialize\0")?);
            let set_voice: SetVoiceFn = std::mem::transmute(sym(b"espeak_SetVoiceByName\0")?);
            let to_phonemes: ToPhonFn = std::mem::transmute(sym(b"espeak_TextToPhonemesWithTerminator\0")?);
            let cdata = CString::new(data_dir.as_os_str().as_encoded_bytes()).map_err(|e| e.to_string())?;
            if init(AUDIO_OUTPUT_SYNCHRONOUS, 0, cdata.as_ptr(), 0) < 0 {
                return Err(format!("espeak_Initialize failed for {}", data_dir.display()));
            }
            let cvoice = CString::new(voice).map_err(|e| e.to_string())?;
            if set_voice(cvoice.as_ptr()) != 0 {
                return Err(format!("espeak_SetVoiceByName({voice}) failed"));
            }
            Ok(Self { _lib: h, to_phonemes })
        }
    }

    /// Text -> one phoneme string (Unicode scalars) per sentence.
    pub fn phonemize(&self, text: &str) -> Vec<Vec<char>> {
        let ctext = match CString::new(text.replace('\0', " ")) { Ok(c) => c, Err(_) => return Vec::new() };
        let mut ptr: *const c_void = ctext.as_ptr() as *const c_void;
        let mut out: Vec<Vec<char>> = Vec::new();
        let mut open = false;
        while !ptr.is_null() {
            let mut term: c_int = 0;
            // SAFETY: ptr walks the NUL-terminated buffer owned by `ctext`; espeak sets it to null at the end.
            let s = unsafe {
                let r = (self.to_phonemes)(&mut ptr, ESPEAK_CHARS_AUTO, ESPEAK_PHONEMES_IPA, &mut term);
                if r.is_null() { String::new() } else { CStr::from_ptr(r).to_string_lossy().into_owned() }
            };
            if !open { out.push(Vec::new()); open = true; }
            let cur = out.last_mut().unwrap();
            cur.extend(s.chars());
            match term & 0x000F_FFFF {
                CLAUSE_PERIOD => cur.push('.'),
                CLAUSE_QUESTION => cur.push('?'),
                CLAUSE_EXCLAMATION => cur.push('!'),
                CLAUSE_COMMA => { cur.push(','); cur.push(' '); }
                CLAUSE_COLON => { cur.push(':'); cur.push(' '); }
                CLAUSE_SEMICOLON => { cur.push(';'); cur.push(' '); }
                _ => {}
            }
            if term & CLAUSE_TYPE_SENTENCE == CLAUSE_TYPE_SENTENCE { open = false; }
        }
        out.retain(|s| !s.is_empty());
        out
    }
}
