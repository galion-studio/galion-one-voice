// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! dlopen of libvosk. Symbol signatures match vosk_api.h as used by
//! `galion-asr-vosk` (PR #26). `vosk_gpu_init` is not bound and is not called.
//! The library handle is left open for the process.

use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;

use crate::error::AsrError;

pub struct Ffi {
    model_new: ModelNew,
    model_free: ModelFree,
    rec_new: RecNew,
    set_words: SetWords,
    accept: Accept,
    result: ResultFn,
    partial: ResultFn,
    final_result: ResultFn,
    reset: ResetFn,
    rec_free: RecFree,
    set_log: SetLog,
}

type ModelNew = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type ModelFree = unsafe extern "C" fn(*mut c_void);
type RecNew = unsafe extern "C" fn(*mut c_void, f32) -> *mut c_void;
type SetWords = unsafe extern "C" fn(*mut c_void, c_int);
type Accept = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type ResultFn = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type ResetFn = unsafe extern "C" fn(*mut c_void);
type RecFree = unsafe extern "C" fn(*mut c_void);
type SetLog = unsafe extern "C" fn(c_int);

impl Ffi {
    pub fn open(explicit: Option<&Path>) -> Result<Self, AsrError> {
        let mut names = Vec::new();
        if let Some(path) = explicit {
            names.push(path.to_path_buf());
        } else if let Some(path) = crate::paths::find_vosk_lib() {
            names.push(path);
        }
        let mut errors = Vec::new();
        for path in &names {
            if let Some(dir) = path.parent() {
                prepend_lib_dir(dir);
            }
            match load_library(path) {
                Ok(handle) => return Self::bind(handle),
                Err(err) => errors.push(format!("{}: {}", path.display(), err.message())),
            }
        }
        let fallback = crate::paths::lib_file_name();
        match load_library(Path::new(fallback)) {
            Ok(handle) => return Self::bind(handle),
            Err(err) => errors.push(format!("{fallback}: {}", err.message())),
        }
        Err(AsrError::new(format!(
            "libvosk could not be loaded ({}). Run voice-stack/asr/scripts/fetch-models.sh or set GALION_VOSK_LIB. No transcript was invented.",
            errors.join("; ")
        )))
    }

    fn bind(handle: *mut c_void) -> Result<Self, AsrError> {
        unsafe {
            let ffi = Self {
                model_new: transmute_sym(sym(handle, "vosk_model_new")?),
                model_free: transmute_sym(sym(handle, "vosk_model_free")?),
                rec_new: transmute_sym(sym(handle, "vosk_recognizer_new")?),
                set_words: transmute_sym(sym(handle, "vosk_recognizer_set_words")?),
                accept: transmute_sym(sym(handle, "vosk_recognizer_accept_waveform")?),
                result: transmute_sym(sym(handle, "vosk_recognizer_result")?),
                partial: transmute_sym(sym(handle, "vosk_recognizer_partial_result")?),
                final_result: transmute_sym(sym(handle, "vosk_recognizer_final_result")?),
                reset: transmute_sym(sym(handle, "vosk_recognizer_reset")?),
                rec_free: transmute_sym(sym(handle, "vosk_recognizer_free")?),
                set_log: transmute_sym(sym(handle, "vosk_set_log_level")?),
            };
            (ffi.set_log)(-1);
            Ok(ffi)
        }
    }

    pub fn model_new(&self, path: &str) -> Result<*mut c_void, AsrError> {
        let c = CString::new(path).map_err(|_| AsrError::new("model path contains a nul"))?;
        let model = unsafe { (self.model_new)(c.as_ptr()) };
        if model.is_null() {
            Err(AsrError::new(format!(
                "vosk_model_new returned null for {path}. No transcript was invented."
            )))
        } else {
            Ok(model)
        }
    }

    pub fn model_free(&self, model: *mut c_void) {
        if !model.is_null() {
            unsafe { (self.model_free)(model) }
        }
    }

    pub fn recognizer_new(&self, model: *mut c_void, rate: f32) -> *mut c_void {
        unsafe { (self.rec_new)(model, rate) }
    }

    pub fn set_words(&self, rec: *mut c_void, on: c_int) {
        unsafe { (self.set_words)(rec, on) }
    }

    pub fn accept(&self, rec: *mut c_void, bytes: &[u8]) -> Result<c_int, AsrError> {
        if bytes.len() > c_int::MAX as usize {
            return Err(AsrError::new("frame is too large"));
        }
        let rc =
            unsafe { (self.accept)(rec, bytes.as_ptr() as *const c_char, bytes.len() as c_int) };
        if rc < 0 {
            Err(AsrError::new(
                "vosk_recognizer_accept_waveform failed. No transcript was invented.",
            ))
        } else {
            Ok(rc)
        }
    }

    pub fn result(&self, rec: *mut c_void) -> String {
        copy_cstr(unsafe { (self.result)(rec) })
    }

    pub fn partial(&self, rec: *mut c_void) -> String {
        copy_cstr(unsafe { (self.partial)(rec) })
    }

    pub fn final_result(&self, rec: *mut c_void) -> String {
        copy_cstr(unsafe { (self.final_result)(rec) })
    }

    pub fn reset(&self, rec: *mut c_void) {
        unsafe { (self.reset)(rec) }
    }

    pub fn recognizer_free(&self, rec: *mut c_void) {
        if !rec.is_null() {
            unsafe { (self.rec_free)(rec) }
        }
    }
}

fn copy_cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

unsafe fn transmute_sym<T>(ptr: *mut c_void) -> T {
    std::mem::transmute_copy(&ptr)
}

fn sym(handle: *mut c_void, name: &str) -> Result<*mut c_void, AsrError> {
    let c = CString::new(name).map_err(|_| AsrError::new(format!("bad symbol {name}")))?;
    clear_error();
    let ptr = raw_sym(handle, c.as_ptr());
    if ptr.is_null() {
        Err(AsrError::new(format!(
            "symbol {name}: {}. No transcript was invented.",
            last_error()
        )))
    } else {
        Ok(ptr)
    }
}

fn prepend_lib_dir(dir: &Path) {
    let dir = dir.display().to_string();
    #[cfg(unix)]
    {
        let old = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
        if !old.split(':').any(|part| part == dir) {
            let joined = if old.is_empty() {
                dir
            } else {
                format!("{dir}:{old}")
            };
            std::env::set_var("LD_LIBRARY_PATH", joined);
        }
    }
    #[cfg(windows)]
    {
        let old = std::env::var("PATH").unwrap_or_default();
        if !old.split(';').any(|part| part == dir) {
            // `dir` is still needed for SetDllDirectory after PATH is set.
            // Moving it into `joined` fails the Windows build (use after move).
            let joined = if old.is_empty() {
                dir.clone()
            } else {
                format!("{dir};{old}")
            };
            std::env::set_var("PATH", joined);
        }
        set_dll_directory(Path::new(&dir));
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = dir;
    }
}

#[cfg(unix)]
fn load_library(path: &Path) -> Result<*mut c_void, AsrError> {
    let c = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| AsrError::new("library path contains a nul"))?;
    let handle = unsafe { dlopen(c.as_ptr(), RTLD_NOW) };
    if handle.is_null() {
        Err(AsrError::new(last_error()))
    } else {
        Ok(handle)
    }
}

#[cfg(unix)]
fn raw_sym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
    unsafe { dlsym(handle, name) }
}

#[cfg(unix)]
fn clear_error() {
    unsafe {
        dlerror();
    }
}

#[cfg(unix)]
fn last_error() -> String {
    unsafe {
        let ptr = dlerror();
        if ptr.is_null() {
            "unknown dlopen error".into()
        } else {
            CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }
}

#[cfg(unix)]
const RTLD_NOW: c_int = 2;

#[cfg(unix)]
extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}

#[cfg(windows)]
fn load_library(path: &Path) -> Result<*mut c_void, AsrError> {
    use std::os::windows::ffi::OsStrExt;
    if let Some(parent) = path.parent() {
        set_dll_directory(parent);
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
    if handle.is_null() {
        Err(AsrError::new(format!("LoadLibraryW failed ({})", unsafe {
            GetLastError()
        })))
    } else {
        Ok(handle)
    }
}

#[cfg(windows)]
fn set_dll_directory(path: &Path) {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SetDllDirectoryW(wide.as_ptr());
    }
}

#[cfg(windows)]
fn raw_sym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
    unsafe { GetProcAddress(handle, name) }
}

#[cfg(windows)]
fn clear_error() {}

#[cfg(windows)]
fn last_error() -> String {
    format!("GetProcAddress failed ({})", unsafe { GetLastError() })
}

#[cfg(windows)]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    fn SetDllDirectoryW(path: *const u16) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(not(any(unix, windows)))]
fn load_library(path: &Path) -> Result<*mut c_void, AsrError> {
    let _ = path;
    Err(AsrError::new(
        "libvosk is loaded on unix and Windows only. No transcript was invented.",
    ))
}

#[cfg(not(any(unix, windows)))]
fn raw_sym(_handle: *mut c_void, _name: *const c_char) -> *mut c_void {
    std::ptr::null_mut()
}

#[cfg(not(any(unix, windows)))]
fn clear_error() {}

#[cfg(not(any(unix, windows)))]
fn last_error() -> String {
    "libvosk is loaded on unix and Windows only".into()
}
