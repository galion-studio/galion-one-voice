//! galion-wer: vosk ASR (libvosk C API via dlopen, same vosk-model-small-en-us-0.15 as wer_t648.py) + WER
//! with wer_t648.py normalisation: lower(), [^a-z' ] -> ' ', split on whitespace; WER = Levenshtein(ref,hyp)/len(ref).
use galion_quality::{audio, cli::{self, r, Args}, resample::Resampler};
use libloading::{Library, Symbol};
use serde_json::json;
use std::ffi::{c_char, c_float, c_int, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::time::Instant;

const USAGE: &str = "usage: galion-wer [--ref TEXT | --ref-file F | --text-dir DIR (uses DIR/<stem>.txt)] \
[--model VOSK_DIR] [--lib libvosk.so] [--jobs N<=2] [--out out.json] clip.wav...\n       galion-wer --score-only --ref TEXT --hyp TEXT";
const R: &str = "/workspace/swarm/voice-v2-sprint/repo/weights/asr/";

pub fn norm(s: &str) -> Vec<String> {
    let low = s.to_lowercase();
    let mapped: String = low.chars().map(|c| if c.is_ascii_lowercase() || c == '\'' || c == ' ' { c } else { ' ' }).collect();
    mapped.split_whitespace().map(|w| w.to_string()).collect()
}

/// Returns (edit distance, substitutions, deletions, insertions) via a full DP with backtrace.
fn align(rf: &[String], hy: &[String]) -> (usize, usize, usize, usize) {
    let (n, m) = (rf.len(), hy.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n { d[i][0] = i } for j in 0..=m { d[0][j] = j }
    for i in 1..=n { for j in 1..=m {
        d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + (rf[i - 1] != hy[j - 1]) as usize);
    } }
    let (mut i, mut j, mut s, mut de, mut ins) = (n, m, 0, 0, 0);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + (rf[i - 1] != hy[j - 1]) as usize { s += (rf[i - 1] != hy[j - 1]) as usize; i -= 1; j -= 1; }
        else if i > 0 && d[i][j] == d[i - 1][j] + 1 { de += 1; i -= 1; }
        else { ins += 1; j -= 1; }
    }
    (d[n][m], s, de, ins)
}

struct Vosk {
    _lib: &'static Library, model: *mut c_void,
    rec_new: unsafe extern "C" fn(*mut c_void, c_float) -> *mut c_void,
    accept: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int,
    result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    final_result: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    rec_free: unsafe extern "C" fn(*mut c_void),
}
unsafe impl Sync for Vosk {}
unsafe impl Send for Vosk {}

impl Vosk {
    fn open(lib: &Path, model: &Path) -> Result<Vosk, String> {
        unsafe {
            let l: &'static Library = Box::leak(Box::new(Library::new(lib).map_err(|e| format!("{}: {e}", lib.display()))?));
            macro_rules! sym { ($n:expr, $t:ty) => {{ let s: Symbol<$t> = l.get($n).map_err(|e| e.to_string())?; *s }} }
            let set_log = sym!(b"vosk_set_log_level", unsafe extern "C" fn(c_int));
            set_log(-1);
            let model_new = sym!(b"vosk_model_new", unsafe extern "C" fn(*const c_char) -> *mut c_void);
            let cm = CString::new(model.to_string_lossy().as_bytes()).unwrap();
            let m = model_new(cm.as_ptr());
            if m.is_null() { return Err(format!("vosk_model_new failed: {}", model.display())); }
            Ok(Vosk { _lib: l, model: m,
                rec_new: sym!(b"vosk_recognizer_new", unsafe extern "C" fn(*mut c_void, c_float) -> *mut c_void),
                accept: sym!(b"vosk_recognizer_accept_waveform", unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int),
                result: sym!(b"vosk_recognizer_result", unsafe extern "C" fn(*mut c_void) -> *const c_char),
                final_result: sym!(b"vosk_recognizer_final_result", unsafe extern "C" fn(*mut c_void) -> *const c_char),
                rec_free: sym!(b"vosk_recognizer_free", unsafe extern "C" fn(*mut c_void)) })
        }
    }
    /// Same feeding as wer_t648.py: s16le 16 kHz in 8000-byte chunks, Result() on endpoint, FinalResult() at end.
    fn transcribe(&self, pcm: &[u8]) -> String {
        let text = |p: *const c_char| -> String {
            let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().to_string();
            serde_json::from_str::<serde_json::Value>(&s).ok().and_then(|v| v["text"].as_str().map(|t| t.to_string())).unwrap_or_default()
        };
        let mut parts = vec![];
        unsafe {
            let rec = (self.rec_new)(self.model, 16000.0);
            for c in pcm.chunks(8000) {
                if (self.accept)(rec, c.as_ptr() as *const c_char, c.len() as c_int) != 0 { parts.push(text((self.result)(rec))); }
            }
            parts.push(text((self.final_result)(rec)));
            (self.rec_free)(rec);
        }
        parts.join(" ")
    }
}

fn pcm16k(p: &Path) -> Result<(Vec<u8>, f64), String> {
    let w = audio::load(p)?;
    let m = w.mono();
    let x = if w.sr == 16000 { m } else { Resampler::new(w.sr, 16000).process(&m) };
    let mut b = Vec::with_capacity(x.len() * 2);
    for v in &x { b.extend_from_slice(&((v * 32768.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()); }
    Ok((b, w.duration_s()))
}

fn score(rf: &[String], hy: &[String]) -> serde_json::Value {
    let (e, s, d, i) = align(rf, hy);
    let w = if rf.is_empty() { f64::NAN } else { e as f64 / rf.len() as f64 };
    json!({"wer": r(w, 3), "wer_raw": r(w, 6), "errors": e, "sub": s, "del": d, "ins": i, "n_ref_words": rf.len(), "hyp": hy.join(" ")})
}

fn main() {
    let a = Args::parse(&["help", "score-only"]);
    if a.has("help") { cli::die(USAGE) }
    if a.has("score-only") {
        let rf = norm(a.get("ref").unwrap_or_else(|| cli::die(USAGE)));
        let hy = norm(a.get("hyp").unwrap_or(""));
        println!("{}", serde_json::to_string_pretty(&score(&rf, &hy)).unwrap()); return;
    }
    if a.pos.is_empty() { cli::die(USAGE) }
    let fixed_ref = match (a.get("ref"), a.get("ref-file")) {
        (Some(t), _) => Some(t.to_string()),
        (None, Some(f)) => Some(std::fs::read_to_string(f).unwrap_or_else(|e| cli::die(&format!("{f}: {e}")))),
        _ => None,
    };
    let text_dir = a.get("text-dir").map(PathBuf::from);
    if fixed_ref.is_none() && text_dir.is_none() { cli::die("galion-wer: need --ref, --ref-file or --text-dir") }
    let lib = a.get("lib").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{R}libvosk/libvosk.so")));
    let model = a.get("model").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(format!("{R}vosk-model-small-en-us-0.15")));
    let jobs: usize = a.get("jobs").and_then(|s| s.parse().ok()).unwrap_or(1).clamp(1, 2);
    let t0 = Instant::now();
    let v = Vosk::open(&lib, &model).unwrap_or_else(|e| cli::die(&e));
    let load_s = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let files: Vec<String> = a.pos.clone();
    let work = |f: &String| -> serde_json::Value {
        let tf = Instant::now();
        let p = Path::new(f);
        let reftext = match &fixed_ref { Some(t) => Ok(t.clone()), None => {
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let tp = text_dir.as_ref().unwrap().join(format!("{stem}.txt"));
            std::fs::read_to_string(&tp).map_err(|e| format!("{}: {e}", tp.display())) } };
        let reftext = match reftext { Ok(t) => t, Err(e) => return json!({"path": f, "error": e}) };
        let (pcm, dur) = match pcm16k(p) { Ok(x) => x, Err(e) => return json!({"path": f, "error": e}) };
        let hyp = v.transcribe(&pcm);
        let mut o = score(&norm(&reftext), &norm(&hyp));
        o["path"] = json!(f); o["duration_s"] = r(dur, 3); o["ms"] = r(tf.elapsed().as_secs_f64() * 1e3, 1);
        o
    };
    let rows: Vec<serde_json::Value> = if jobs == 1 { files.iter().map(work).collect() } else {
        let mut out: Vec<Option<serde_json::Value>> = vec![None; files.len()];
        let next = std::sync::atomic::AtomicUsize::new(0);
        let res = std::sync::Mutex::new(&mut out);
        std::thread::scope(|s| { for _ in 0..jobs { s.spawn(|| loop {
            let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if i >= files.len() { break }
            let v = work(&files[i]); res.lock().unwrap()[i] = Some(v);
        }); } });
        out.into_iter().map(|x| x.unwrap()).collect()
    };
    // summary over the 3-decimal per-clip WERs, population sd: identical convention to own-clone-analysis/scores/wer.json
    let wers: Vec<f64> = rows.iter().filter_map(|x| x["wer"].as_f64()).collect();
    let raw: Vec<f64> = rows.iter().filter_map(|x| x["wer_raw"].as_f64()).collect();
    let (m, sd) = cli::mean_sd(&wers);
    let (m_raw, _) = cli::mean_sd(&raw);
    let le10 = if wers.is_empty() { f64::NAN } else { wers.iter().filter(|&&w| w <= 0.1).count() as f64 / wers.len() as f64 };
    let audio_s: f64 = rows.iter().filter_map(|x| x["duration_s"].as_f64()).sum();
    let score_s = t1.elapsed().as_secs_f64();
    let out = json!({"tool": "galion-wer", "asr": model.display().to_string(), "normalisation": "wer_t648.py: lower, [^a-z' ]->space, split",
        "files": rows,
        "summary": {"n": wers.len(), "mean": r(m, 3), "mean_raw": r(m_raw, 5), "sd": r(sd, 3), "min": r(wers.iter().cloned().fold(f64::INFINITY, f64::min), 3),
            "max": r(wers.iter().cloned().fold(f64::NEG_INFINITY, f64::max), 3), "frac_le_10pct": r(le10, 3)},
        "timing": {"load_s": r(load_s, 3), "score_s": r(score_s, 3), "audio_s": r(audio_s, 3), "rtf": r(score_s / audio_s.max(1e-9), 4), "jobs": jobs}});
    let s = serde_json::to_string_pretty(&out).unwrap();
    match a.get("out") { Some(p) => std::fs::write(p, s + "\n").unwrap_or_else(|e| cli::die(&e.to_string())), None => println!("{s}") }
}
