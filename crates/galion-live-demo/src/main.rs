// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! `galion-live-demo [--in you.wav] [--reply words.wav] [--out DIR] [--consent none|sense,adapt]
//!                   [--purpose personal|workplace|education|...] [--rika-opener] [--realtime] [--quiet]
//!                   [--web 127.0.0.1:PORT] [--loops N] [--hold SECS]`
//! Exit 0 ok, 2 usage / IO error, 69 `--web` feed could not bind (port busy).
//!
//! FP-02 V1 wiring: `--reply` puts the spoken EN-cascade reply in the **me** band (so the dot
//! shows *speaking* while the words play); the you band stays the synthetic room/breath/voice
//! script (Rika lean-in + listening). `--loops` keeps ONE process (and ONE bound port) for all turns.
use galion_live_demo::mimi::{MimiMode, MimiVoice};
use galion_live_demo::{parse_consent, parse_purpose, resample_to_24k, run_demo, Config, MeVoice, ReplyVoice, RunCfg, ToneVoice};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: galion-live-demo [--in you.wav] [--reply words.wav] [--loops N(0=forever)] [--out DIR=out/live] [--consent none|sense|sense,adapt] \
[--purpose personal|accessibility|medical|workplace|education] [--rika-opener] [--realtime] [--no-play] [--mimi off|bypass|codec|nokernels] [--dot] [--web 127.0.0.1:PORT] [--hold SECS] [--quiet]\n\
(--mimi default: bypass in release builds, off in debug builds; --dot needs --features dot-viz; --web/--hold need --features dot-viz-web)";

fn main() -> ExitCode {
    let mut cfg = Config { input: None, out_dir: PathBuf::from("out/live"), run: RunCfg::default(), echo: true };
    let mut mimi = MimiMode::default_for_build();
    let mut dot = false;
    // V1-02: live single-screen feed (additive; only active with --features dot-viz-web).
    let mut web: Option<String> = None;
    let mut hold_secs: u64 = 0;
    let mut reply: Option<PathBuf> = None;
    let mut loops: u64 = 1;
    // FP-03: play the reply locally on each me onset (default on with --realtime + --reply).
    let mut play = true;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = |name: &str| args.next().ok_or_else(|| format!("{name} needs a value"));
        let r: Result<(), String> = (|| {
            match a.as_str() {
                "--in" => cfg.input = Some(PathBuf::from(val("--in")?)),
                "--reply" => reply = Some(PathBuf::from(val("--reply")?)),
                "--loops" => loops = val("--loops")?.parse().map_err(|_| "--loops needs a whole number (0 = forever)".to_string())?,
                "--out" => cfg.out_dir = PathBuf::from(val("--out")?),
                "--consent" => cfg.run.consent_bits = parse_consent(&val("--consent")?)?,
                "--purpose" => cfg.run.purpose = parse_purpose(&val("--purpose")?)?,
                "--rika-opener" => cfg.run.rika_opener = true,
                "--realtime" => cfg.run.realtime = true,
                "--no-play" => play = false,
                "--quiet" => cfg.echo = false,
                "--dot" => dot = true,
                "--web" => { web = Some(val("--web")?); dot = true }
                "--hold" => hold_secs = val("--hold")?.parse().map_err(|_| "--hold needs whole seconds".to_string())?,
                "--mimi" => mimi = MimiMode::parse(&val("--mimi")?)?,
                "-h" | "--help" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                o => return Err(format!("unknown argument '{o}'")),
            }
            Ok(())
        })();
        if let Err(e) = r {
            eprintln!("galion-live-demo: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    }
    // Me band: the spoken reply WAV (FP-02, words) or the wordless tone voice (W21-01 default).
    let reply_pcm = match reply.as_deref().map(galion_presence::wav::read_wav).transpose() {
        Ok(p) => p.map(|p| resample_to_24k(&p.samples, p.sample_rate)),
        Err(e) => {
            eprintln!("galion-live-demo: --reply: {e}");
            return ExitCode::from(2);
        }
    };
    let mut voice: Box<dyn MeVoice> = match (reply_pcm, mimi) {
        (Some(r), MimiMode::Off) => Box::new(ReplyVoice::new(r)),
        (None, MimiMode::Off) => Box::new(ToneVoice::default()),
        (Some(r), m) => match MimiVoice::new(ReplyVoice::new(r.clone()), m) {
            Ok(v) => Box::new(v),
            Err(e) => {
                eprintln!("galion-live-demo: mimi lane unavailable ({e}); falling back to --mimi off");
                Box::new(ReplyVoice::new(r))
            }
        },
        (None, m) => match MimiVoice::new(ToneVoice::default(), m) {
            Ok(v) => Box::new(v),
            Err(e) => {
                eprintln!("galion-live-demo: mimi lane unavailable ({e}); falling back to --mimi off");
                Box::new(ToneVoice::default())
            }
        },
    };
    #[cfg(feature = "dot-viz")]
    let mut dot_sink = if dot {
        let sense = cfg.run.consent_bits & galion_emotion::band::consent_bits::SENSE != 0;
        // Web feed (V1 screen) needs only /dot — no PNG frames; plain --dot writes PNGs.
        let png = if web.is_none() { Some(cfg.out_dir.as_path()) } else { None };
        match galion_live_demo::dot::DotSink::new(png, sense, true) {
            #[allow(unused_mut)]
            Ok(mut d) => {
                #[cfg(feature = "dot-viz-web")]
                if let Some(a) = web.as_deref() {
                    match a.parse::<std::net::SocketAddr>().map_err(|e| e.to_string()).and_then(|sa| d.attach_web(sa)) {
                        Ok(()) => println!("galion-live-demo: one-dot feed live at http://{a}/dot"),
                        Err(e) => {
                            // FP-02: never run a "live" demo with no feed behind the one screen.
                            eprintln!("galion-live-demo: web feed unavailable on {a} ({e}) — port busy? (pkill -f galion-live-demo)");
                            return ExitCode::from(69);
                        }
                    }
                }
                #[cfg(not(feature = "dot-viz-web"))]
                if web.is_some() {
                    eprintln!("galion-live-demo: --web ignored (build with --features dot-viz-web)");
                }
                Some(d)
            }
            Err(e) => {
                eprintln!("galion-live-demo: dot surface unavailable ({e}); continuing without it");
                None
            }
        }
    } else {
        None
    };
    #[cfg(feature = "dot-viz")]
    let mut extra: Vec<&mut dyn galion_live_demo::FrameSink> = dot_sink.iter_mut().map(|d| d as &mut dyn galion_live_demo::FrameSink).collect();
    #[cfg(not(feature = "dot-viz"))]
    let mut extra: Vec<&mut dyn galion_live_demo::FrameSink> = {
        if dot {
            eprintln!("galion-live-demo: --dot ignored (build with --features dot-viz)");
        }
        Vec::new()
    };
    // FP-03: audible words through the speakers, started/stopped with the dot's speaking state.
    let mut player = match (&reply, play && cfg.run.realtime) {
        (Some(w), true) => match galion_live_demo::play::detect_player() {
            Some(cmd) => Some(galion_live_demo::play::PlaySink::new(w, cmd)),
            None => {
                eprintln!("galion-live-demo: no audio player (paplay/pw-play/aplay/ffplay) — words are in the stereo WAV only");
                None
            }
        },
        _ => None,
    };
    if let Some(p) = player.as_ref() {
        println!("galion-live-demo: local playback via {} on each speaking onset", p.player());
    }
    if let Some(p) = player.as_mut() {
        extra.push(p);
    }
    let mut turn: u64 = 0;
    loop {
        turn += 1;
        match run_demo(&cfg, voice.as_mut(), &mut extra) {
            Ok(a) if loops == 0 || turn < loops => {
                println!(
                    "galion-live-demo: turn {turn} OK frames={} me_onsets={} yields={} lean_in_frames={}",
                    a.summary.frames, a.summary.me_onsets, a.summary.yields, a.summary.lean_in_frames
                );
                continue;
            }
            Ok(a) => {
                println!(
                    "galion-live-demo: OK frames={} paused={} late={} me_onsets={} yields={} lean_in_frames={} soft_hum_frames={}",
                    a.summary.frames, a.summary.paused_frames, a.summary.late_frames, a.summary.me_onsets, a.summary.yields, a.summary.lean_in_frames, a.summary.soft_hum_frames
                );
                for p in [&a.input_wav, &a.stereo_wav, &a.capture, &a.sums] {
                    println!("  wrote {}", p.display());
                }
                // Keep the last frame served briefly after the final turn (used by --check).
                if hold_secs > 0 && web.is_some() && cfg!(feature = "dot-viz-web") {
                    println!("galion-live-demo: holding web feed {hold_secs}s");
                    std::thread::sleep(std::time::Duration::from_secs(hold_secs));
                }
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("galion-live-demo: {e}");
                return ExitCode::from(2);
            }
        }
    }
}
