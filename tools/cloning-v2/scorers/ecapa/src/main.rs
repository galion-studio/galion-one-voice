//! galion-ecapa: Rust-native speaker similarity (speechbrain ECAPA-TDNN, VoxCeleb) via ONNX Runtime.
//! Pipeline matches bakeoff-t648/tools/score_t648.py exactly:
//!   wav -> mono mean -> torchaudio sinc_interp_hann resample to 16 kHz -> speechbrain Fbank(n_mels=80)
//!   -> InputNormalization(sentence, std_norm=False) -> ECAPA (ONNX) -> L2 normalize -> cosine.
use anyhow::{bail, Context, Result};
use realfft::RealFftPlanner;
use serde_json::{json, Map, Value};
use std::time::Instant;

const SR: u32 = 16000;
const N_FFT: usize = 400;
const HOP: usize = 160;
const N_MELS: usize = 80;
const N_STFT: usize = N_FFT / 2 + 1;
const DEFAULT_MODEL: &str = "/workspace/swarm/voice-v2-sprint/cloning-v2/scorers/ecapa/models/ecapa.onnx";

// ---------- wav ----------
fn load_wav(path: &str) -> Result<(Vec<f32>, u32)> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {path}"))?;
    let spec = r.spec();
    let ch = spec.channels as usize;
    let inter: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            // torchaudio normalize=True: int16/32768, int32/2^31, 24-bit stored left-justified -> /2^23, u8 -> (x-128)/128
            let scale = (1i64 << (bits - 1)) as f32;
            if bits <= 8 {
                r.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
            } else {
                r.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
            }
        }
    };
    if ch == 1 {
        return Ok((inter, spec.sample_rate));
    }
    let n = inter.len() / ch;
    let mut mono = vec![0f32; n];
    for i in 0..n {
        let mut s = 0f32;
        for c in 0..ch {
            s += inter[i * ch + c];
        }
        mono[i] = s / ch as f32;
    }
    Ok((mono, spec.sample_rate))
}

fn gcd(a: u32, b: u32) -> u32 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Port of torchaudio.functional.resample (sinc_interp_hann, lowpass_filter_width=6, rolloff=0.99).
fn resample(x: &[f32], orig: u32, new: u32) -> Vec<f32> {
    if orig == new {
        return x.to_vec();
    }
    let g = gcd(orig, new);
    let (o, n) = ((orig / g) as usize, (new / g) as usize);
    let lw = 6.0f64;
    let base = (o.min(n) as f64) * 0.99;
    let width = (lw * o as f64 / base).ceil() as usize;
    let klen = 2 * width + o;
    // kernel[j][i], computed like torchaudio (float32 dtype path), accumulated in f64 then cast.
    let mut kern = vec![0f32; n * klen];
    for j in 0..n {
        for i in 0..klen {
            let idx = (i as f32 - width as f32) / o as f32;
            let tt = -(j as f32) / n as f32 + idx;
            let mut t = (tt * base as f32) as f64;
            t = t.clamp(-lw, lw);
            let t32 = t as f32;
            let w = ((t32 as f64) * std::f64::consts::PI / lw / 2.0).cos().powi(2);
            let tp = t32 as f64 * std::f64::consts::PI;
            let s = if tp == 0.0 { 1.0 } else { tp.sin() / tp };
            kern[j * klen + i] = (s * w * (base / o as f64)) as f32;
        }
    }
    let len = x.len();
    let mut padded = vec![0f32; width + len + width + o];
    padded[width..width + len].copy_from_slice(x);
    let target = ((n as f64 * len as f64) / o as f64).ceil() as usize;
    let nblk = (padded.len() - klen) / o + 1;
    let mut out = Vec::with_capacity(nblk * n);
    for b in 0..nblk {
        let seg = &padded[b * o..b * o + klen];
        for j in 0..n {
            let k = &kern[j * klen..(j + 1) * klen];
            let mut acc = 0f32;
            for i in 0..klen {
                acc += seg[i] * k[i];
            }
            out.push(acc);
        }
    }
    out.truncate(target);
    out
}

// ---------- speechbrain Fbank ----------
struct Fbank {
    window: Vec<f32>,
    mel: Vec<f32>, // [N_STFT][N_MELS]
}

impl Fbank {
    fn new() -> Self {
        // torch.hamming_window(400) periodic
        let window = (0..N_FFT)
            .map(|i| (0.54 - 0.46 * (2.0 * std::f64::consts::PI * i as f64 / N_FFT as f64).cos()) as f32)
            .collect();
        let to_mel = |hz: f64| 2595.0 * (1.0 + hz / 700.0).log10();
        let to_hz = |m: f64| 700.0 * (10f64.powf(m / 2595.0) - 1.0);
        let (m0, m1) = (to_mel(0.0), to_mel(SR as f64 / 2.0));
        // torch.linspace in float32
        let hz: Vec<f32> = (0..N_MELS + 2)
            .map(|i| {
                let m = (m0 as f32) + ((m1 - m0) as f32) * (i as f32) / ((N_MELS + 1) as f32);
                to_hz(m as f64) as f32
            })
            .collect();
        let mut mel = vec![0f32; N_STFT * N_MELS];
        for k in 0..N_MELS {
            let fc = hz[k + 1];
            let band = hz[k + 1] - hz[k];
            for f in 0..N_STFT {
                let freq = (SR / 2) as f32 * f as f32 / (N_STFT - 1) as f32;
                let slope = (freq - fc) / band;
                let v = (slope + 1.0).min(1.0 - slope).max(0.0);
                mel[f * N_MELS + k] = v;
            }
        }
        Fbank { window, mel }
    }

    /// returns [T][80] row-major, sentence-mean-normalized
    fn features(&self, wav: &[f32], planner: &mut RealFftPlanner<f32>) -> Vec<f32> {
        let pad = N_FFT / 2;
        let len = wav.len();
        let nfr = 1 + len / HOP;
        let fft = planner.plan_fft_forward(N_FFT);
        let mut buf = fft.make_input_vec();
        let mut spec = fft.make_output_vec();
        let mut scratch = fft.make_scratch_vec();
        let mut out = vec![0f32; nfr * N_MELS];
        let mut pw = vec![0f32; N_STFT];
        for t in 0..nfr {
            let start = t * HOP; // in padded coords
            for i in 0..N_FFT {
                let p = start + i;
                let s = if p >= pad && p - pad < len { wav[p - pad] } else { 0.0 };
                buf[i] = s * self.window[i];
            }
            fft.process_with_scratch(&mut buf, &mut spec, &mut scratch).unwrap();
            for f in 0..N_STFT {
                pw[f] = spec[f].re * spec[f].re + spec[f].im * spec[f].im;
            }
            let row = &mut out[t * N_MELS..(t + 1) * N_MELS];
            for f in 0..N_STFT {
                let p = pw[f];
                if p == 0.0 { continue; }
                let m = &self.mel[f * N_MELS..(f + 1) * N_MELS];
                for k in 0..N_MELS {
                    row[k] += p * m[k];
                }
            }
        }
        // amplitude_to_DB: 10*log10(clamp(x,1e-10)), top_db=80 over whole utterance
        let mut mx = f32::NEG_INFINITY;
        for v in out.iter_mut() {
            *v = 10.0 * v.max(1e-10).log10();
            if *v > mx { mx = *v; }
        }
        let floor = mx - 80.0;
        for v in out.iter_mut() {
            if *v < floor { *v = floor; }
        }
        // sentence mean normalization per mel bin
        let mut mean = vec![0f64; N_MELS];
        for t in 0..nfr {
            for k in 0..N_MELS { mean[k] += out[t * N_MELS + k] as f64; }
        }
        for k in 0..N_MELS { mean[k] /= nfr as f64; }
        for t in 0..nfr {
            for k in 0..N_MELS { out[t * N_MELS + k] -= mean[k] as f32; }
        }
        out
    }
}

// ---------- model ----------
struct Ecapa {
    sess: ort::session::Session,
    fb: Fbank,
    planner: RealFftPlanner<f32>,
}

impl Ecapa {
    fn new(model: &str, threads: usize) -> Result<Self> {
        let sess = ort::session::Session::builder()?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)?
            .with_intra_threads(threads)?
            .with_inter_threads(1)?
            .commit_from_file(model)
            .with_context(|| format!("load model {model}"))?;
        Ok(Ecapa { sess, fb: Fbank::new(), planner: RealFftPlanner::new() })
    }
    fn embed16(&mut self, wav16: &[f32]) -> Result<Vec<f32>> {
        let feats = self.fb.features(wav16, &mut self.planner);
        let t = feats.len() / N_MELS;
        let x = ort::value::Tensor::from_array(([1usize, t, N_MELS], feats))?;
        let outs = self.sess.run(ort::inputs!["feats" => x])?;
        let (_, e) = outs["emb"].try_extract_tensor::<f32>()?;
        let n = e.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt().max(1e-12);
        Ok(e.iter().map(|v| (*v as f64 / n) as f32).collect())
    }
}

fn load16(path: &str) -> Result<Vec<f32>> {
    let (w, sr) = load_wav(path)?;
    Ok(resample(&w, sr, SR))
}
fn cos(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum()
}
fn r4(x: f64) -> f64 { (x * 1e4).round() / 1e4 }

fn usage() -> ! {
    eprintln!(
"galion-ecapa — speechbrain ECAPA speaker similarity in Rust (ONNX Runtime)
usage: galion-ecapa --ref REF.wav [f1.wav ...] [--model ecapa.onnx] [--threads N(<=2 default 2)] [--no-ceiling] [--out out.json] [--raw]
  prints JSON: {{ref, half_vs_half_ceiling, scores{{path:cos}}, mean, sd, min, max, n, timing}}
  cosine rounded to 4 dp unless --raw. Model default: $GALION_ECAPA_MODEL or {DEFAULT_MODEL}");
    std::process::exit(2)
}

fn main() -> Result<()> {
    let t0 = Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut refp = None;
    let mut model = std::env::var("GALION_ECAPA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let mut threads = 2usize;
    let mut ceiling = true;
    let mut raw = false;
    let mut out = None;
    let mut files = vec![];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--ref" => { i += 1; refp = args.get(i).cloned(); }
            "--model" => { i += 1; model = args.get(i).cloned().unwrap_or_else(|| usage()); }
            "--threads" => { i += 1; threads = args.get(i).and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()); }
            "--out" => { i += 1; out = args.get(i).cloned(); }
            "--no-ceiling" => ceiling = false,
            "--raw" => raw = true,
            "-h" | "--help" => usage(),
            s if s.starts_with("--") => { eprintln!("unknown flag {s}"); usage() }
            s => files.push(s.to_string()),
        }
        i += 1;
    }
    let Some(refp) = refp else { usage() };
    if threads == 0 { bail!("--threads must be >= 1"); }
    let rnd = |x: f64| if raw { x } else { r4(x) };

    let mut m = Ecapa::new(&model, threads)?;
    let t_load = t0.elapsed().as_secs_f64();
    let rw = load16(&refp)?;
    let re = m.embed16(&rw)?;
    let mut res = Map::new();
    res.insert("ref".into(), json!(refp));
    if ceiling {
        let h = rw.len() / 2;
        let a = m.embed16(&rw[..h])?;
        let b = m.embed16(&rw[h..])?;
        res.insert("half_vs_half_ceiling".into(), json!(rnd(cos(&a, &b))));
    }
    let mut scores = Map::new();
    let mut vals = vec![];
    for f in &files {
        let e = m.embed16(&load16(f)?)?;
        let c = rnd(cos(&e, &re));
        vals.push(c);
        scores.insert(f.clone(), json!(c));
    }
    res.insert("scores".into(), Value::Object(scores));
    let n = vals.len();
    res.insert("n".into(), json!(n));
    if n > 0 {
        let mean = vals.iter().sum::<f64>() / n as f64;
        let sd = if n > 1 { (vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt() } else { 0.0 };
        res.insert("mean".into(), json!(rnd(mean)));
        res.insert("sd".into(), json!(rnd(sd)));
        res.insert("min".into(), json!(vals.iter().cloned().fold(f64::INFINITY, f64::min)));
        res.insert("max".into(), json!(vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)));
    }
    res.insert("timing".into(), json!({"model_load_s": r4(t_load), "wall_s_total": r4(t0.elapsed().as_secs_f64())}));
    let s = serde_json::to_string_pretty(&Value::Object(res))?;
    if let Some(o) = out { std::fs::write(&o, &s)?; }
    println!("{s}");
    Ok(())
}
