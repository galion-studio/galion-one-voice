//! Per-file articulation analysis: builds all frame tracks and the profile JSON.
use crate::{audio, dsp, formant, pitch, stats::*};
use serde_json::{json, Value};

pub const HOP: f64 = 0.01;
pub const FS_AN: u32 = 16000; // pitch / intensity / MFCC
pub const FS_CONS: u32 = 24000; // consonant analysis

pub struct Opts { pub fmin: f64, pub fmax: f64, pub ceiling: Option<f64>, pub min_pause: f64, pub min_dip: f64, pub sil_db: f64, pub dbg_peaks: bool, pub frames: bool, pub beta_mean: f64 }
impl Default for Opts { fn default() -> Self { Opts { fmin: 65.0, fmax: 600.0, ceiling: None, min_pause: 0.3, min_dip: 2.0, sil_db: 25.0, dbg_peaks: false, frames: false, beta_mean: 0.2 } } }

#[derive(Clone)]
pub struct Vowel { pub s: usize, pub e: usize, pub peak: usize, pub f: [f64; 3] }
pub struct Burst { pub t: f64, pub rel_db: f64, pub vot: Option<f64>, pub centroid: f64 }

pub struct Analysis {
    pub path: String, pub fs: u32, pub dur: f64,
    pub times: Vec<f64>, pub int_db: Vec<f64>, pub sil_thr: f64, pub speech: Vec<bool>,
    pub f0: Vec<f64>, pub vprob: Vec<f64>, pub f0_yin: Vec<f64>,
    pub form: Vec<[f64; 3]>, pub ceiling: f64,
    pub mfcc: Vec<[f64; dsp::NCEP]>,
    pub nuclei: Vec<usize>, pub vowels: Vec<Vowel>,
    pub span: (usize, usize), pub pauses: Vec<(f64, f64)>,
    pub fric: Vec<[f64; 4]>, pub fric_frames: Vec<usize>, pub bursts: Vec<Burst>, pub hf_tilt_db: f64,
    pub profile: Value,
}

fn summ(v: &[f64]) -> Value {
    let n = v.iter().filter(|x| x.is_finite()).count();
    if n == 0 { return json!({"n":0}); }
    json!({"n": n, "mean": r2(mean(v)), "median": r2(median(v)), "sd": r2(sd(v)), "p10": r2(quantile(v,0.1)), "p90": r2(quantile(v,0.9)), "min": r2(quantile(v,0.0)), "max": r2(quantile(v,1.0))})
}
pub fn r2(x: f64) -> Value { if x.is_finite() { json!((x * 1000.0).round() / 1000.0) } else { Value::Null } }

pub fn analyze(path: &str, o: &Opts) -> Result<Analysis, String> {
    let a = audio::load_wav(path)?;
    let dur = a.x.len() as f64 / a.fs as f64;
    let x16 = audio::resample(&a.x, a.fs, FS_AN);
    let nf = ((dur - 0.0) / HOP).floor().max(1.0) as usize;
    let times: Vec<f64> = (0..nf).map(|i| i as f64 * HOP + HOP / 2.0).collect();
    let c16: Vec<usize> = times.iter().map(|t| (t * FS_AN as f64).round() as usize).collect();
    let int_db = dsp::intensity_db(&x16, FS_AN, &c16);
    let p99 = quantile(&int_db, 0.99);
    let sil_thr = (p99 - o.sil_db).max(quantile(&int_db, 0.0));
    let speech: Vec<bool> = int_db.iter().map(|&v| v > sil_thr).collect();
    let xlp = dsp::lowpass_zp(&x16, FS_AN, 1000.0);
    let pt = pitch::track(&xlp, FS_AN, &c16, &speech, o.fmin, o.fmax, o.beta_mean);
    let f0m = median(&pt.f0.iter().cloned().filter(|&f| f > 0.0).collect::<Vec<_>>());
    let ceiling = o.ceiling.unwrap_or(if f0m.is_finite() && f0m > 160.0 { 5500.0 } else { 5000.0 });
    let fsf = (2.0 * ceiling).round() as u32;
    let xf = audio::resample(&a.x, a.fs, fsf);
    let ft = formant::track(&xf, fsf, &times, 5, ceiling);
    let form: Vec<[f64; 3]> = ft.frames.iter().map(|f| f.f).collect();
    let mfcc = dsp::mfcc(&x16, FS_AN, &c16);

    // speech span & pauses
    let first = speech.iter().position(|&s| s).unwrap_or(0);
    let last = speech.iter().rposition(|&s| s).unwrap_or(nf.saturating_sub(1));
    let mut pauses = Vec::new();
    let mut i = first;
    while i <= last {
        if !speech[i] { let s = i; while i <= last && !speech[i] { i += 1; } let d = (i - s) as f64 * HOP; if d >= o.min_pause { pauses.push((s as f64 * HOP, d)); } } else { i += 1; }
    }
    // syllable nuclei (de Jong & Wempe 2009 style)
    let lv: Vec<bool> = (0..nf).map(|j| pt.f0[j] > 0.0 || pt.f0_yin[j] > 0.0 || pt.vprob[j] > 0.2).collect(); // lenient voicing for nuclei
    let voiced = |k: usize| -> bool { let lo = k.saturating_sub(2); let hi = (k + 2).min(nf - 1); (lo..=hi).any(|j| lv[j]) };
    let mut peaks: Vec<usize> = (1..nf.saturating_sub(1)).filter(|&k| int_db[k] > sil_thr && int_db[k] >= int_db[k - 1] && int_db[k] > int_db[k + 1]).collect();
    // merge peaks without sufficient dip: keep the higher one
    let mut kept: Vec<usize> = Vec::new();
    for p in peaks.drain(..) {
        if let Some(&q) = kept.last() {
            let dip = int_db[q..=p].iter().cloned().fold(f64::INFINITY, f64::min);
            if int_db[q].min(int_db[p]) - dip < o.min_dip { if int_db[p] > int_db[q] { kept.pop(); kept.push(p); } continue; }
        }
        kept.push(p);
    }
    let nuclei: Vec<usize> = kept.into_iter().filter(|&k| voiced(k)).collect();
    // vowel segments around nuclei
    let vowels: Vec<Vowel> = nuclei.iter().map(|&k| {
        let lim = int_db[k] - 3.0;
        let (mut s, mut e) = (k, k);
        while s > 0 && k - s < 10 && lv[s - 1] && int_db[s - 1] >= lim { s -= 1; }
        while e + 1 < nf && e - k < 10 && lv[e + 1] && int_db[e + 1] >= lim { e += 1; }
        let mut f = [f64::NAN; 3];
        for d in 0..3 { f[d] = median(&(s..=e).map(|j| form[j][d]).collect::<Vec<_>>()); }
        Vowel { s, e, peak: k, f }
    }).collect();

    // consonants at 24 kHz
    let fs_c = a.fs.min(FS_CONS);
    let xc = audio::resample(&a.x, a.fs, fs_c);
    let hi_band = (fs_c as f64 / 2.0 - 500.0).min(11000.0);
    let wl = (0.020 * fs_c as f64) as usize; let mut nfft = 1; while nfft < wl { nfft <<= 1; }
    let st = dsp::Stft::new(dsp::hann(wl), nfft);
    let mut p = Vec::new();
    let mut fric = Vec::new(); let mut fric_frames = Vec::new();
    let mut tilt = Vec::new();
    for k in 0..nf {
        if !speech[k] { continue; }
        let c = (times[k] * fs_c as f64).round() as isize;
        st.power(&xc, c, &mut p, 0.0);
        let tot = dsp::band_energy(&p, fs_c, nfft, 60.0, hi_band);
        let hf = dsp::band_energy(&p, fs_c, nfft, 2500.0, hi_band);
        let lf = dsp::band_energy(&p, fs_c, nfft, 60.0, 4000.0);
        let h4 = dsp::band_energy(&p, fs_c, nfft, 4000.0, hi_band);
        if lf > 0.0 && h4 > 0.0 && pt.f0[k] > 0.0 { tilt.push(10.0 * (h4 / lf).log10()); }
        if pt.f0[k] == 0.0 && pt.vprob[k] < 0.5 && tot > 0.0 && hf / tot > 0.5 {
            fric.push(dsp::moments(&p, fs_c, nfft, 500.0, hi_band)); fric_frames.push(k);
        }
    }
    // plosive bursts: abrupt broadband (pre-emphasised) onsets after a closure
    let bw = (0.005 * fs_c as f64) as usize; let bh = bw / 2;
    let nb = xc.len() / bh;
    let e: Vec<f64> = (0..nb).map(|n| {
        let s = n * bh; let mut acc = 0.0;
        for i in s.max(1)..(s + bw).min(xc.len()) { let v = (xc[i] - 0.97 * xc[i - 1]) as f64; acc += v * v; }
        10.0 * (acc / bw as f64 + 1e-12).log10()
    }).collect();
    let eref = quantile(&e, 0.99);
    let wl2 = (0.010 * fs_c as f64) as usize; let mut nf2 = 1; while nf2 < wl2 { nf2 <<= 1; }
    let st2 = dsp::Stft::new(dsp::hann(wl2), nf2);
    let mut bursts: Vec<Burst> = Vec::new();
    let mut n = 12;
    while n + 2 < nb {
        let pre = e[n - 12..n - 2].iter().sum::<f64>() / 10.0;
        let premax = e[n - 12..n - 2].iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if e[n] - pre >= 15.0 && e[n] - e[n - 1] >= 6.0 && e[n] > eref - 35.0 && premax < eref - 25.0 {
            let mut pk = n; for q in n..(n + 3).min(nb) { if e[q] > e[pk] { pk = q; } }
            let t = (pk * bh + bw / 2) as f64 / fs_c as f64;
            if bursts.last().map_or(true, |b| t - b.t > 0.04) {
                let fi = ((t - HOP / 2.0) / HOP).round().max(0.0) as usize;
                let vot = if fi < nf && pt.f0[fi.min(nf - 1)] > 0.0 && pt.f0[fi.saturating_sub(1)] > 0.0 { Some(0.0) } else {
                    (fi..nf.min(fi + 16)).find(|&j| pt.f0[j] > 0.0).map(|j| (times[j] - HOP / 2.0 - t).max(0.0))
                };
                st2.power(&xc, (t * fs_c as f64) as isize, &mut p, 0.0);
                let cen = dsp::moments(&p, fs_c, nf2, 500.0, hi_band)[0];
                bursts.push(Burst { t, rel_db: e[pk] - eref, vot, centroid: cen });
            }
            n = pk + 8; continue;
        }
        n += 1;
    }
    let hf_tilt_db = median(&tilt);
    let mut an = Analysis { path: path.to_string(), fs: a.fs, dur, times, int_db, sil_thr, speech, f0: pt.f0, vprob: pt.vprob, f0_yin: pt.f0_yin, form, ceiling, mfcc, nuclei, vowels, span: (first, last), pauses, fric, fric_frames, bursts, hf_tilt_db, profile: Value::Null };
    an.profile = profile(&an, o);
    Ok(an)
}

fn st_stats(f: &[f64]) -> (f64, f64) {
    let v: Vec<f64> = f.iter().cloned().filter(|&x| x > 0.0).collect();
    if v.is_empty() { return (f64::NAN, f64::NAN); }
    let m = median(&v);
    let s: Vec<f64> = v.iter().map(|x| 12.0 * (x / m).log2()).collect();
    (sd(&s), quantile(&s, 0.95) - quantile(&s, 0.05))
}

pub fn profile(a: &Analysis, o: &Opts) -> Value {
    let nf = a.times.len();
    let vf: Vec<f64> = a.f0.iter().cloned().filter(|&f| f > 0.0).collect();
    let yf: Vec<f64> = a.f0_yin.iter().cloned().filter(|&f| f > 0.0).collect();
    let nsp = a.speech.iter().filter(|&&s| s).count().max(1);
    let (sdst, rngst) = st_stats(&a.f0);
    let vfr: Vec<usize> = (0..nf).filter(|&k| a.f0[k] > 0.0 && a.speech[k]).collect();
    let fcol = |d: usize| -> Vec<f64> { vfr.iter().map(|&k| a.form[k][d]).collect() };
    let vcol = |d: usize| -> Vec<f64> { a.vowels.iter().map(|v| v.f[d]).collect() };
    let pts: Vec<(f64, f64)> = a.vowels.iter().map(|v| (v.f[1], v.f[0])).collect();
    let (f1v, f2v) = (vcol(0), vcol(1));
    let (a1, b1, a2, b2) = (quantile(&f1v, 0.05), quantile(&f1v, 0.95), quantile(&f2v, 0.05), quantile(&f2v, 0.95));
    let trimmed: Vec<(f64, f64)> = pts.iter().cloned().filter(|p| p.1 >= a1 && p.1 <= b1 && p.0 >= a2 && p.0 <= b2).collect();
    let bark: Vec<(f64, f64)> = pts.iter().map(|p| (hz_to_bark(p.0), hz_to_bark(p.1))).filter(|p| p.0.is_finite() && p.1.is_finite()).collect();
    let (cx, cy) = (mean(&bark.iter().map(|p| p.0).collect::<Vec<_>>()), mean(&bark.iter().map(|p| p.1).collect::<Vec<_>>()));
    let disp = mean(&bark.iter().map(|p| ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()).collect::<Vec<_>>());
    let span_s = (a.span.1 as f64 - a.span.0 as f64 + 1.0) * HOP;
    let ptot: f64 = a.pauses.iter().map(|p| p.1).sum();
    let phon = (span_s - ptot).max(1e-6);
    let nsyl = a.nuclei.len() as f64;
    let pd: Vec<f64> = a.pauses.iter().map(|p| p.1).collect();
    let fm = |d: usize| -> Vec<f64> { a.fric.iter().map(|m| m[d]).collect() };
    let sib: Vec<f64> = a.fric.iter().map(|m| m[0]).filter(|&c| c > 4000.0).collect();
    let bdb: Vec<f64> = a.bursts.iter().map(|b| b.rel_db).collect();
    let bc: Vec<f64> = a.bursts.iter().map(|b| b.centroid).collect();
    let vots: Vec<f64> = a.bursts.iter().filter_map(|b| b.vot).filter(|&v| v > 0.0).map(|v| v * 1000.0).collect();
    let nvot = a.bursts.iter().filter(|b| b.vot.is_some()).count().max(1);
    let longlag = a.bursts.iter().filter(|b| b.vot.map_or(false, |v| v > 0.025)).count() as f64 / nvot as f64;
    let spint: Vec<f64> = (0..nf).filter(|&k| a.speech[k]).map(|k| a.int_db[k]).collect();
    let mut v = json!({
        "file": a.path, "duration_s": r2(a.dur), "sample_rate": a.fs,
        "analysis": {"hop_s": HOP, "formant_ceiling_hz": a.ceiling, "f0_range_hz": [o.fmin, o.fmax], "silence_threshold_db": r2(a.sil_thr), "min_pause_s": o.min_pause},
        "f0": {
            "pyin": {"median_hz": r2(median(&vf)), "mean_hz": r2(mean(&vf)), "p5_hz": r2(quantile(&vf,0.05)), "p95_hz": r2(quantile(&vf,0.95)), "sd_st": r2(sdst), "range_p5_p95_st": r2(rngst), "voiced_frac_of_speech": r2(vfr.len() as f64 / nsp as f64)},
            "yin": {"median_hz": r2(median(&yf)), "voiced_frac_of_speech": r2(yf.len() as f64 / nsp as f64)}
        },
        "formants_voiced_frames": {"n": vfr.len(), "f1": summ(&fcol(0)), "f2": summ(&fcol(1)), "f3": summ(&fcol(2))},
        "vowels": {"n": a.vowels.len(), "f1": summ(&f1v), "f2": summ(&f2v), "f3": summ(&vcol(2))},
        "vowel_space": {
            "n_vowels": a.vowels.len(), "hull_area_hz2": r2(hull_area(&pts)), "hull_area_trimmed_hz2": r2(hull_area(&trimmed)),
            "hull_area_bark2": r2(hull_area(&bark)), "hull_area_trimmed_bark2": r2(hull_area(&trimmed.iter().map(|p| (hz_to_bark(p.0), hz_to_bark(p.1))).collect::<Vec<_>>())), "f1_range_p5_p95_hz": r2(b1 - a1), "f2_range_p5_p95_hz": r2(b2 - a2), "dispersion_bark": r2(disp)
        },
        "rate": {"n_syllables": a.nuclei.len(), "speech_span_s": r2(span_s), "phonation_time_s": r2(phon), "articulation_rate_syl_s": r2(nsyl / phon), "speech_rate_syl_s": r2(nsyl / span_s), "mean_syllable_dur_s": r2(phon / nsyl.max(1.0))},
        "pauses": {"n": a.pauses.len(), "per_min": r2(a.pauses.len() as f64 / span_s * 60.0), "total_s": r2(ptot), "pause_ratio": r2(ptot / span_s), "dur": summ(&pd),
                    "mean_run_syl": r2(nsyl / (a.pauses.len() as f64 + 1.0)), "durations_s": pd.iter().map(|d| r2(*d)).collect::<Vec<_>>()},
        "consonants": {
            "fricative": {"n_frames": a.fric.len(), "frac_of_speech": r2(a.fric.len() as f64 / nsp as f64), "centroid_hz": summ(&fm(0)), "sd_hz": summ(&fm(1)), "skewness": summ(&fm(2)), "kurtosis": summ(&fm(3)),
                           "sibilant_frames": sib.len(), "sibilant_centroid_hz": r2(median(&sib))},
            "plosive": {"n_bursts": a.bursts.len(), "per_s": r2(a.bursts.len() as f64 / phon), "burst_rel_db": summ(&bdb), "burst_centroid_hz": summ(&bc), "vot_ms": summ(&vots), "long_lag_frac": r2(longlag)},
            "hf_tilt_4k_db": r2(a.hf_tilt_db)
        },
        "intensity": {"dynamic_range_p5_p95_db": r2(quantile(&spint,0.95) - quantile(&spint,0.05))}
    });
    if o.frames {
        v["frames"] = json!({
            "t": a.times.iter().map(|t| r2(*t)).collect::<Vec<_>>(),
            "f0": a.f0.iter().map(|f| r2(*f)).collect::<Vec<_>>(),
            "f0_yin": a.f0_yin.iter().map(|f| r2(*f)).collect::<Vec<_>>(),
            "int_db": a.int_db.iter().map(|f| r2(*f)).collect::<Vec<_>>(),
            "f1": a.form.iter().map(|f| r2(f[0])).collect::<Vec<_>>(),
            "f2": a.form.iter().map(|f| r2(f[1])).collect::<Vec<_>>(),
            "f3": a.form.iter().map(|f| r2(f[2])).collect::<Vec<_>>(),
            "nuclei_t": a.nuclei.iter().map(|&k| r2(a.times[k])).collect::<Vec<_>>(),
            "bursts_t": a.bursts.iter().map(|b| r2(b.t)).collect::<Vec<_>>()
        });
    }
    v
}
