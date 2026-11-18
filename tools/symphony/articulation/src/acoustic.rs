// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! In-process acoustic bricks via `praatfan_rust` (MIT, clean-room Praat in Rust).
//! Glue only: settings follow the Praat manual conventions, picked per voice
//! from the voice's own pitch (no voice-specific constants).

use crate::{to_ticks, Articulation, Brick, Section};
use praatfan_rust::Sound;

/// Read any PCM WAV, down-mix to mono (mean of channels), as f64.
pub fn read_wav_mono(path: &str) -> Result<(Vec<f64>, f64), String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{path}: {e}"))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f64> = match spec.sample_format {
        hound::SampleFormat::Float => r
            .samples::<f32>()
            .map(|s| s.map(|v| v as f64))
            .collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f64;
            r.samples::<i32>()
                .map(|s| s.map(|v| v as f64 / scale))
                .collect::<Result<_, _>>()
        }
    }
    .map_err(|e| format!("{path}: {e}"))?;
    let mono: Vec<f64> = raw
        .chunks(ch)
        .map(|c| c.iter().sum::<f64>() / c.len() as f64)
        .collect();
    Ok((mono, spec.sample_rate as f64))
}

/// Praat-manual conventions, chosen from the voice itself.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Settings {
    pub pitch_floor_hz: f64,
    pub pitch_ceiling_hz: f64,
    pub formant_ceiling_hz: f64,
}

fn voiced_f0(sound: &Sound, floor: f64, ceil: f64) -> Vec<f64> {
    let p = sound.to_pitch_ac(0.01, floor, ceil);
    let mut v: Vec<f64> = p.values().iter().copied().filter(|f| *f > 0.0).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

/// Two-pass pitch range (De Looze & Hirst): floor = 0.75*q25, ceiling = 1.5*q75 of a
/// wide first pass. Formant ceiling: 5500 Hz if median F0 >= 165 Hz, else 5000 Hz
/// (Praat manual "Sound: To Formant (burg)" guidance). Works for any voice.
pub fn settings_for(sound: &Sound) -> Settings {
    let wide = voiced_f0(sound, 60.0, 700.0);
    if wide.len() < 10 {
        return Settings {
            pitch_floor_hz: 75.0,
            pitch_ceiling_hz: 600.0,
            formant_ceiling_hz: 5500.0,
        };
    }
    let q = |p: f64| crate::quantile_sorted(&wide, p);
    let floor = (0.75 * q(0.25)).max(50.0);
    let ceil = (1.5 * q(0.75)).min(900.0);
    let med = q(0.5);
    Settings {
        pitch_floor_hz: floor,
        pitch_ceiling_hz: ceil,
        formant_ceiling_hz: if med >= 165.0 { 5500.0 } else { 5000.0 },
    }
}

/// Throat (F0, voicing, HNR) and mouth (F1..F4 in voiced ticks) tracks, plus the
/// formant-dispersion vocal-tract length (Fitch 1997: VTL = c / (2 * dF), dF = (F4-F1)/3).
pub fn analyse(samples: &[f64], sr: f64) -> (Articulation, Settings) {
    let sound = Sound::from_slice(samples, sr);
    let dur = samples.len() as f64 / sr;
    let mut a = Articulation::new(dur);
    let s = settings_for(&sound);
    let n = a.n_ticks;

    let pitch = sound.to_pitch_ac(0.01, s.pitch_floor_hz, s.pitch_ceiling_hz);
    let pt: Vec<f64> = pitch.times().to_vec();
    let pv: Vec<f64> = pitch.values().to_vec();
    let f0: Vec<f64> = pv
        .iter()
        .map(|f| if *f > 0.0 { *f } else { f64::NAN })
        .collect();
    let voicing: Vec<f64> = pv
        .iter()
        .map(|f| if *f > 0.0 { 1.0 } else { 0.0 })
        .collect();
    // Voicing fraction per tick (mean, not median).
    let mut vs = vec![(0.0f64, 0usize); n];
    for (t, v) in pt.iter().zip(&voicing) {
        let i = (t / crate::TICK_S).floor() as usize;
        if i < n {
            vs[i].0 += v;
            vs[i].1 += 1;
        }
    }
    let vfrac: Vec<Option<f64>> = vs
        .iter()
        .map(|(s, c)| if *c > 0 { Some(s / *c as f64) } else { None })
        .collect();
    let voiced_tick: Vec<bool> = vfrac.iter().map(|v| v.unwrap_or(0.0) >= 0.5).collect();
    let gate = |t: Vec<Option<f64>>| -> Vec<Option<f64>> {
        t.into_iter()
            .zip(&voiced_tick)
            .map(|(x, ok)| if *ok { x } else { None })
            .collect()
    };

    let f0_ticks = gate(to_ticks(&pt, &f0, n));
    // F0 in semitones re 100 Hz: comparable across voices on a log scale.
    let f0_st: Vec<Option<f64>> = f0_ticks
        .iter()
        .map(|x| x.map(|f| 12.0 * (f / 100.0).log2()))
        .collect();
    a.add(
        "f0_hz",
        Section::Throat,
        "Hz",
        "praatfan pitch ac",
        f0_ticks,
    );
    a.add(
        "f0_st",
        Section::Throat,
        "st re 100 Hz",
        "praatfan pitch ac",
        f0_st,
    );
    a.add(
        "voicing",
        Section::Throat,
        "fraction",
        "praatfan pitch ac",
        vfrac,
    );

    let hnr = sound.to_harmonicity_cc(0.01, s.pitch_floor_hz, 0.1, 1.0);
    let hv: Vec<f64> = hnr
        .values()
        .iter()
        .map(|v| if *v > -100.0 { *v } else { f64::NAN })
        .collect();
    a.add(
        "hnr_db",
        Section::Throat,
        "dB",
        "praatfan harmonicity cc",
        gate(to_ticks(&hnr.times().to_vec(), &hv, n)),
    );

    let fm = sound.to_formant_burg(0.01, 5, s.formant_ceiling_hz, 0.025, 50.0);
    let ft = fm.times().to_vec();
    let mut fk: Vec<Vec<Option<f64>>> = Vec::new();
    for k in 1..=4 {
        let v = fm.formant_values(k).to_vec();
        let t = gate(to_ticks(&ft, &v, n));
        fk.push(t.clone());
        a.add(
            &format!("f{k}_hz"),
            Section::Mouth,
            "Hz",
            "praatfan formant burg",
            t,
        );
    }
    for k in 1..=3 {
        let b = fm.bandwidth_values(k).to_vec();
        a.add(
            &format!("b{k}_hz"),
            Section::Mouth,
            "Hz",
            "praatfan formant burg",
            gate(to_ticks(&ft, &b, n)),
        );
    }
    // Formant dispersion per tick -> vocal tract length (cm), c = 35 000 cm/s.
    let vtl: Vec<Option<f64>> = (0..n)
        .map(|i| match (fk[0][i], fk[3][i]) {
            (Some(f1), Some(f4)) if f4 > f1 => Some(35000.0 / (2.0 * (f4 - f1) / 3.0)),
            _ => None,
        })
        .collect();
    a.add(
        "vtl_cm",
        Section::Mouth,
        "cm",
        "glue: Fitch 1997 formant dispersion",
        vtl,
    );

    let hop = (crate::TICK_S * sr).round().max(1.0) as usize;
    let rms: Vec<Option<f64>> = (0..n)
        .map(|i| {
            let a0 = i * hop;
            let b0 = (a0 + hop).min(samples.len());
            if b0 <= a0 {
                return None;
            }
            let e = samples[a0..b0].iter().map(|v| v * v).sum::<f64>() / (b0 - a0) as f64;
            Some(if e <= 1e-12 { -120.0 } else { 10.0 * e.log10() })
        })
        .collect();
    a.add("rms_dbfs", Section::Throat, "dBFS", "glue: tick rms", rms);

    a.bricks.push(Brick {
        name: "praatfan_rust".into(),
        version: "0.1.16 (git tag v0.1.16, 5950fb6f)".into(),
        licence: "MIT".into(),
        lang: "Rust".into(),
        process: "in process".into(),
    });
    (a, s)
}
