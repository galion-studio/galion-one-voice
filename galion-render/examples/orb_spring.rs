//! cargo run -p galion-render --example orb_spring
//!
//! One second of the Luti orb spring at 240 Hz. Targets are [`DotParams`]
//! (the discrete golden) and change with the 80 ms heartbeat. The spring's
//! dt is the display frame. No window, no audio, no files.
use galion_render::{DisplayHz, DotParams, DotState, OrbEngine};

fn main() {
    let hz = DisplayHz::Hz240;
    let mut engine = OrbEngine::idle();
    let frames = hz.hz();
    for i in 0..frames {
        let t_ms = (i as u64) * 1000 / (hz.hz() as u64);
        let dot = if t_ms < 500 {
            DotParams::compute(DotState::Idle, 0.0, t_ms)
        } else {
            DotParams::compute(DotState::Speaking, 0.6, t_ms)
        };
        let sample = engine.step_toward_dot(&dot, hz.dt_s());
        if i == 0 || i == frames / 2 || i + 1 == frames {
            println!("frame {i} t_ms {t_ms} {}", sample.to_json());
        }
    }
}
