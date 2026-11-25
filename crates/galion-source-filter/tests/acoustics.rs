// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Generated probes. No audio file is read or written.

use galion_source_filter::{
    frame_samples, measure_frame_cost, Breath, BreathPhase, MomentState, MouthControls,
    SourceFilter, TractBaseline, VoiceBaseline, BREATH_FLOOR, FRAME_MS, NASAL_HI_HZ, NASAL_LO_HZ,
    NEUTRAL_TOLERANCE,
};

const SR: u32 = 24_000;
const N: usize = 4_096;

fn impulse(amp: f32) -> Vec<f32> {
    let mut buf = vec![0.0; N];
    buf[0] = amp;
    buf
}

fn render(controls: MouthControls, amp: f32) -> Vec<f32> {
    let mut filter = SourceFilter::samantha(SR);
    filter.set_controls(controls);
    let mut buf = impulse(amp);
    filter.process(&mut buf);
    let peak = buf.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(
        peak < 0.95,
        "probe clipped at {peak}; the frequency read would not be linear"
    );
    buf
}

fn power_at(samples: &[f32], freq: f32) -> f32 {
    let w = std::f32::consts::TAU * freq / SR as f32;
    let mut re = 0.0f32;
    let mut im = 0.0f32;
    for (i, sample) in samples.iter().enumerate() {
        let t = w * i as f32;
        re += sample * t.cos();
        im -= sample * t.sin();
    }
    let n = samples.len() as f32;
    let re = re / n;
    let im = im / n;
    re * re + im * im
}

fn peak_hz(samples: &[f32], lo: f32, hi: f32, step: f32) -> (f32, f32) {
    let steps = ((hi - lo) / step).round() as usize;
    let mut best_f = lo;
    let mut best_p = -1.0f32;
    for i in 0..=steps {
        let freq = lo + i as f32 * step;
        let power = power_at(samples, freq);
        if power > best_p {
            best_p = power;
            best_f = freq;
        }
    }
    (best_f, best_p)
}

fn tilt_ratio(controls: MouthControls) -> f32 {
    let buf = render(controls, 0.05);
    let low = power_at(&buf, 300.0);
    let high = power_at(&buf, 3_000.0);
    high / low
}

#[test]
fn neutral_controls_leave_the_signal_unchanged() {
    let mut probe = vec![0.0f32; 1_920];
    let mut state = 0x1234_5678u32;
    for sample in &mut probe {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *sample = (state as i32 as f32) * (0.4 / i32::MAX as f32);
    }
    for rate in [16_000u32, 22_050, 24_000] {
        let mut filter = SourceFilter::samantha(rate);
        let mut buf = probe.clone();
        filter.process(&mut buf);
        let err = buf
            .iter()
            .zip(&probe)
            .map(|(got, want)| (got - want).abs())
            .fold(0.0f32, f32::max);
        assert!(
            err <= NEUTRAL_TOLERANCE,
            "rate {rate} moved a neutral frame by {err}"
        );
        assert_eq!(buf, probe);
    }
}

#[test]
fn tension_raises_spectral_tilt() {
    let down = tilt_ratio(MouthControls {
        tension: -0.5,
        ..MouthControls::neutral()
    });
    let rest = tilt_ratio(MouthControls::neutral());
    let mid = tilt_ratio(MouthControls {
        tension: 0.5,
        ..MouthControls::neutral()
    });
    let up = tilt_ratio(MouthControls {
        tension: 1.0,
        ..MouthControls::neutral()
    });
    assert!(
        (rest - 1.0).abs() < 0.08,
        "neutral tilt ratio {rest} should stay near 1"
    );
    assert!(down < rest, "softer glottis {down} vs rest {rest}");
    assert!(rest < mid, "rest {rest} vs half tension {mid}");
    assert!(mid < up, "half tension {mid} vs full {up}");
    assert!(
        up > rest * 2.0,
        "full tension ratio {up} should clearly brighten"
    );
}

#[test]
fn breathiness_lowers_spectral_tilt() {
    let rest = tilt_ratio(MouthControls::neutral());
    let airy = tilt_ratio(MouthControls {
        breathiness: 1.0,
        ..MouthControls::neutral()
    });
    assert!(
        airy < rest * 0.5,
        "breathiness ratio {airy} should fall under rest {rest}"
    );
}

#[test]
fn openness_moves_f1_and_f2() {
    let rest = TractBaseline::SAMANTHA;
    let open = MouthControls {
        openness: 1.0,
        ..MouthControls::neutral()
    };
    let shut = MouthControls {
        openness: -1.0,
        ..MouthControls::neutral()
    };
    let (open_f1, open_f2) = rest.shifted(1.0);
    let (shut_f1, shut_f2) = rest.shifted(-1.0);
    let opened = render(open, 0.04);
    let closed = render(shut, 0.04);
    let (p1, _) = peak_hz(&opened, 200.0, 1_200.0, 4.0);
    let (p2, _) = peak_hz(&opened, 1_400.0, 3_200.0, 4.0);
    let (c1, _) = peak_hz(&closed, 100.0, 900.0, 4.0);
    let (c2, _) = peak_hz(&closed, 800.0, 2_200.0, 4.0);
    assert!(
        (p1 - open_f1).abs() + 40.0 < (p1 - rest.f1_hz).abs(),
        "F1 peak {p1} should sit nearer {open_f1} than rest {}",
        rest.f1_hz
    );
    assert!(
        (p2 - open_f2).abs() + 40.0 < (p2 - rest.f2_hz).abs(),
        "F2 peak {p2} should sit nearer {open_f2} than rest {}",
        rest.f2_hz
    );
    assert!(
        (p1 - rest.f1_hz).abs() > 80.0,
        "F1 should leave {0}, peak {p1}",
        rest.f1_hz
    );
    assert!(
        (p2 - rest.f2_hz).abs() > 80.0,
        "F2 should leave {0}, peak {p2}",
        rest.f2_hz
    );
    assert!(c1 < rest.f1_hz - 40.0, "closed F1 {c1} target {shut_f1}");
    assert!(c2 < rest.f2_hz - 40.0, "closed F2 {c2} target {shut_f2}");
}

fn half_power_width(samples: &[f32], lo: f32, hi: f32) -> f32 {
    let step = 4.0;
    let (peak_f, peak_p) = peak_hz(samples, lo, hi, step);
    let half = peak_p * 0.5;
    let mut left = peak_f;
    let mut freq = peak_f;
    while freq - step >= lo {
        freq -= step;
        if power_at(samples, freq) < half {
            break;
        }
        left = freq;
    }
    let mut right = peak_f;
    freq = peak_f;
    while freq + step <= hi {
        freq += step;
        if power_at(samples, freq) < half {
            break;
        }
        right = freq;
    }
    right - left
}

#[test]
fn openness_widens_the_formant() {
    let wide = render(
        MouthControls {
            openness: 1.0,
            ..MouthControls::neutral()
        },
        0.04,
    );
    let narrow = render(
        MouthControls {
            openness: 0.45,
            ..MouthControls::neutral()
        },
        0.04,
    );
    let wide_w = half_power_width(&wide, 200.0, 1_400.0);
    let narrow_w = half_power_width(&narrow, 200.0, 1_400.0);
    assert!(
        wide_w > narrow_w * 1.2,
        "open bandwidth {wide_w} Hz should exceed the smaller opening {narrow_w} Hz"
    );
}

#[test]
fn nasal_coupling_dips_the_nasal_bands() {
    let dry = impulse(0.1);
    let wet = render(
        MouthControls {
            nasal: 1.0,
            ..MouthControls::neutral()
        },
        0.1,
    );
    let drop_db = |freq: f32| -> f32 {
        let a = power_at(&dry, freq).max(1.0e-20);
        let b = power_at(&wet, freq).max(1.0e-20);
        10.0 * (b / a).log10()
    };
    let low = drop_db(NASAL_LO_HZ);
    let high = drop_db(NASAL_HI_HZ);
    let mid = drop_db(800.0);
    assert!(low < -8.0, "250 Hz dip {low} dB");
    assert!(high < -8.0, "1.5 kHz dip {high} dB");
    assert!(mid.abs() < 2.0, "800 Hz should stay put, moved {mid} dB");
}

#[test]
fn breath_noise_is_added_on_the_exhale_only() {
    let voice = VoiceBaseline::standin();
    let moment = MomentState::at(voice.pos);
    let exhale = Breath {
        phase: BreathPhase::Exhale,
        volume: 0.4,
        flow: -0.6,
    };
    let controls = MouthControls::from_moment(&voice, &moment, exhale, 0.0);
    assert!(controls.breath_noise >= BREATH_FLOOR);
    assert!(controls.tension.abs() < 1.0e-6);
    assert!(controls.openness.abs() < 1.0e-6);
    for phase in [
        BreathPhase::Rest,
        BreathPhase::Inhale,
        BreathPhase::Hold,
        BreathPhase::Speech,
    ] {
        let breath = Breath {
            phase,
            volume: 0.6,
            flow: -0.5,
        };
        let other = MouthControls::from_moment(&voice, &moment, breath, 0.0);
        assert_eq!(other.breath_noise, 0.0, "{phase:?}");
    }

    let width = frame_samples(SR);
    let mut filter = SourceFilter::samantha(SR);
    let mut buf = vec![0.0f32; width * 2];
    filter.process_frames(
        &mut buf,
        &[
            controls,
            MouthControls::from_moment(&voice, &moment, Breath::rest(), 0.0),
        ],
    );
    let (first, second) = buf.split_at(width);
    let rms = |chunk: &[f32]| -> f32 {
        (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt()
    };
    assert!(rms(first) > 1.0e-3, "exhale rms {}", rms(first));
    assert!(rms(first) < 0.2, "exhale rms {} is too hot", rms(first));
    let mean = first.iter().sum::<f32>() / first.len() as f32;
    let var = first.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / first.len() as f32;
    assert!(var > 1.0e-6, "exhale should be noise, variance {var}");
    assert!(
        second.iter().all(|s| *s == 0.0),
        "the frame after the exhale must be silent"
    );

    let mut tone = vec![0.0f32; width];
    for (i, sample) in tone.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        *sample = (std::f32::consts::TAU * 180.0 * t).sin() * 0.2;
    }
    let dry = tone.clone();
    let mut quiet = SourceFilter::samantha(SR);
    quiet.set_moment(
        &voice,
        &moment,
        Breath {
            phase: BreathPhase::Speech,
            volume: 0.5,
            flow: -0.3,
        },
        0.0,
    );
    quiet.process(&mut tone);
    assert_eq!(tone, dry);
}

#[test]
fn controls_change_on_the_80ms_boundary() {
    let width = frame_samples(SR);
    assert_eq!(width, (SR as usize) * (FRAME_MS as usize) / 1000);
    let mut buf = vec![0.0f32; width * 2];
    for (i, sample) in buf.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        *sample = (std::f32::consts::TAU * NASAL_LO_HZ * t).sin() * 0.25;
    }
    let dry = buf.clone();
    let mut filter = SourceFilter::samantha(SR);
    filter.process_frames(
        &mut buf,
        &[
            MouthControls::neutral(),
            MouthControls {
                nasal: 1.0,
                ..MouthControls::neutral()
            },
        ],
    );
    let (first, second) = buf.split_at(width);
    let (dry_a, dry_b) = dry.split_at(width);
    assert_eq!(first, dry_a);
    let rms = |chunk: &[f32]| -> f32 {
        (chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32).sqrt()
    };
    assert!(
        rms(second) < rms(dry_b) * 0.5,
        "nasal frame rms {} vs dry {}",
        rms(second),
        rms(dry_b)
    );
}

#[test]
fn the_moment_does_not_move_the_tract_baseline() {
    let mut filter = SourceFilter::samantha(SR);
    let before = filter.baseline();
    let voice = VoiceBaseline::standin();
    let bits = voice.identity_bits();
    let mut moment = MomentState::at(voice.pos);
    // Arousal is axis 1 on `galion-emotion-core`. This test does not re-export the index.
    moment.pos[1] = (voice.pos[1] + 0.3).min(1.0);
    filter.set_moment(&voice, &moment, Breath::rest(), 0.0);
    assert_eq!(filter.baseline().f1_hz.to_bits(), before.f1_hz.to_bits());
    assert_eq!(filter.baseline().f2_hz.to_bits(), before.f2_hz.to_bits());
    assert_eq!(filter.controls().openness, 0.0);
    assert_eq!(filter.controls().tension, 0.0);
    assert_eq!(filter.controls().breathiness, 0.0);
    assert_eq!(voice.identity_bits(), bits);
}

#[test]
fn realtime_factor_is_below_one() {
    let frames = 400u32;
    let neutral = measure_frame_cost(SR, frames, MouthControls::neutral());
    let full = measure_frame_cost(
        SR,
        frames,
        MouthControls {
            tension: 1.0,
            breathiness: 0.0,
            openness: 1.0,
            nasal: 1.0,
            breath_noise: 1.0,
        },
    );
    eprintln!(
        "frame-cost neutral sr={} n={} frames={} ns_per_frame={} audio_ns={} rtf={:.9}",
        neutral.sample_rate,
        neutral.samples_per_frame,
        neutral.frames,
        neutral.ns_per_frame,
        neutral.audio_ns,
        neutral.rtf
    );
    eprintln!(
        "frame-cost full sr={} n={} frames={} ns_per_frame={} audio_ns={} rtf={:.9}",
        full.sample_rate,
        full.samples_per_frame,
        full.frames,
        full.ns_per_frame,
        full.audio_ns,
        full.rtf
    );
    assert!(neutral.ns_per_frame > 0, "neutral frame was timed as zero");
    assert!(full.ns_per_frame > 0, "full frame was timed as zero");
    assert!(neutral.audio_ns == 80_000_000);
    assert!(
        neutral.rtf < 1.0,
        "neutral rtf {} ({} ns/frame)",
        neutral.rtf,
        neutral.ns_per_frame
    );
    assert!(
        full.rtf < 1.0,
        "full rtf {} ({} ns/frame)",
        full.rtf,
        full.ns_per_frame
    );
    assert!(full.samples_per_frame == 1_920);
}
