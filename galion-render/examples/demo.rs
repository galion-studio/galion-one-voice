//! cargo run -p galion-render --example demo [--features web]
//! Synthetic dual-band frames at 12.5 Hz for ~4 s; no audio, no model.
use galion_render::*;

fn main() {
    let mut fan = Fanout::new();
    #[cfg(feature = "terminal")]
    if let Err(e) = fan.attach(Box::new(TerminalSurface::stdout())) { eprintln!("terminal: {e}"); }
    #[cfg(feature = "window")]
    if let Err(e) = fan.attach(Box::new(WindowSurface::new(Default::default()))) { eprintln!("window: {e} (falling back)"); }
    #[cfg(feature = "web")]
    {
        let w = WebSurface::serve("127.0.0.1:7878".parse().unwrap());
        if let Err(e) = fan.attach(Box::new(w)) { eprintln!("web: {e}"); }
    }
    let words = ["hello", "how", "are", "you", "today", "friend"];
    for i in 0..50u64 {
        let t = i as f32 * 0.08;
        let mut f = VizFrame { seq: i, t_ms: i * 80, latency_ms: 210.0 + (t * 3.0).sin() * 20.0, rtf: 0.62, ..Default::default() };
        f.listen.level = ((t * 2.1).sin() * 0.5 + 0.5) * if i < 25 { 1.0 } else { 0.1 };
        f.listen.active = i < 25;
        f.listen.emotion = Emotion { label: "curious", intensity: 0.6, valence: 0.3 };
        f.listen.transcript_tail = words[..((i as usize / 4).min(5) + 1)].join(" ");
        f.speak.level = ((t * 3.3).cos() * 0.5 + 0.5) * if i >= 20 { 1.0 } else { 0.05 };
        f.speak.active = i >= 20;
        f.speak.emotion = Emotion { label: "warm", intensity: 0.7, valence: 0.8 };
        f.speak.transcript_tail = if i >= 20 { "I'm well, thank you".into() } else { String::new() };
        fan.present(&f);
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    fan.close_all();
    println!("surfaces compiled: {:?}", compiled_surfaces());
}
