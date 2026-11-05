// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! galion-v2-asr: streaming Vosk sidecar. dlopens libvosk (Apache-2.0),
//! never links it. stdin frames: b'A' u16le n, n*i16le (16 kHz mono) |
//! b'F' (force final + reset) | b'S' (sync ack) | b'Q'. stdout NDJSON, one line per event:
//! {"k":"ready"} {"k":"p","text":..} {"k":"f","text":..}.
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::io::{Read, Write};

type ModelNew = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type RecNew = unsafe extern "C" fn(*mut c_void, f32) -> *mut c_void;
type Accept = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type Res = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type Reset = unsafe extern "C" fn(*mut c_void);
type SetLog = unsafe extern "C" fn(c_int);
extern "C" {
    fn dlopen(f: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(h: *mut c_void, s: *const c_char) -> *mut c_void;
}
unsafe fn sym(h: *mut c_void, n: &str) -> *mut c_void {
    let c = CString::new(n).unwrap();
    let p = dlsym(h, c.as_ptr());
    if p.is_null() { eprintln!("galion-v2-asr: missing symbol {n}"); std::process::exit(3); }
    p
}
fn text_of(blob: *const c_char, key: &str) -> String {
    if blob.is_null() { return String::new(); }
    let s = unsafe { CStr::from_ptr(blob) }.to_string_lossy().to_string();
    serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v.get(key).and_then(|t| t.as_str()).map(str::to_string)).unwrap_or_default()
}
fn main() {
    let root = std::env::var("GALION_REPO").unwrap_or_else(|_| ".".into());
    let lib = std::env::var("GALION_VOSK_LIB").unwrap_or_else(|_| format!("{root}/weights/asr/libvosk/libvosk.so"));
    let model = std::env::var("GALION_VOSK_MODEL").unwrap_or_else(|_| format!("{root}/weights/asr/vosk-model-small-en-us-0.15"));
    let c = CString::new(lib.clone()).unwrap();
    let h = unsafe { dlopen(c.as_ptr(), 2) };
    if h.is_null() { eprintln!("galion-v2-asr: dlopen {lib} failed. No transcript was invented."); std::process::exit(2); }
    unsafe {
        let set_log: SetLog = std::mem::transmute(sym(h, "vosk_set_log_level"));
        let model_new: ModelNew = std::mem::transmute(sym(h, "vosk_model_new"));
        let rec_new: RecNew = std::mem::transmute(sym(h, "vosk_recognizer_new"));
        let accept: Accept = std::mem::transmute(sym(h, "vosk_recognizer_accept_waveform"));
        let result: Res = std::mem::transmute(sym(h, "vosk_recognizer_result"));
        let partial: Res = std::mem::transmute(sym(h, "vosk_recognizer_partial_result"));
        let fin: Res = std::mem::transmute(sym(h, "vosk_recognizer_final_result"));
        let reset: Reset = std::mem::transmute(sym(h, "vosk_recognizer_reset"));
        set_log(-1);
        let mc = CString::new(model.clone()).unwrap();
        let m = model_new(mc.as_ptr());
        if m.is_null() { eprintln!("galion-v2-asr: model {model} failed to load"); std::process::exit(2); }
        let rec = rec_new(m, 16000.0);
        let mut out = std::io::stdout().lock();
        let mut inp = std::io::stdin().lock();
        let _ = writeln!(out, "{{\"k\":\"ready\"}}"); let _ = out.flush();
        let mut last_p = String::new();
        let mut tag = [0u8; 1];
        loop {
            if inp.read_exact(&mut tag).is_err() { break; }
            match tag[0] {
                b'A' => {
                    let mut nb = [0u8; 2];
                    if inp.read_exact(&mut nb).is_err() { break; }
                    let n = u16::from_le_bytes(nb) as usize;
                    let mut pcm = vec![0u8; n * 2];
                    if inp.read_exact(&mut pcm).is_err() { break; }
                    let r = accept(rec, pcm.as_ptr() as *const c_char, pcm.len() as c_int);
                    if r == 1 {
                        let t = text_of(result(rec), "text");
                        if !t.is_empty() { let _ = writeln!(out, "{}", serde_json::json!({"k":"f","text":t,"why":"vosk"})); }
                        last_p.clear();
                    } else {
                        let p = text_of(partial(rec), "partial");
                        if p != last_p && !p.is_empty() { let _ = writeln!(out, "{}", serde_json::json!({"k":"p","text":p})); last_p = p; }
                    }
                    let _ = out.flush();
                }
                b'F' => {
                    let t = text_of(fin(rec), "text");
                    let _ = writeln!(out, "{}", serde_json::json!({"k":"f","text":t,"why":"forced"}));
                    let _ = out.flush();
                    reset(rec); last_p.clear();
                }
                b'S' => { let _ = writeln!(out, "{{\"k\":\"s\"}}"); let _ = out.flush(); }
                _ => break,
            }
        }
    }
}
