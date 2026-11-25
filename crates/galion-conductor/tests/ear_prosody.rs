// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic PCM through the ear, the emotion core, and this conductor.
//!
//! `galion-conductor` is its own workspace, so the root workspace cannot
//! name it. This test is the join.

use galion_conductor::{Conductor, Ear, FakeBrain, TickEvent};
use horbm_ear::link::{push_frame, FloorCue, ACTIVITY_MARK};
use horbm_ear::port::{EmotionCore, VoiceBaseline};
use horbm_ear::{Ear as ProsodyEar, FrameOut, FRAME_SAMPLES, SAMPLE_RATE};

const PI: f32 = std::f32::consts::PI;

fn sine(hz: f32, amp: f32) -> [f32; FRAME_SAMPLES] {
    let mut buf = [0.0f32; FRAME_SAMPLES];
    let step = 2.0 * PI * hz / SAMPLE_RATE as f32;
    let mut phase = 0.0f32;
    for s in &mut buf {
        *s = amp * phase.sin();
        phase += step;
    }
    buf
}

fn drive(
    prosody: &mut ProsodyEar,
    core: &mut EmotionCore,
    conductor: &mut Conductor<FakeBrain>,
    baseline: VoiceBaseline,
    pcm: &[f32],
) -> (FrameOut, galion_conductor::TickOut) {
    let out = push_frame(prosody, core, &mut (), baseline, pcm).expect("link");
    let tick = match out.cue {
        Some(FloorCue::Voiced) => conductor.tick(Ear::Speech(ACTIVITY_MARK), None),
        Some(FloorCue::Quiet) => conductor.tick(Ear::Quiet, None),
        None => panic!("the pipe did not cue the conductor"),
    };
    (out.frame, tick)
}

#[test]
fn voiced_sine_steps_the_core_and_the_conductor_commits() {
    let mut prosody = ProsodyEar::new();
    assert!(prosody.sensing());
    let mut core = EmotionCore::at_rest();
    let mut conductor = Conductor::new(FakeBrain::new());
    let baseline = VoiceBaseline::standin();
    let tone = sine(180.0, 0.22);
    let quiet = [0.0f32; FRAME_SAMPLES];

    for _ in 0..2 {
        let (frame, tick) = drive(&mut prosody, &mut core, &mut conductor, baseline, &tone);
        let frame = match frame {
            FrameOut::Heard(frame) => frame,
            other => panic!("heard frame, got {other:?}"),
        };
        assert!(frame.voiced);
        assert!((frame.f0_hz - 180.0).abs() < 8.0, "f0 {}", frame.f0_hz);
        assert!(frame.energy > 0.1);
        assert_eq!(frame.pause_ms, 0);
        assert!(tick.event == TickEvent::None);
    }
    assert_eq!(core.frames(), 2);
    assert!(!core.inbound_mixed());

    let mut saw_eager = false;
    let mut saw_commit = false;
    for _ in 0..3 {
        let (frame, tick) = drive(&mut prosody, &mut core, &mut conductor, baseline, &quiet);
        assert!(matches!(frame, FrameOut::Heard(_)));
        if tick.event == TickEvent::EagerEndOfTurn {
            saw_eager = true;
        }
        if tick.event == TickEvent::Committed {
            saw_commit = true;
            let clause = tick.clause.expect("clause");
            assert_eq!(clause.clause, "I don't know.");
            assert!(!clause.clause.contains("180"));
        }
    }
    assert!(saw_eager, "conductor did not end the turn");
    assert!(saw_commit, "conductor did not release the clause");
    assert_eq!(core.frames(), 5);

    let mut prosody = ProsodyEar::from_env(Some("off"));
    let mut core = EmotionCore::at_rest();
    let mut conductor = Conductor::new(FakeBrain::new());
    for _ in 0..4 {
        let (frame, tick) = drive(&mut prosody, &mut core, &mut conductor, baseline, &tone);
        assert!(matches!(frame, FrameOut::Denied));
        assert_eq!(tick.event, TickEvent::None);
        assert_eq!(tick.brain_calls, 0);
    }
    assert_eq!(core.frames(), 0);
    assert_eq!(prosody.frames_analysed(), 0);
    assert_eq!(prosody.retained_samples(), 0);
}
