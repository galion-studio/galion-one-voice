//! Built-in blends. Samantha may include seductive. Jarvis never does.

use crate::frame::NebulaFrame;
use crate::shade::Voice;
use nebula_elements::DEMO;

pub fn frame_at(voice: Voice, time: f32) -> NebulaFrame {
    let period = 8.0f32;
    let stage = (time / period).floor() as usize;
    let local = (time / period).fract();
    let blend = local.clamp(0.0, 1.0);
    let sequence: &[&[&str]] = match voice {
        Voice::Samantha => &[
            &["neutral"],
            &["calm", "tender"],
            &["happy"],
            &["amused", "happy"],
            &["excited"],
            &["surprised"],
            &["tender", "seductive"],
            &["sad", "tender"],
            &["angry"],
            &["fearful"],
            &["disgusted"],
            &["disappointed", "sad"],
        ],
        Voice::Jarvis => &[
            &["neutral"],
            &["calm"],
            &["happy"],
            &["amused"],
            &["excited"],
            &["surprised", "fearful"],
            &["tender"],
            &["sad"],
            &["angry"],
            &["disgusted"],
            &["disappointed"],
            &["calm", "neutral"],
        ],
    };
    let current = sequence[stage % sequence.len()];
    let next = sequence[(stage + 1) % sequence.len()];
    let mut frame = NebulaFrame::neutral(voice);
    frame.weights = [0.0; 13];
    frame.elements = [""; 13];
    paint(&mut frame, current, 1.0 - smooth(blend));
    paint(&mut frame, next, smooth(blend));
    if voice == Voice::Jarvis {
        if let Some(i) = DEMO.iter().position(|l| l.samantha_only) {
            frame.weights[i] = 0.0;
        }
    }
    let sum: f32 = frame.weights.iter().sum();
    if sum > 1e-4 {
        for w in &mut frame.weights {
            *w /= sum;
        }
    }
    let mut valence = 0.0f32;
    let mut arousal = 0.0f32;
    let mut energy = 0.0f32;
    for (i, line) in DEMO.iter().enumerate() {
        let w = frame.weights[i];
        energy += w * (0.25 + 0.75 * line.motion.min(1.2));
        arousal += w * (line.motion * 0.6 - 0.2);
        valence += w * if line.display_srgb[0] > line.display_srgb[2] { 0.4 } else { -0.1 };
    }
    frame.valence = valence.clamp(-1.0, 1.0);
    frame.arousal = arousal.clamp(-1.0, 1.0);
    frame.dominance = 0.1;
    frame.energy = energy.clamp(0.0, 1.0);
    frame.vad = 0.85;
    frame.timestamp = time as f64;
    frame
}

fn paint(frame: &mut NebulaFrame, names: &[&str], amount: f32) {
    if names.is_empty() || amount <= 0.0 {
        return;
    }
    let share = amount / names.len() as f32;
    for name in names {
        if let Some(index) = DEMO.iter().position(|line| line.emotion == *name) {
            frame.weights[index] += share;
            frame.elements[index] = DEMO[index].element_id;
        }
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
