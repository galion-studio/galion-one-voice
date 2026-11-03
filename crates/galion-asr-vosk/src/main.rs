// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-asr-vosk` — Vosk letter A, out of process, CPU only.
//!
//! Loads libvosk with dlopen (not linked into galion-cascade). Reads one
//! NDJSON utterance, feeds 20 ms frames, writes partial lines and one final.
//! A missing model or library is an error line. This process does not invent
//! a transcript.
//!
//! `vosk_gpu_init` is not called.
#![allow(unsafe_code)]

use galion_asr_vosk::{
    clean_transcript, encode_error, encode_final, encode_grammar, encode_partial, encode_ready,
    model_dir_ok, parse_utterance, vosk_text, vosk_words, Utterance, FRAME_SAMPLES, SAMPLE_RATE,
};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::time::Instant;

fn main() {
    if let Err(e) = run() {
        emit(&encode_error(&e));
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    let model_path = Path::new(&args.model);
    if !model_dir_ok(model_path) {
        return Err(format!(
            "Vosk model not found at {} (need am/final.mdl and conf/model.conf). Download with scripts/asr/fetch-vosk.sh. Typed text still works. No transcript was invented.",
            args.model
        ));
    }
    apply_threads();
    let lib = Ffi::open(args.lib.as_deref())?;
    lib.set_log_level(-1);
    let model = lib.model_new(&args.model)?;
    emit(&encode_ready(&args.model));
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let req = parse_utterance(&line)?;
        if req.rate != SAMPLE_RATE {
            return Err(format!(
                "expected {SAMPLE_RATE} Hz PCM, got {}. No transcript was invented.",
                req.rate
            ));
        }
        recognise(&lib, model, &req)?;
    }
    lib.model_free(model);
    Ok(())
}

fn recognise(lib: &Ffi, model: *mut c_void, req: &Utterance) -> Result<(), String> {
    let grammar = req.grammar.as_ref().map(|g| encode_grammar(g));
    let rec = match &grammar {
        Some(g) => {
            let rec = lib.recognizer_grm(model, SAMPLE_RATE as f32, g)?;
            if rec.is_null() {
                return Err(
                    "Vosk could not build a grammar recognizer for this model. Phrase lists need a lookahead graph (HCLr.fst + Gr.fst). Unset GALION_VOSK_GRAMMAR for free decoding. No transcript was invented.".into(),
                );
            }
            rec
        }
        None => {
            let rec = lib.recognizer_new(model, SAMPLE_RATE as f32)?;
            if rec.is_null() {
                return Err(
                    "vosk_recognizer_new returned null. No transcript was invented.".into(),
                );
            }
            rec
        }
    };
    lib.set_words(rec, 1);
    let t0 = Instant::now();
    let mut bytes = Vec::with_capacity(req.pcm.len() * 2);
    for s in &req.pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    let frame = FRAME_SAMPLES * 2;
    let mut off = 0;
    let mut pieces: Vec<String> = Vec::new();
    let mut words = Vec::new();
    let mut last_partial = String::new();
    while off < bytes.len() {
        let end = (off + frame).min(bytes.len());
        let rc = lib.accept(rec, &bytes[off..end])?;
        off = end;
        if let Ok(partial) = vosk_text(&lib.partial(rec), "partial") {
            let partial = clean_transcript(&partial);
            if !partial.is_empty() && partial != last_partial {
                last_partial = partial.clone();
                emit(&encode_partial(&partial, elapsed_ms(t0)));
            }
        }
        if rc == 1 {
            take_result(lib, rec, &mut pieces, &mut words, false)?;
            lib.reset(rec);
        }
    }
    take_result(lib, rec, &mut pieces, &mut words, true)?;
    lib.recognizer_free(rec);
    let text = clean_transcript(&pieces.join(" "));
    words.retain(|w| w.word != "[unk]" && w.word != "<unk>");
    emit(&encode_final(&text, elapsed_ms(t0), &words));
    Ok(())
}

fn take_result(
    lib: &Ffi,
    rec: *mut c_void,
    pieces: &mut Vec<String>,
    words: &mut Vec<galion_asr_vosk::Word>,
    final_flush: bool,
) -> Result<(), String> {
    let blob = if final_flush {
        lib.final_result(rec)
    } else {
        lib.result(rec)
    };
    let (text, w) = vosk_words(&blob)?;
    let text = clean_transcript(&text);
    if !text.is_empty() {
        pieces.push(text);
    }
    words.extend(w);
    Ok(())
}

fn elapsed_ms(t0: Instant) -> u32 {
    t0.elapsed().as_millis().min(u32::MAX as u128) as u32
}

fn emit(line: &str) {
    let mut out = io::stdout();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn apply_threads() {
    if std::env::var_os("OMP_NUM_THREADS").is_none() {
        let n = std::env::var("GALION_VOSK_THREADS").unwrap_or_else(|_| "2".into());
        std::env::set_var("OMP_NUM_THREADS", n);
    }
    if std::env::var_os("OPENBLAS_NUM_THREADS").is_none() {
        let n = std::env::var("GALION_VOSK_THREADS").unwrap_or_else(|_| "2".into());
        std::env::set_var("OPENBLAS_NUM_THREADS", n);
    }
}

struct Args {
    model: String,
    lib: Option<String>,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut model = std::env::var("GALION_VOSK_MODEL")
            .unwrap_or_else(|_| galion_asr_vosk::DEFAULT_MODEL_DIR.into());
        let mut lib = std::env::var("GALION_VOSK_LIB").ok();
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            match a.as_str() {
                "--model" => model = it.next().ok_or("--model needs a path")?,
                "--lib" => lib = Some(it.next().ok_or("--lib needs a path")?),
                "-h" | "--help" => {
                    eprintln!(
                        "galion-asr-vosk --model <dir> [--lib <libvosk.so>]\n\
                         NDJSON on stdin/stdout. CPU only. See docs/asr/VOSK.md"
                    );
                    std::process::exit(0);
                }
                other => return Err(format!("unknown argument {other}")),
            }
        }
        Ok(Self { model, lib })
    }
}

struct Ffi {
    model_new: ModelNew,
    model_free: ModelFree,
    rec_new: RecNew,
    rec_grm: RecGrm,
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
type RecGrm = unsafe extern "C" fn(*mut c_void, f32, *const c_char) -> *mut c_void;
type SetWords = unsafe extern "C" fn(*mut c_void, c_int);
type Accept = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type ResultFn = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type ResetFn = unsafe extern "C" fn(*mut c_void);
type RecFree = unsafe extern "C" fn(*mut c_void);
type SetLog = unsafe extern "C" fn(c_int);

impl Ffi {
    fn open(explicit: Option<&str>) -> Result<Self, String> {
        let owned: Vec<String> = if let Some(p) = explicit {
            vec![p.to_string()]
        } else {
            let mut v = Vec::new();
            if let Ok(p) = std::env::var("GALION_VOSK_LIB") {
                if !p.is_empty() {
                    v.push(p);
                }
            }
            v.push(galion_asr_vosk::DEFAULT_LIB.into());
            v.push("libvosk.so.1".into());
            v.push("libvosk.so".into());
            v
        };
        let mut errors = Vec::new();
        for name in &owned {
            match dlopen_one(name) {
                Ok(h) => return Self::bind(h),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        Err(format!(
            "libvosk could not be loaded ({}). Place it at {} or set GALION_VOSK_LIB. CPU only. No transcript was invented. See docs/asr/VOSK.md",
            errors.join("; "),
            galion_asr_vosk::DEFAULT_LIB
        ))
    }

    fn bind(h: *mut c_void) -> Result<Self, String> {
        // SAFETY: symbol signatures match vosk_api.h (Apache-2.0). The handle stays open for the process.
        unsafe {
            Ok(Self {
                model_new: std::mem::transmute(sym(h, "vosk_model_new")?),
                model_free: std::mem::transmute(sym(h, "vosk_model_free")?),
                rec_new: std::mem::transmute(sym(h, "vosk_recognizer_new")?),
                rec_grm: std::mem::transmute(sym(h, "vosk_recognizer_new_grm")?),
                set_words: std::mem::transmute(sym(h, "vosk_recognizer_set_words")?),
                accept: std::mem::transmute(sym(h, "vosk_recognizer_accept_waveform")?),
                result: std::mem::transmute(sym(h, "vosk_recognizer_result")?),
                partial: std::mem::transmute(sym(h, "vosk_recognizer_partial_result")?),
                final_result: std::mem::transmute(sym(h, "vosk_recognizer_final_result")?),
                reset: std::mem::transmute(sym(h, "vosk_recognizer_reset")?),
                rec_free: std::mem::transmute(sym(h, "vosk_recognizer_free")?),
                set_log: std::mem::transmute(sym(h, "vosk_set_log_level")?),
            })
        }
    }

    fn set_log_level(&self, level: c_int) {
        // SAFETY: set_log_level takes an int and has no pointer.
        unsafe { (self.set_log)(level) }
    }

    fn model_new(&self, path: &str) -> Result<*mut c_void, String> {
        let c = CString::new(path).map_err(|_| "model path contains a nul")?;
        // SAFETY: c is a valid C string; the model pointer is freed in model_free.
        let m = unsafe { (self.model_new)(c.as_ptr()) };
        if m.is_null() {
            Err(format!(
                "vosk_model_new returned null for {path}. No transcript was invented."
            ))
        } else {
            Ok(m)
        }
    }

    fn model_free(&self, model: *mut c_void) {
        if !model.is_null() {
            // SAFETY: model came from model_new in this process.
            unsafe { (self.model_free)(model) }
        }
    }

    fn recognizer_new(&self, model: *mut c_void, rate: f32) -> Result<*mut c_void, String> {
        // SAFETY: model is live; the recognizer is freed after the utterance.
        Ok(unsafe { (self.rec_new)(model, rate) })
    }

    fn recognizer_grm(
        &self,
        model: *mut c_void,
        rate: f32,
        grammar: &str,
    ) -> Result<*mut c_void, String> {
        let c = CString::new(grammar).map_err(|_| "grammar contains a nul")?;
        // SAFETY: model is live; grammar is a JSON array C string.
        Ok(unsafe { (self.rec_grm)(model, rate, c.as_ptr()) })
    }

    fn set_words(&self, rec: *mut c_void, on: c_int) {
        // SAFETY: rec is a live recognizer.
        unsafe { (self.set_words)(rec, on) }
    }

    fn accept(&self, rec: *mut c_void, bytes: &[u8]) -> Result<c_int, String> {
        if bytes.len() > c_int::MAX as usize {
            return Err("frame too large".into());
        }
        // SAFETY: bytes is a live PCM16 buffer; length is the byte count libvosk expects.
        let rc =
            unsafe { (self.accept)(rec, bytes.as_ptr() as *const c_char, bytes.len() as c_int) };
        if rc < 0 {
            Err("vosk_recognizer_accept_waveform failed. No transcript was invented.".into())
        } else {
            Ok(rc)
        }
    }

    fn result(&self, rec: *mut c_void) -> String {
        cstr(unsafe { (self.result)(rec) })
    }
    fn partial(&self, rec: *mut c_void) -> String {
        cstr(unsafe { (self.partial)(rec) })
    }
    fn final_result(&self, rec: *mut c_void) -> String {
        cstr(unsafe { (self.final_result)(rec) })
    }
    fn reset(&self, rec: *mut c_void) {
        unsafe { (self.reset)(rec) }
    }
    fn recognizer_free(&self, rec: *mut c_void) {
        if !rec.is_null() {
            unsafe { (self.rec_free)(rec) }
        }
    }
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: libvosk returns a NUL-terminated string valid until the next call on this recognizer.
    // Copy it before any further call.
    unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
}

fn dlopen_one(name: &str) -> Result<*mut c_void, String> {
    let c = CString::new(name).map_err(|_| "library path contains a nul".to_string())?;
    // SAFETY: c is a valid C string; RTLD_NOW is the glibc flag.
    let h = unsafe { dlopen(c.as_ptr(), RTLD_NOW) };
    if h.is_null() {
        Err(last_dl_error())
    } else {
        Ok(h)
    }
}

fn sym(h: *mut c_void, name: &str) -> Result<*mut c_void, String> {
    let c = CString::new(name).map_err(|_| format!("bad symbol {name}"))?;
    // SAFETY: h came from dlopen; name is a valid C string. Clear dlerror first.
    unsafe {
        dlerror();
        let p = dlsym(h, c.as_ptr());
        if p.is_null() {
            Err(format!("symbol {name}: {}", last_dl_error()))
        } else {
            Ok(p)
        }
    }
}

fn last_dl_error() -> String {
    // SAFETY: dlerror returns NULL or a thread-local C string.
    unsafe {
        let p = dlerror();
        if p.is_null() {
            "unknown dlopen error".into()
        } else {
            CStr::from_ptr(p).to_string_lossy().into_owned()
        }
    }
}

extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *mut c_char;
}

const RTLD_NOW: c_int = 2;
