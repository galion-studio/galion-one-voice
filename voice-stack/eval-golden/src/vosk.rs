// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! libvosk, loaded at runtime. Same calls as
//! `docs/galion-voice-v2/sprint-v2/bakeoff-t648/tools/wer_t648.py` and
//! `galion-asr-vosk`: log level -1, 16 kHz, 8000-byte chunks, result on an
//! endpoint, final result at the end. `vosk_gpu_init` is not called.
//! A missing library is an error string. No transcript is invented.

use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::path::Path;
use std::time::Instant;

pub struct Transcript {
    pub text: String,
    pub first_partial_ms: Option<f64>,
    pub wall_ms: f64,
}

pub struct Vosk {
    api: Api,
    model: *mut c_void,
}

struct Api {
    handle: *mut c_void,
    set_log: unsafe extern "C" fn(c_int),
    model_new: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    model_free: unsafe extern "C" fn(*mut c_void),
    rec_new: unsafe extern "C" fn(*mut c_void, c_float) -> *mut c_void,
    accept: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int,
    result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    partial: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    final_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    rec_free: unsafe extern "C" fn(*mut c_void),
}

impl Vosk {
    pub fn open(lib: &Path, model_dir: &Path) -> Result<Self, String> {
        let api = Api::load(lib)?;
        unsafe { (api.set_log)(-1) };
        let path = CString::new(model_dir.to_string_lossy().as_bytes())
            .map_err(|_| "vosk model path is not a C string".to_string())?;
        let model = unsafe { (api.model_new)(path.as_ptr()) };
        if model.is_null() {
            return Err(format!(
                "vosk_model_new returned null for {}. No transcript was invented.",
                model_dir.display()
            ));
        }
        Ok(Self { api, model })
    }

    pub fn transcribe(&self, pcm_s16le_16k: &[u8]) -> Result<Transcript, String> {
        let started = Instant::now();
        let rec = unsafe { (self.api.rec_new)(self.model, 16_000.0) };
        if rec.is_null() {
            return Err("vosk_recognizer_new returned null. No transcript was invented.".into());
        }
        let mut parts = Vec::new();
        let mut first_partial_ms = None;
        for chunk in pcm_s16le_16k.chunks(8000) {
            let rc = unsafe {
                (self.api.accept)(rec, chunk.as_ptr() as *const c_char, chunk.len() as c_int)
            };
            let partial = cstr(unsafe { (self.api.partial)(rec) });
            let partial_text = json_string_field(&partial, "partial");
            if first_partial_ms.is_none() && !partial_text.trim().is_empty() {
                first_partial_ms = Some(elapsed_ms(started));
            }
            if rc != 0 {
                let raw = cstr(unsafe { (self.api.result)(rec) });
                let text = json_string_field(&raw, "text");
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
        let raw = cstr(unsafe { (self.api.final_result)(rec) });
        unsafe { (self.api.rec_free)(rec) };
        let text = json_string_field(&raw, "text");
        if !text.is_empty() {
            parts.push(text);
        }
        Ok(Transcript {
            text: parts.join(" "),
            first_partial_ms,
            wall_ms: elapsed_ms(started),
        })
    }
}

impl Drop for Vosk {
    fn drop(&mut self) {
        unsafe {
            if !self.model.is_null() {
                (self.api.model_free)(self.model);
                self.model = std::ptr::null_mut();
            }
            close_lib(self.api.handle);
        }
    }
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

/// Pull one JSON string field out of a Vosk result object.
pub fn json_string_field(raw: &str, field: &str) -> String {
    let key = format!("\"{field}\"");
    let Some(at) = raw.find(&key) else {
        return String::new();
    };
    let rest = &raw[at + key.len()..];
    let Some(colon) = rest.find(':') else {
        return String::new();
    };
    let rest = rest[colon + 1..].trim_start();
    let Some(rest) = rest.strip_prefix('"') else {
        return String::new();
    };
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            '"' => break,
            _ => out.push(c),
        }
    }
    out
}

impl Api {
    fn load(lib: &Path) -> Result<Self, String> {
        let handle = open_lib(lib)?;
        unsafe {
            Ok(Self {
                handle,
                set_log: sym(handle, b"vosk_set_log_level\0")?,
                model_new: sym(handle, b"vosk_model_new\0")?,
                model_free: sym(handle, b"vosk_model_free\0")?,
                rec_new: sym(handle, b"vosk_recognizer_new\0")?,
                accept: sym(handle, b"vosk_recognizer_accept_waveform\0")?,
                result: sym(handle, b"vosk_recognizer_result\0")?,
                partial: sym(handle, b"vosk_recognizer_partial_result\0")?,
                final_result: sym(handle, b"vosk_recognizer_final_result\0")?,
                rec_free: sym(handle, b"vosk_recognizer_free\0")?,
            })
        }
    }
}

unsafe fn sym<T>(handle: *mut c_void, name: &[u8]) -> Result<T, String> {
    let ptr = lookup(handle, name);
    if ptr.is_null() {
        return Err(format!(
            "libvosk is missing {}. No transcript was invented.",
            String::from_utf8_lossy(name).trim_end_matches('\0')
        ));
    }
    Ok(std::mem::transmute_copy(&ptr))
}

#[cfg(target_os = "linux")]
fn open_lib(path: &Path) -> Result<*mut c_void, String> {
    const RTLD_NOW: c_int = 2;
    const RTLD_GLOBAL: c_int = 0x00100;
    let c = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "library path is not a C string".to_string())?;
    let handle = unsafe { dlopen(c.as_ptr(), RTLD_NOW | RTLD_GLOBAL) };
    if handle.is_null() {
        return Err(format!("dlopen {}: {}", path.display(), dl_error()));
    }
    Ok(handle)
}

#[cfg(target_os = "linux")]
fn lookup(handle: *mut c_void, name: &[u8]) -> *mut c_void {
    unsafe { dlsym(handle, name.as_ptr() as *const c_char) }
}

#[cfg(target_os = "linux")]
fn close_lib(handle: *mut c_void) {
    if !handle.is_null() {
        unsafe { dlclose(handle) };
    }
}

#[cfg(target_os = "linux")]
fn dl_error() -> String {
    let ptr = unsafe { dlerror() };
    if ptr.is_null() {
        return "unknown dlopen error".into();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(target_os = "linux")]
#[link(name = "dl")]
extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn dlclose(handle: *mut c_void) -> c_int;
}

#[cfg(target_os = "windows")]
fn open_lib(path: &Path) -> Result<*mut c_void, String> {
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        return Err(format!(
            "LoadLibrary {} failed. No transcript was invented.",
            path.display()
        ));
    }
    Ok(handle)
}

#[cfg(target_os = "windows")]
fn lookup(handle: *mut c_void, name: &[u8]) -> *mut c_void {
    unsafe { GetProcAddress(handle, name.as_ptr() as *const c_char) }
}

#[cfg(target_os = "windows")]
fn close_lib(handle: *mut c_void) {
    if !handle.is_null() {
        unsafe { FreeLibrary(handle) };
    }
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn FreeLibrary(module: *mut c_void) -> i32;
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn open_lib(path: &Path) -> Result<*mut c_void, String> {
    let _ = path;
    Err("libvosk is loaded on Linux and Windows only. No transcript was invented.".into())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn lookup(_handle: *mut c_void, _name: &[u8]) -> *mut c_void {
    std::ptr::null_mut()
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn close_lib(_handle: *mut c_void) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_vosk_json_text() {
        assert_eq!(
            json_string_field(r#"{"text" : "hi there"}"#, "text"),
            "hi there"
        );
        assert_eq!(json_string_field(r#"{"partial": "hel"}"#, "partial"), "hel");
        assert_eq!(json_string_field("{}", "text"), "");
    }
}
