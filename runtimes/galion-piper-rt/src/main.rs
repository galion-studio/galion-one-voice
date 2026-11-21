// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! `galion-piper-rt`: drop-in, out-of-process replacement for the
//! `piper --json-input --output_raw` child used by `PiperWorker`.
//!
//! Same wire protocol as rhasspy piper 2023.11.14-2: one JSON line
//! `{"text":"..."}` per request on stdin, raw s16le PCM per sentence on
//! stdout (flushed as soon as that sentence is synthesized), then one
//! `Real-time factor: ...` line on stderr per request.
//!
//! What it adds (lat240 / agent 01): onnxruntime intra-op thread count,
//! spinning and CPU-affinity control (the C++ CLI exposes none of these and
//! always starts one intra-op thread per core), a pre-warm at spawn so the
//! first caller line is not the cold one, and an optional idle keepalive.
//!
//! CPU is the default. `GALION_TTS_DEVICE=cuda` (or `--device cuda`) asks for
//! the CUDA execution provider. That path runs only when a GPU is visible and
//! a CUDA build of onnxruntime 1.31.1 loads. Otherwise one stderr line says
//! `falling back to CPU` and the session stays on the CPU provider.
//! The onnxruntime shared library is loaded at runtime (`--ort_lib`); nothing
//! links ORT into the voice crates. This crate is its own cargo workspace and is
//! excluded from the main one (owner approval t613s2992, 2026-11-04, LOCKS
//! `formats.exceptions.a21_piper`).
//!
//! Piper training on own-voice clips is forbidden. This runtime is not the
//! deployed Jarvis voice. See [`policy`].
#![allow(unsafe_code)]

mod device;
mod espeak;
mod policy;

// Recorded bounds stay in the binary. They are not a runtime branch.
const _: () = assert!(policy::OWN_VOICE_CLIP_TRAINING_FORBIDDEN);
const _: () = assert!(!policy::IS_DEPLOYED_JARVIS_VOICE);

use device::{arena_bytes, cuda_lib_candidates, device_id, parse_request, plan, probe_hardware, resolve_cuda_lib, Plan};
use ort::ep::ExecutionProvider;
use ort::session::{builder::GraphOptimizationLevel, Session};
use ort::value::Tensor;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct Args {
    model: PathBuf,
    config: Option<PathBuf>,
    json_input: bool,
    noise_scale: Option<f32>,
    length_scale: Option<f32>,
    noise_w: Option<f32>,
    sentence_silence: f32,
    espeak_data: Option<PathBuf>,
    espeak_lib: Option<PathBuf>,
    ort_lib: Option<PathBuf>,
    intra_threads: Option<usize>,
    spin: bool,
    cpus: Option<Vec<usize>>,
    warmup: usize,
    keepalive_ms: u64,
    quiet: bool,
    debug: bool,
    device: Option<String>,
}

fn usage() -> ! {
    eprintln!("usage: galion-piper-rt --model FILE [--config FILE] [--output_raw] [--json-input] \
[--noise_scale N] [--length_scale N] [--noise_w N] [--sentence_silence S] [--espeak_data DIR] \
[--espeak_lib FILE] [--ort_lib FILE] [--device cpu|cuda] [--intra_threads N] [--spin 0|1] [--cpus 6-7] [--warmup N] [--keepalive_ms MS] [-q]");
    eprintln!("device defaults to cpu. cuda uses the CUDA execution provider when GALION_ORT_CUDA_LIB (or a G: unpack) loads; otherwise the process logs a fallback and stays on CPU.");
    std::process::exit(2)
}

fn parse_cpus(s: &str) -> Option<Vec<usize>> {
    let mut v = Vec::new();
    for part in s.split(',').filter(|p| !p.is_empty()) {
        let (lo, hi) = match part.split_once('-') { Some((a, b)) => (a, b), None => (part, part) };
        let (lo, hi): (usize, usize) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
        if hi < lo { return None; }
        v.extend(lo..=hi);
    }
    if v.is_empty() { None } else { Some(v) }
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        model: PathBuf::new(), config: None, json_input: false, noise_scale: None, length_scale: None, noise_w: None,
        sentence_silence: 0.2, espeak_data: None, espeak_lib: None, ort_lib: None,
        intra_threads: std::env::var("GALION_PIPER_THREADS").ok().and_then(|v| v.parse().ok()),
        spin: std::env::var("GALION_PIPER_SPIN").map(|v| v != "0").unwrap_or(true),
        cpus: std::env::var("GALION_PIPER_CPUS").ok().and_then(|v| parse_cpus(&v)),
        warmup: 2, keepalive_ms: std::env::var("GALION_PIPER_KEEPALIVE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
        quiet: false, debug: false, device: None,
    };
    while let Some(k) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match k.as_str() {
            "-m" | "--model" => a.model = val().into(),
            "-c" | "--config" => a.config = Some(val().into()),
            "--output_raw" => {}
            "--debug" => a.debug = true,
            "--json-input" => a.json_input = true,
            "--noise_scale" => a.noise_scale = val().parse().ok(),
            "--length_scale" => a.length_scale = val().parse().ok(),
            "--noise_w" => a.noise_w = val().parse().ok(),
            "--sentence_silence" => a.sentence_silence = val().parse().unwrap_or(0.2),
            "--espeak_data" => a.espeak_data = Some(val().into()),
            "--espeak_lib" => a.espeak_lib = Some(val().into()),
            "--ort_lib" => a.ort_lib = Some(val().into()),
            "--device" => a.device = Some(val()),
            "--intra_threads" => a.intra_threads = val().parse().ok(),
            "--spin" => a.spin = val() != "0",
            "--cpus" => a.cpus = parse_cpus(&val()),
            "--warmup" => a.warmup = val().parse().unwrap_or(2),
            "--keepalive_ms" => a.keepalive_ms = val().parse().unwrap_or(0),
            "-q" | "--quiet" => a.quiet = true,
            "-h" | "--help" => usage(),
            "--version" => { println!("galion-piper-rt {} (onnxruntime {ORT_VERSION}, piper protocol 2023.11.14-2)", env!("CARGO_PKG_VERSION")); std::process::exit(0) }
            flag if policy::is_training_flag(flag) => {
                eprintln!("[galion-piper-rt] refused: {}", policy::RECORD);
                std::process::exit(2)
            }
            _ => { eprintln!("unknown argument {k}"); usage() }
        }
    }
    if a.model.as_os_str().is_empty() { usage() }
    a
}

/// Runtime files (onnxruntime 1.31.1, espeak-ng from the A21 Piper pack) live in
/// `<repo>/weights/runtime/`. Lookup order: `GALION_PIPER_RUNTIME_DIR`, `GALION_REPO`,
/// the model's `../runtime` (`weights/tts/x.onnx` -> `weights/runtime`), the
/// installed binary's parent (`weights/runtime/piper-rt/galion-piper-rt`), then
/// this crate's checkout.
fn runtime_dir(model: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("GALION_PIPER_RUNTIME_DIR") { return p.into(); }
    if let Ok(r) = std::env::var("GALION_REPO") { return Path::new(&r).join("weights/runtime"); }
    let has_pack = |w: &Path| w.join("piper").is_dir();
    if let Some(w) = model.canonicalize().ok().as_deref().and_then(Path::parent).and_then(Path::parent).map(|w| w.join("runtime")) {
        if has_pack(&w) { return w; }
    }
    if let Some(w) = std::env::current_exe().ok().as_deref().and_then(Path::parent).and_then(Path::parent).map(Path::to_path_buf) {
        if has_pack(&w) { return w; }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../weights/runtime")
}

/// onnxruntime shared library this runtime is pinned to (LOCKS `a21_piper.onnxruntime`).
pub const ORT_VERSION: &str = "1.31.1";
/// Default intra-op threads when neither `--intra_threads` nor `GALION_PIPER_THREADS` is set.
pub const DEFAULT_THREADS: usize = 4;

fn set_affinity(cpus: &[usize]) -> Result<(), String> {
    // SAFETY: cpu_set_t is plain data; CPU_SET writes inside it; pid 0 = this process.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &c in cpus { libc::CPU_SET(c, &mut set); }
        if libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set) != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}

struct Voice {
    session: Session,
    espeak: espeak::Espeak,
    ids: HashMap<char, Vec<i64>>,
    scales: [f32; 3],
}

impl Voice {
    fn ids_for(&self, phonemes: &[char]) -> Vec<i64> {
        let mut v = Vec::with_capacity(phonemes.len() * 2 + 3);
        let get = |c: char| self.ids.get(&c).cloned().unwrap_or_default();
        let pad = get('_');
        v.extend(get('^'));
        v.extend(&pad);
        for &p in phonemes {
            if let Some(id) = self.ids.get(&p) { v.extend(id); v.extend(&pad); }
        }
        v.extend(get('$'));
        v
    }

    /// One sentence of phoneme ids -> s16 PCM, scaled like piper (peak-normalized).
    fn infer(&mut self, ids: &[i64]) -> Result<Vec<i16>, String> {
        let n = ids.len();
        let input = Tensor::from_array(([1usize, n], ids.to_vec())).map_err(|e| e.to_string())?;
        let lens = Tensor::from_array(([1usize], vec![n as i64])).map_err(|e| e.to_string())?;
        let scales = Tensor::from_array(([3usize], self.scales.to_vec())).map_err(|e| e.to_string())?;
        let out = self.session
            .run(ort::inputs!["input" => input, "input_lengths" => lens, "scales" => scales])
            .map_err(|e| e.to_string())?;
        let (_, audio) = out[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let peak = audio.iter().fold(0.01f32, |m, &x| m.max(x.abs()));
        let k = 32767.0 / peak;
        Ok(audio.iter().map(|&x| (x * k).clamp(-32767.0, 32767.0) as i16).collect())
    }
}

fn load_ort(path: &Path) -> Result<ort::environment::Environment, String> {
    let builder = ort::init_from(path).map_err(|e| e.to_string())?;
    builder.with_name("galion-piper-rt").build().map_err(|e| e.to_string())
}

fn cuda_library_or_cpu(cpu_lib: &Path, gpu_lib: Option<&Path>, plan: Plan) -> (PathBuf, Plan) {
    if plan.try_cuda {
        if let Some(gpu) = gpu_lib {
            if gpu.is_file() {
                return (gpu.to_path_buf(), plan);
            }
        }
        return (
            cpu_lib.to_path_buf(),
            Plan {
                try_cuda: false,
                log: Some(
                    "GALION_TTS_DEVICE=cuda but the CUDA onnxruntime library is not on disk; falling back to CPU"
                        .into(),
                ),
            },
        );
    }
    (cpu_lib.to_path_buf(), plan)
}

fn provider_available(plan: Plan) -> Plan {
    if !plan.try_cuda {
        return plan;
    }
    match ort::ep::CUDA::is_available() {
        Ok(true) => plan,
        Ok(false) => Plan {
            try_cuda: false,
            log: Some(
                "GALION_TTS_DEVICE=cuda but the loaded onnxruntime reports no CUDAExecutionProvider; falling back to CPU"
                    .into(),
            ),
        },
        Err(e) => Plan {
            try_cuda: false,
            log: Some(format!(
                "GALION_TTS_DEVICE=cuda but CUDAExecutionProvider could not be queried ({e}); falling back to CPU"
            )),
        },
    }
}

fn build_session(
    env: &ort::environment::Environment,
    model: &Path,
    threads: usize,
    spin: bool,
    cuda: bool,
    cuda_device: i32,
    arena: usize,
) -> Result<Session, String> {
    let b = Session::builder(env).map_err(|e| e.to_string())?;
    let b = b.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| e.to_string())?;
    let b = b.with_intra_threads(threads).map_err(|e| e.to_string())?;
    let b = b.with_inter_threads(1).map_err(|e| e.to_string())?;
    let b = b.with_intra_op_spinning(spin).map_err(|e| e.to_string())?;
    let mut b = if cuda {
        let ep = ort::ep::CUDA::default()
            .with_device_id(cuda_device)
            .with_memory_limit(arena)
            .with_conv_algorithm_search(ort::ep::cuda::ConvAlgorithmSearch::Heuristic)
            .with_conv_max_workspace(false)
            .with_cuda_graph(false)
            .with_tf32(false)
            .build()
            .error_on_failure();
        b.with_execution_providers([ep]).map_err(|e| e.to_string())?
    } else {
        b
    };
    b.commit_from_file(model).map_err(|e| e.to_string())
}

fn open_session(
    env: &ort::environment::Environment,
    model: &Path,
    threads: usize,
    spin: bool,
    mut chosen: Plan,
    cuda_device: i32,
    arena: usize,
) -> (Result<Session, String>, Plan) {
    let built = build_session(env, model, threads, spin, chosen.try_cuda, cuda_device, arena);
    if let Err(err) = &built {
        if chosen.try_cuda {
            chosen = Plan {
                try_cuda: false,
                log: Some(format!(
                    "GALION_TTS_DEVICE=cuda but the CUDA execution provider did not load ({err}); falling back to CPU"
                )),
            };
            return (build_session(env, model, threads, spin, false, cuda_device, arena), chosen);
        }
    }
    (built, chosen)
}

fn main() {
    let a = parse_args();
    if let Some(c) = &a.cpus {
        if let Err(e) = set_affinity(c) { eprintln!("[galion-piper-rt] affinity {c:?}: {e}"); std::process::exit(1); }
    }
    let rt = runtime_dir(&a.model);
    let config_path = a.config.clone().unwrap_or_else(|| PathBuf::from(format!("{}.json", a.model.display())));
    let cfg: serde_json::Value = match std::fs::read_to_string(&config_path).ok().and_then(|s| serde_json::from_str(&s).ok()) {
        Some(v) => v,
        None => { eprintln!("[galion-piper-rt] cannot read config {}", config_path.display()); std::process::exit(1) }
    };
    let rate = cfg["audio"]["sample_rate"].as_u64().unwrap_or(22050) as u32;
    let voice = cfg["espeak"]["voice"].as_str().unwrap_or("en-us").to_string();
    let inf = &cfg["inference"];
    let scales = [
        a.noise_scale.unwrap_or(inf["noise_scale"].as_f64().unwrap_or(0.667) as f32),
        a.length_scale.unwrap_or(inf["length_scale"].as_f64().unwrap_or(1.0) as f32),
        a.noise_w.unwrap_or(inf["noise_w"].as_f64().unwrap_or(0.8) as f32),
    ];
    let mut ids = HashMap::new();
    if let Some(m) = cfg["phoneme_id_map"].as_object() {
        for (k, v) in m {
            let mut ch = k.chars();
            if let (Some(c), None) = (ch.next(), ch.next()) {
                ids.insert(c, v.as_array().map(|a| a.iter().filter_map(|x| x.as_i64()).collect()).unwrap_or_default());
            }
        }
    }
    let cpu_ort_lib = a.ort_lib.clone().unwrap_or_else(|| rt.join(format!("ort/onnxruntime-linux-x64-{ORT_VERSION}/lib/libonnxruntime.so.{ORT_VERSION}")));
    let espeak_lib = a.espeak_lib.clone().unwrap_or_else(|| rt.join("piper/libespeak-ng.so.1"));
    let espeak_data = a.espeak_data.clone().unwrap_or_else(|| rt.join("piper/espeak-ng-data"));

    let requested_raw = a.device.clone().unwrap_or_else(|| std::env::var("GALION_TTS_DEVICE").unwrap_or_default());
    let (request, unknown) = parse_request(&requested_raw);
    let hardware = probe_hardware();
    let explicit_cuda = std::env::var_os("GALION_ORT_CUDA_LIB").map(PathBuf::from);
    let repo = std::env::var("GALION_REPO").ok().map(PathBuf::from);
    let cuda_lib = resolve_cuda_lib(explicit_cuda.as_deref(), &cuda_lib_candidates(repo.as_deref()));
    let chosen = plan(request, unknown, &hardware, cuda_lib.as_deref());
    let (ort_lib, chosen) = cuda_library_or_cpu(&cpu_ort_lib, cuda_lib.as_deref(), chosen);

    let t_load = Instant::now();
    let fail = |what: &str, e: String| -> ! { eprintln!("[galion-piper-rt] {what}: {e}"); std::process::exit(1) };
    let (env, mut chosen) = if chosen.try_cuda && ort_lib != cpu_ort_lib {
        match load_ort(&ort_lib) {
            Ok(env) => (env, chosen),
            Err(e) => {
                let note = format!("GALION_TTS_DEVICE=cuda but the CUDA onnxruntime library failed to load ({e}); falling back to CPU");
                match load_ort(&cpu_ort_lib) {
                    Ok(env) => (env, Plan { try_cuda: false, log: Some(note) }),
                    Err(e2) => fail("load onnxruntime", e2),
                }
            }
        }
    } else {
        match load_ort(&ort_lib) {
            Ok(env) => (env, chosen),
            Err(e) => fail("load onnxruntime", e),
        }
    };
    chosen = provider_available(chosen);
    let ncpu = a.cpus.as_ref().map(|c| c.len()).unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    let threads = a.intra_threads.unwrap_or(ncpu.min(DEFAULT_THREADS)).max(1);
    let cuda_id = device_id(std::env::var("GALION_TTS_CUDA_DEVICE").ok().as_deref());
    let arena = arena_bytes(std::env::var("GALION_TTS_CUDA_ARENA_BYTES").ok().as_deref());
    let (session, chosen) = open_session(&env, &a.model, threads, a.spin, chosen, cuda_id, arena);
    let session = match session { Ok(s) => s, Err(e) => fail("load model", e) };
    if let Some(note) = &chosen.log {
        eprintln!("[galion-piper-rt] {note}");
    }
    let device_name = if chosen.try_cuda { "cuda" } else { "cpu" };
    eprintln!("[galion-piper-rt] device={device_name}");
    let espeak = match espeak::Espeak::open(&espeak_lib, &espeak_data, &voice) { Ok(e) => e, Err(e) => fail("espeak", e) };
    let mut v = Voice { session, espeak, ids, scales };
    // Pre-warm inside the child: the first caller line must not pay first-run allocation.
    for i in 0..a.warmup {
        let s = v.espeak.phonemize(if i % 2 == 0 { "Hello there." } else { "Okay, I hear you now." });
        for p in &s { let ids = v.ids_for(p); let _ = v.infer(&ids); }
    }
    if !a.quiet {
        eprintln!("[galion-piper-rt] loaded {} in {:.3}s (device={device_name} threads={threads} spin={} cpus={:?} warmup={})",
            a.model.display(), t_load.elapsed().as_secs_f64(), a.spin as u8, a.cpus, a.warmup);
    }

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line { Ok(l) => { if tx.send(l).is_err() { break; } } Err(_) => break }
        }
    });
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let silence = vec![0i16; (a.sentence_silence.max(0.0) * rate as f32) as usize];
    let keep = if a.keepalive_ms > 0 { Some(Duration::from_millis(a.keepalive_ms)) } else { None };
    loop {
        let line = match keep {
            Some(d) => match rx.recv_timeout(d) {
                Ok(l) => l,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Idle keepalive: one tiny inference keeps weights and arenas hot. No output.
                    let s = v.espeak.phonemize("Mm.");
                    for p in &s { let ids = v.ids_for(p); let _ = v.infer(&ids); }
                    continue;
                }
                Err(_) => break,
            },
            None => match rx.recv() { Ok(l) => l, Err(_) => break },
        };
        let text = if a.json_input {
            match serde_json::from_str::<serde_json::Value>(&line) {
                Ok(j) => j["text"].as_str().unwrap_or("").to_string(),
                Err(e) => { eprintln!("[galion-piper-rt] bad json line: {e}"); continue }
            }
        } else { line };
        let t0 = Instant::now();
        let mut infer_s = 0.0f64;
        let mut samples = 0usize;
        for p in v.espeak.phonemize(&text) {
            let ids = v.ids_for(&p);
            if a.debug {
                let l: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
                eprintln!("[galion-piper-rt] [debug] Converted {} phoneme(s) to {} phoneme id(s): {}", p.len(), ids.len(), l.join(", "));
            }
            let ti = Instant::now();
            let pcm = match v.infer(&ids) { Ok(p) => p, Err(e) => { eprintln!("[galion-piper-rt] infer: {e}"); continue } };
            infer_s += ti.elapsed().as_secs_f64();
            samples += pcm.len() + silence.len();
            let mut bytes = Vec::with_capacity((pcm.len() + silence.len()) * 2);
            for s in pcm.iter().chain(silence.iter()) { bytes.extend_from_slice(&s.to_le_bytes()); }
            // Each sentence goes out the moment it exists (streaming, not whole-request).
            if out.write_all(&bytes).and_then(|_| out.flush()).is_err() { return; }
        }
        let audio_s = samples as f64 / rate as f64;
        let _ = t0;
        eprintln!("[galion-piper-rt] [info] Real-time factor: {} (infer={} sec, audio={} sec)",
            if audio_s > 0.0 { infer_s / audio_s } else { 0.0 }, infer_s, audio_s);
    }
}
