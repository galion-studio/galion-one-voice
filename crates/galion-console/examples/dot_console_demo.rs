// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! W21-04 · one dot driven by the real 80 ms heartbeat + live console bars.
//!
//! ```text
//! cargo run -p galion-console --features dot-viz --example dot_console_demo -- [opts]
//!   --seconds N        run N seconds of audio clock (default 8)
//!   --headless         no sleeping, no screen animation; one summary line per ~1 s
//!                      (auto when stdout is not a TTY)
//!   --realtime         pace at 80 ms/frame even when headless (live PNG/web viewing)
//!   --png-dir DIR      PNG output dir (default target/dot-demo); `latest.png` always
//!   --png-latest-only  only rewrite latest.png (no numbered sequence)
//!   --no-png           no PNG surface
//!   --size WxH         PNG size (default 320x180)
//!   --feed stub|real   timing/affect feeds (default real when compiled: galion-tensor
//!                      kernel timing + galion-emotion SENSE; stub = deterministic)
//!   --sense-off        SENSE consent off: affect shows "no reading", Rika stays OFF
//!   --web ADDR         also serve / + /dot JSON (needs feature dot-viz-web).
//!                      GET /v2 redirects to /.
//!   --synthetic        scripted room → you → me loop. Off by default: silence
//!                      until a mic level or a typed line arrives.
//!   --no-dot           panel without the terminal dot
//! NO_COLOR=1 for monochrome.
//! ```
//! Without `--synthetic` the you and me bands stay silent, so the circle
//! idles until a phone turn. `--synthetic` is the old room → in-breath →
//! you speak → pause → me speaks loop (barge-in every other turn, 6 s).

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use galion_console::dotviz::{demo_band, DemoPump, DemoSeg, DotConsole, PanelOpts};
use galion_console::live::{AffectFeed, StubAffect, StubTiming, TimingFeed};
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_MS, FRAME_SAMPLES, RIKA_BITS, SAMPLE_RATE};
use galion_render::{PngSeqConfig, PngSeqSurface};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

fn fill_you(buf: &mut [f32], i: u64, s: DemoSeg, barge: bool, rng: &mut Lcg) {
    let voice = s == DemoSeg::You || barge;
    for (k, x) in buf.iter_mut().enumerate() {
        let n = (i as usize * FRAME_SAMPLES + k) as f32 / SAMPLE_RATE as f32;
        *x = if voice {
            let env = 0.55 + 0.45 * (n * 3.7 * std::f32::consts::TAU).sin().abs();
            0.1 * env * (n * 200.0 * std::f32::consts::TAU).sin() + 0.001 * rng.next()
        } else if s == DemoSeg::Breath {
            0.003 * 3f32.sqrt() * rng.next()
        } else {
            0.001 * 3f32.sqrt() * rng.next()
        };
    }
}

fn fill_me(buf: &mut [f32], i: u64, on: bool, gain: f32) {
    for (k, x) in buf.iter_mut().enumerate() {
        let n = (i as usize * FRAME_SAMPLES + k) as f32 / SAMPLE_RATE as f32;
        *x = if on {
            let env = 0.4 + 0.6 * (n * 4.2 * std::f32::consts::TAU).sin().abs();
            gain * 0.12 * env * (n * 140.0 * std::f32::consts::TAU).sin()
        } else {
            0.0
        };
    }
}

fn val(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|p| args.get(p + 1)).cloned()
}

fn run<T: TimingFeed, A: AffectFeed>(args: &[String], mut pump: DemoPump<T, A>, sense: bool) -> std::io::Result<()> {
    let has = |k: &str| args.iter().any(|a| a == k);
    let synthetic = has("--synthetic");
    let seconds: f32 = val(args, "--seconds").and_then(|s| s.parse().ok()).unwrap_or(8.0);
    let frames = ((seconds * 1000.0) / FRAME_MS as f32).ceil().max(1.0) as u64;
    let headless = has("--headless") || std::env::var_os("GALION_HEADLESS").is_some_and(|v| v == "1") || !std::io::stdout().is_terminal();
    let realtime = !headless || has("--realtime");
    let color = std::env::var_os("NO_COLOR").is_none() && !headless;
    let png_dir = val(args, "--png-dir").unwrap_or_else(|| "target/dot-demo".into());
    let (pw, ph) = val(args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))))
        .unwrap_or((320u32, 180u32));

    let mut dc = DotConsole::new();
    let mut notes = Vec::new();
    if !has("--no-png") {
        let cfg = PngSeqConfig { dir: png_dir.clone().into(), width: pw, height: ph, sequence: !has("--png-latest-only"), ..Default::default() };
        match dc.attach(Box::new(PngSeqSurface::new(cfg))) {
            Ok(()) => notes.push(format!("png → {png_dir}/latest.png{}", if has("--png-latest-only") { "" } else { " + frame_NNNNNN.png" })),
            Err(e) => notes.push(format!("png surface failed: {e}")),
        }
    }
    #[cfg(feature = "phone-turn")]
    let mut phone: Option<galion_console::phone_host::PhoneHost> = None;
    if let Some(addr) = val(args, "--web") {
        #[cfg(feature = "dot-viz-web")]
        match addr.parse() {
            Ok(a) => {
                let web = galion_render::WebSurface::serve(a)
                    .with_page(galion_console::dotviz::SCREEN_HTML)
                    .with_v2_page(galion_console::dotviz::V2_SCREEN_HTML);
                #[cfg(feature = "phone-turn")]
                {
                    phone = Some(galion_console::phone_host::PhoneHost::from_surface(&web));
                }
                match dc.attach(Box::new(web)) {
                    Ok(()) => notes.push(format!("web → screen http://{addr}/  · /v2 redirects · /dot /mic /turn /reply")),
                    Err(e) => notes.push(format!("web surface failed: {e}")),
                }
            }
            Err(e) => notes.push(format!("bad --web addr {addr}: {e}")),
        }
        #[cfg(not(feature = "dot-viz-web"))]
        notes.push(format!("--web {addr} ignored: build with --features dot-viz-web"));
    }
    let mut gate = DefaultConsentGate::default();
    gate.grant(0, "en", Purpose::PersonalAssistant);
    let mut hb = Heartbeat::new(HeartbeatConfig::rika_v0(), gate);
    let bits = if sense { RIKA_BITS } else { 0 };
    let (tn, an) = pump.names();
    if synthetic {
        notes.push("synthetic room/you/me script on".into());
    } else {
        notes.push("idle until mic or text (--synthetic runs the room script)".into());
    }
    let mut out_w = std::io::stdout().lock();
    writeln!(out_w, "galion dot+console demo · W21-04 · {frames} frames ({seconds}s) · feeds {tn}+{an} · SENSE {} · {}", if sense { "on" } else { "off" }, match (headless, realtime) { (true, true) => "headless+realtime", (true, false) => "headless (fast)", _ => "tty realtime" })?;
    for n in &notes {
        writeln!(out_w, "  {n}")?;
    }
    let opts = PanelOpts { color, dot_rows: if has("--no-dot") { 0 } else { 12 }, ..Default::default() };
    if !headless {
        write!(out_w, "\x1b[?25l\x1b[2J")?;
    }

    let mut you = vec![0.0f32; FRAME_SAMPLES];
    let mut me = vec![0.0f32; FRAME_SAMPLES];
    let mut rng = Lcg(13);
    let mut me_voiced_prev = false;
    let mut yielded = false;
    let t0 = Instant::now();
    let mut late_next = false;
    for i in 0..frames {
        let (s, barge) = demo_band(synthetic, i);
        if s != DemoSeg::Me {
            yielded = false;
        }
        let phone_you;
        #[cfg(feature = "phone-turn")]
        {
            phone_you = phone.as_mut().is_some_and(|p| p.drive_you(&mut you));
        }
        #[cfg(not(feature = "phone-turn"))]
        {
            phone_you = false;
        }
        if !phone_you {
            if s == DemoSeg::Idle {
                you.fill(0.0);
            } else {
                fill_you(&mut you, i, s, barge, &mut rng);
            }
        }
        let inp = FrameIn {
            you_pcm: &you,
            affect: None,
            tension: None,
            consent_bits: bits,
            purpose: Purpose::PersonalAssistant,
            session_open: true,
            me_speaking: me_voiced_prev,
            late: late_next,
        };
        let ts = Instant::now();
        let out = hb.step(&inp);
        let step_us = ts.elapsed().as_secs_f32() * 1e6;
        if out.yield_now() {
            yielded = true;
        }
        let phone_me;
        #[cfg(feature = "phone-turn")]
        {
            phone_me = phone.as_mut().is_some_and(|p| p.drive_me(&mut me));
        }
        #[cfg(not(feature = "phone-turn"))]
        {
            phone_me = false;
        }
        let want_me = s == DemoSeg::Me && !yielded;
        let me_voiced = if phone_me {
            !out.paused
        } else {
            want_me && (me_voiced_prev || out.me_onset_allowed) && !out.paused
        };
        if me_voiced && !me_voiced_prev {
            hb.note_me_onset(&out, true);
        }
        if !phone_me {
            if s == DemoSeg::Idle {
                me.fill(0.0);
            } else {
                fill_me(&mut me, i, me_voiced, out.me_gain());
            }
        }
        let snap = pump.pump(i, &you, SAMPLE_RATE);
        dc.frame(&out, &you, &me, me_voiced, &snap, step_us);
        me_voiced_prev = me_voiced;

        if headless {
            if i % 12 == 0 || i + 1 == frames {
                writeln!(out_w, "{}", dc.summary_line(&out, &snap))?;
            }
        } else {
            write!(out_w, "\x1b[H{}", dc.panel(&out, &snap, step_us, opts))?;
            out_w.flush()?;
        }
        if realtime {
            let deadline = t0 + Duration::from_millis((i + 1) * FRAME_MS as u64);
            let now = Instant::now();
            late_next = now > deadline + Duration::from_millis(FRAME_MS as u64);
            if now < deadline {
                std::thread::sleep(deadline - now);
            }
        }
    }
    if !headless {
        write!(out_w, "\x1b[?25h")?;
    }
    let st = dc.stats;
    let summary = format!(
        "DONE frames={} paused={} lean_in_frames={} yields={} rika_lean_in_ms={:?} mean_total_ms={:.2} max_total_ms={:.2} over_budget={} max_step_us={:.0} surfaces={:?} dropped={:?} wall={:.2}s",
        st.frames,
        st.paused,
        st.lean_in_frames,
        st.yields,
        hb.lean_in_ms(),
        st.sum_total_ms / st.frames.max(1) as f64,
        st.max_total_ms,
        st.over_budget,
        st.max_step_us,
        dc.fanout.kinds(),
        dc.fanout.dropped(),
        t0.elapsed().as_secs_f32()
    );
    writeln!(out_w, "{summary}")?;
    if !has("--no-png") {
        let _ = std::fs::write(std::path::Path::new(&png_dir).join("summary.txt"), format!("{summary}\n"));
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let sense = !args.iter().any(|a| a == "--sense-off");
    let feed = val(&args, "--feed").unwrap_or_else(|| "real".into());
    match feed.as_str() {
        "stub" => run(&args, DemoPump::new(StubTiming::default(), StubAffect::new(sense)), sense),
        "real" => {
            #[cfg(all(feature = "tensor-feed", feature = "emotion-feed"))]
            {
                use galion_console::feeds::{EmotionAffectFeed, TensorFeedConfig, TensorTimingFeed};
                let timing = TensorTimingFeed::new(TensorFeedConfig::default()).expect("tensor feed config");
                let affect = if sense { EmotionAffectFeed::sense_on() } else { EmotionAffectFeed::unconsented() };
                run(&args, DemoPump::new(timing, affect), sense)
            }
            #[cfg(not(all(feature = "tensor-feed", feature = "emotion-feed")))]
            {
                eprintln!("real feeds not compiled (need tensor-feed + emotion-feed); using stub");
                run(&args, DemoPump::new(StubTiming::default(), StubAffect::new(sense)), sense)
            }
        }
        other => {
            eprintln!("--feed {other}: expected stub|real");
            std::process::exit(2)
        }
    }
}
