// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic contours only. Sines are built in the test. No audio file.

use horbm_ear::{
    Ear, EarConfig, EotConfig, FrameOut, ProsodyCue, Scorer, TurnMsg, FRAME_MS, FRAME_SAMPLES,
    SAMPLE_RATE,
};

const PI: f32 = std::f32::consts::PI;

fn sine(hz: f32, amp: f32) -> [f32; FRAME_SAMPLES] {
    let mut buf = [0.0f32; FRAME_SAMPLES];
    if hz <= 0.0 || amp <= 0.0 {
        return buf;
    }
    let step = 2.0 * PI * hz / SAMPLE_RATE as f32;
    let mut phase = 0.0f32;
    for sample in &mut buf {
        *sample = amp * phase.sin();
        phase += step;
    }
    buf
}

fn quiet_cue(pause_ms: u32) -> ProsodyCue {
    ProsodyCue {
        voiced: false,
        f0_hz: 0.0,
        energy: 0.0,
        speaking_rate_hz: 0.0,
        pause_ms,
    }
}

fn heard_cue(f0: f32, energy: f32, rate: f32) -> ProsodyCue {
    ProsodyCue {
        voiced: true,
        f0_hz: f0,
        energy,
        speaking_rate_hz: rate,
        pause_ms: 0,
    }
}

struct Step {
    msg: TurnMsg,
    voiced: bool,
    /// A yield on this step is a cut-in.
    holding: bool,
}

fn push_pcm(ear: &mut Ear, scorer: &mut Scorer, pcm: &[f32], partial: &str, holding: bool) -> Step {
    let frame = ear.push(pcm);
    let voiced = matches!(frame, FrameOut::Heard(ref heard) if heard.voiced);
    let msg = scorer.push(frame, partial);
    Step {
        msg,
        voiced,
        holding,
    }
}

/// Milliseconds from the last voiced step to the first yield, on the 80 ms grid.
fn latency_ms(steps: &[Step]) -> Option<u32> {
    let last_voiced = steps.iter().rposition(|step| step.voiced)?;
    let yield_at = steps
        .iter()
        .position(|step| matches!(step.msg, TurnMsg::Yield { .. }))?;
    Some(yield_at.saturating_sub(last_voiced) as u32 * FRAME_MS)
}

fn cut_ins(steps: &[Step]) -> u32 {
    steps
        .iter()
        .filter(|step| step.holding && matches!(step.msg, TurnMsg::Yield { .. }))
        .count() as u32
}

/// Linear interpolation. Rank is `p/100 * (n - 1)` on the sorted sample.
fn percentile(samples: &[u32], pct: f64) -> u32 {
    assert!(!samples.is_empty());
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (pct / 100.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let weight = rank - lo as f64;
    ((sorted[lo] as f64) * (1.0 - weight) + (sorted[hi] as f64) * weight).round() as u32
}

fn strong_fall(partial: &str, hz: &[f32]) -> (Vec<Step>, TurnMsg) {
    let mut ear = Ear::new();
    let mut scorer = Scorer::bundled();
    let amps = [0.30, 0.24, 0.18, 0.12];
    let mut steps = Vec::new();
    for (index, freq) in hz.iter().copied().enumerate() {
        let pcm = sine(freq, amps[index]);
        steps.push(push_pcm(&mut ear, &mut scorer, &pcm, partial, true));
    }
    let mut kind = TurnMsg::Hold;
    for _ in 0..2 {
        let pcm = sine(0.0, 0.0);
        let step = push_pcm(&mut ear, &mut scorer, &pcm, partial, false);
        if matches!(step.msg, TurnMsg::Yield { .. }) && matches!(kind, TurnMsg::Hold) {
            kind = step.msg;
        }
        steps.push(step);
    }
    (steps, kind)
}

fn rising_question(hz: &[f32]) -> (Vec<Step>, TurnMsg) {
    let partial = "is the kettle ready";
    let mut ear = Ear::new();
    let mut scorer = Scorer::bundled();
    let mut steps = Vec::new();
    for freq in hz {
        let pcm = sine(*freq, 0.22);
        steps.push(push_pcm(&mut ear, &mut scorer, &pcm, partial, true));
    }
    let pcm = sine(0.0, 0.0);
    let step = push_pcm(&mut ear, &mut scorer, &pcm, partial, false);
    let kind = step.msg;
    steps.push(step);
    (steps, kind)
}

#[test]
fn falling_pitch_yields_a_statement_and_a_rise_yields_a_question() {
    let partial = "the kettle is ready";
    let pitches = [
        [240.0, 200.0, 160.0, 130.0],
        [220.0, 180.0, 150.0, 120.0],
        [260.0, 210.0, 170.0, 140.0],
        [200.0, 170.0, 145.0, 115.0],
    ];
    let mut latencies = Vec::new();
    let mut cuts = 0u32;
    for hz in pitches {
        let (steps, kind) = strong_fall(partial, &hz);
        cuts += cut_ins(&steps);
        assert_eq!(kind, TurnMsg::Yield { question: false }, "{hz:?}");
        let latency = latency_ms(&steps).expect("yield");
        assert_eq!(latency, 80, "{hz:?}");
        latencies.push(latency);
        assert!(steps.iter().any(|step| step.voiced), "{hz:?}");
    }

    for hz in [[140.0, 165.0, 195.0, 230.0], [130.0, 155.0, 185.0, 220.0]] {
        let (steps, kind) = rising_question(&hz);
        cuts += cut_ins(&steps);
        assert_eq!(kind, TurnMsg::Yield { question: true }, "{hz:?}");
        let latency = latency_ms(&steps).expect("question");
        assert_eq!(latency, 80, "{hz:?}");
        latencies.push(latency);
    }

    // Mild fall: exact cues, so YIN error cannot move the 160 ms bucket.
    let mild_hz = [
        [200.0, 193.0, 186.0, 180.0],
        [210.0, 202.0, 194.0, 188.0],
        [190.0, 184.0, 178.0, 172.0],
    ];
    for hz in mild_hz {
        let mut scorer = Scorer::bundled();
        let mut steps = Vec::new();
        let energies = [0.20, 0.16, 0.12, 0.08];
        for (index, freq) in hz.iter().copied().enumerate() {
            let msg = scorer.push_cue(heard_cue(freq, energies[index], 3.0), partial);
            steps.push(Step {
                msg,
                voiced: true,
                holding: true,
            });
        }
        for (n, pause) in [80, 160].into_iter().enumerate() {
            let msg = scorer.push_cue(quiet_cue(pause), partial);
            steps.push(Step {
                msg,
                voiced: false,
                holding: n == 0,
            });
        }
        cuts += cut_ins(&steps);
        assert_eq!(
            steps.last().map(|step| step.msg),
            Some(TurnMsg::Yield { question: false })
        );
        assert_eq!(latency_ms(&steps), Some(160));
        latencies.push(160);
    }

    assert_eq!(cuts, 0);
    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    assert_eq!(p50, 80);
    assert_eq!(p95, 160);
    println!(
        "eot measure: trials={} decision_p50_ms={p50} decision_p95_ms={p95} false_cut_ins={cuts} frame_ms={FRAME_MS} result=pass",
        latencies.len()
    );
}

#[test]
fn a_filler_pause_and_an_unfinished_word_do_not_cut_in() {
    let mut cuts = 0u32;
    for partial in ["I was going to um...", "the kettle and"] {
        let mut ear = Ear::new();
        let mut scorer = Scorer::bundled();
        let contour = if partial.ends_with("and") {
            [240.0, 200.0, 160.0, 130.0]
        } else {
            [150.0, 156.0, 148.0, 154.0]
        };
        let amps = if partial.ends_with("and") {
            [0.30, 0.24, 0.18, 0.12]
        } else {
            [0.18, 0.18, 0.18, 0.18]
        };
        let mut steps = Vec::new();
        for (index, freq) in contour.iter().copied().enumerate() {
            let pcm = sine(freq, amps[index]);
            steps.push(push_pcm(&mut ear, &mut scorer, &pcm, partial, true));
        }
        for _ in 0..5 {
            let pcm = sine(0.0, 0.0);
            steps.push(push_pcm(&mut ear, &mut scorer, &pcm, partial, true));
        }
        cuts += cut_ins(&steps);
        assert!(
            steps
                .iter()
                .all(|step| !matches!(step.msg, TurnMsg::Yield { .. })),
            "{partial}"
        );
        assert!(steps.iter().any(|step| step.voiced), "{partial}");
    }

    // Flat finished word, short pause. Pause alone is not an end while sensing is on.
    let mut ear = Ear::new();
    let mut scorer = Scorer::bundled();
    let partial = "the kettle is ready";
    for _ in 0..4 {
        let pcm = sine(180.0, 0.22);
        let step = push_pcm(&mut ear, &mut scorer, &pcm, partial, true);
        cuts += u32::from(step.holding && matches!(step.msg, TurnMsg::Yield { .. }));
        assert_eq!(step.msg, TurnMsg::Hold);
    }
    for _ in 0..2 {
        let pcm = sine(0.0, 0.0);
        let step = push_pcm(&mut ear, &mut scorer, &pcm, partial, true);
        cuts += u32::from(step.holding && matches!(step.msg, TurnMsg::Yield { .. }));
        assert_eq!(step.msg, TurnMsg::Hold);
    }
    assert_eq!(cuts, 0);
    println!("eot measure: filler_and_flat false_cut_ins={cuts} result=pass");
}

#[test]
fn sensing_off_is_pause_only_for_deny_and_the_env_value() {
    let tone = sine(180.0, 0.22);
    let quiet = sine(0.0, 0.0);
    for mut ear in [
        Ear::from_env(Some("0")),
        Ear::from_env(Some("off")),
        Ear::from_env(Some("deny")),
        Ear::from_config(EarConfig { sense: false }),
    ] {
        assert!(!ear.sensing());
        let mut scorer = Scorer::bundled();
        for pcm in [&tone, &tone, &quiet, &quiet] {
            let frame = ear.push(pcm);
            assert!(matches!(frame, FrameOut::Denied));
            assert_eq!(
                scorer.push(frame, "I was going to um..."),
                TurnMsg::PauseOnly
            );
        }
        assert_eq!(ear.frames_analysed(), 0);
        assert_eq!(ear.retained_samples(), 0);
    }

    let mut ear = Ear::new();
    let mut scorer = Scorer::bundled();
    let frame = ear.push(&tone);
    assert!(matches!(frame, FrameOut::Heard(_)));
    assert_eq!(scorer.push(frame, "ready"), TurnMsg::Hold);
    ear.deny();
    assert!(!ear.sensing());
    let frame = ear.push(&quiet);
    assert!(matches!(frame, FrameOut::Denied));
    assert_eq!(scorer.push(frame, "ready"), TurnMsg::PauseOnly);

    let cfg = EotConfig::bundled();
    assert_eq!(cfg.yield_pause_ms, FRAME_MS);
    println!("eot measure: sensing_off=PauseOnly deny=PauseOnly env=0|off|deny result=pass");
}

#[test]
fn no_partial_yields_a_falling_sine_at_the_mild_pause() {
    let mut ear = Ear::new();
    let mut scorer = Scorer::bundled();
    for (hz, amp) in [(240.0, 0.30), (200.0, 0.24), (160.0, 0.18), (130.0, 0.12)] {
        let frame = ear.push(&sine(hz, amp));
        assert_eq!(scorer.push_prosody(frame), TurnMsg::Hold, "{hz}");
    }
    let first = ear.push(&sine(0.0, 0.0));
    assert_eq!(scorer.push_prosody(first), TurnMsg::Hold);
    let second = ear.push(&sine(0.0, 0.0));
    assert_eq!(
        scorer.push_prosody(second),
        TurnMsg::Yield { question: false }
    );
}
