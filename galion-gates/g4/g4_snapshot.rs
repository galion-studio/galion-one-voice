// Galion Voice — Perpetual Open MIT License
// G4 render snapshot harness (agent A16). Built by galion-gates/g4-render.sh as a throwaway
// cargo package with a path dependency on galion-render. Deterministic, headless, no deps.
// Snapshot (sha256'd by the shell) = three sections over a scripted 64-frame heartbeat:
//   [null-dot]  NullSurface::last_dot -> DotParams::to_json per frame (A12 one-dot contract)
//   [term-dot]  TerminalDotSurface 80x24, 24-bit colour, obsidian bg (user surface)
//   [term-dev]  TerminalSurface 80 cols, no colour (dev console)
// Checks: NullSurface accepts every frame; two full renders byte-identical; present() p99 < 1 ms
// on NullSurface and TerminalDotSurface (release build).
// Prints one JSON line on stdout; writes the snapshot to argv[1].
use galion_render::dot::TerminalDotOpts;
use galion_render::{Emotion, NullSurface, RenderSurface, TerminalDotSurface, TerminalSurface, VizFrame};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FRAMES: u64 = 64;

#[derive(Clone)]
struct Shared(Arc<Mutex<Vec<u8>>>);
impl Write for Shared {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> { self.0.lock().unwrap().extend_from_slice(b); Ok(b.len()) }
    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

/// Scripted 64-frame idle -> listening -> speaking -> idle sequence on the 80 ms codec clock.
fn script() -> Vec<VizFrame> {
    let mut v = Vec::with_capacity(FRAMES as usize);
    for i in 0..FRAMES {
        let mut f = VizFrame::default();
        f.seq = i; f.t_ms = i * 80;
        let ph = (i as f32) * 0.35;
        match i {
            0..=11 => {}
            12..=31 => {
                f.listen.active = true;
                f.listen.level = 0.25 + 0.5 * (0.5 + 0.5 * ph.sin());
                f.listen.emotion = Emotion { label: "curious", intensity: 0.6, valence: 0.3 };
                f.listen.transcript_tail = "can you hear me".into();
            }
            32..=55 => {
                f.speak.active = true;
                f.speak.level = 0.2 + 0.6 * (0.5 + 0.5 * ph.cos());
                f.speak.emotion = Emotion { label: "warm", intensity: 0.7, valence: 0.6 };
                f.speak.transcript_tail = "yes, I'm here".into();
                f.listen.transcript_tail = "can you hear me".into();
            }
            _ => { f.speak.transcript_tail = "yes, I'm here".into(); }
        }
        f.latency_ms = 160.0 + (i % 7) as f32;
        f.rtf = 0.42;
        v.push(f);
    }
    v
}

fn timed<S: RenderSurface>(mut t: S, frames: &[VizFrame]) -> Vec<u128> {
    t.open().expect("open");
    let mut ns = Vec::with_capacity(frames.len());
    for f in frames { let s = Instant::now(); t.present(f).expect("present"); ns.push(s.elapsed().as_nanos()); }
    t.close().expect("close");
    ns
}

/// Full snapshot + (null p99 inputs, dot-terminal timings, accepted count).
fn render_all(frames: &[VizFrame]) -> (Vec<u8>, Vec<u128>, Vec<u128>, u64) {
    let mut snap: Vec<u8> = Vec::new();
    // [null-dot] — only trait methods + the public last_dot field.
    snap.extend_from_slice(b"[null-dot]\n");
    let mut n = NullSurface::default();
    n.open().unwrap();
    let mut accepted = 0u64;
    let mut nns = Vec::with_capacity(frames.len());
    for f in frames {
        let s = Instant::now();
        if n.present(f).is_ok() { accepted += 1; }
        nns.push(s.elapsed().as_nanos());
        match &n.last_dot { Some(d) => snap.extend_from_slice(d.to_json(f.t_ms).as_bytes()), None => snap.extend_from_slice(b"null") }
        snap.push(b'\n');
    }
    n.close().unwrap();
    // [term-dot]
    snap.extend_from_slice(b"[term-dot]\n");
    let buf = Shared(Arc::new(Mutex::new(Vec::new())));
    let dns = timed(TerminalDotSurface::with_writer(Box::new(buf.clone()), 80, 24, TerminalDotOpts { color: true, gradient_bg: false }), frames);
    snap.extend_from_slice(&buf.0.lock().unwrap());
    // [term-dev]
    snap.extend_from_slice(b"\n[term-dev]\n");
    let buf = Shared(Arc::new(Mutex::new(Vec::new())));
    let _ = timed(TerminalSurface::with_writer(Box::new(buf.clone()), 80, false), frames);
    snap.extend_from_slice(&buf.0.lock().unwrap());
    (snap, nns, dns, accepted)
}

fn pct(mut v: Vec<u128>, p: f64) -> u128 { v.sort_unstable(); v[((v.len() - 1) as f64 * p).round() as usize] }

fn main() {
    let snap_path = std::env::args().nth(1).expect("usage: g4_snapshot <snapshot-out>");
    let frames = script();
    let _warm = render_all(&frames);
    let (a, nns, dns, accepted) = render_all(&frames);
    let (b, _, _, _) = render_all(&frames);
    let deterministic = a == b;
    let null_ok = accepted == FRAMES;
    std::fs::write(&snap_path, &a).expect("write snapshot");
    let np99 = pct(nns, 0.99);
    let dmax = *dns.iter().max().unwrap();
    let (dp50, dp99) = (pct(dns.clone(), 0.5), pct(dns, 0.99));
    let under_1ms = np99 < 1_000_000 && dp99 < 1_000_000;
    println!(
        "{{\"frames\":{},\"null_accepted\":{},\"null_ok\":{},\"deterministic\":{},\"snapshot_bytes\":{},\"null_p99_ns\":{},\"dot_term_p50_ns\":{},\"dot_term_p99_ns\":{},\"dot_term_max_ns\":{},\"present_p99_under_1ms\":{}}}",
        FRAMES, accepted, null_ok, deterministic, a.len(), np99, dp50, dp99, dmax, under_1ms
    );
    std::process::exit(if null_ok && deterministic && under_1ms { 0 } else { 1 });
}
