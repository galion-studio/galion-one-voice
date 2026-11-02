// Galion Voice — Perpetual Open MIT License
//! galion-console binary. Terminal is the default front-end (works over plain SSH).
//!
//! Usage:
//!   galion-console                 interactive TUI (q / Esc / Ctrl-C to quit)
//!   galion-console --headless [N]  render N ticks (default 20) to an in-memory
//!                                  80x24 buffer and print the final frame as text
//!                                  (CI smoke test; needs no TTY)
//!   galion-console --headless N --answers SEQ
//!                                  scripted YES/NO presses, one char per tick:
//!                                  `y` = yes, `n` = no, anything else = no press.
//!                                  Each committed HeartbeatEvent is printed to stderr.
//!
//! Live wiring (A11) — any mode:
//!   --live                 heartbeat/tensor/emotion stubs: one latency per stage + affect
//!   --sense-off            SENSE consent off: affect + sense stage show "no reading"
//!   --feed stub|real       live data source (default stub). `real` = galion-tensor
//!                          kernel timing + galion-emotion AffectStore SENSE (needs the
//!                          default `real-feeds` cargo feature; stub-only builds refuse).
//!   --mic-wav PATH         CAPTURE from a WAV fixture (integer PCM, mono/stereo,
//!                          8–192 kHz; galion-presence loader; looped, resampled to
//!                          24 kHz). Implies --feed real. Needs feature `wav-mic`.
//!   --mic synth|live       synth (default) = deterministic tone. `live` is STUBBED:
//!                          no audio device on the box → exits 2 with the reason.
//!   --affect SPEC          SENSE estimator for --feed real: `loudness` (default,
//!                          valence unknown) | `synthetic:<neutral|sweep|joy|angry|sad|
//!                          tired|calm|valence:V>` | `reference:PATH` (offline label
//!                          track replay, e.g. SenseVoice-Small dev-oracle output).
//!                          Implies --feed real. No model is loaded.
//!   --answers-log PATH     persist committed YES/NO answers as JSONL (and restore on start).
//!                          Interactive --live defaults to $GALION_CONSOLE_ANSWERS or
//!                          $XDG_STATE_HOME/galion-console/answers.jsonl; headless only
//!                          persists when the flag is given. --no-persist disables.
//!   NO_COLOR=1             monochrome output (https://no-color.org)
//!
//! Keys (interactive): `y`/`Left` = YES, `n`/`Right` = NO. A press only arms the
//! answer; it commits as one heartbeat event on the next 80 ms tick (last press wins).

use std::time::{Duration, Instant};

use galion_console::{live, ui, Answer, AnswerLog, ConsoleState, LiveSource, MetricsSource, StubFeed, SyntheticSource};
use ratatui::{
    backend::TestBackend,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    Terminal,
};

const TICK: Duration = Duration::from_millis(80); // one 80 ms audio frame per tick

struct Opts {
    live: bool,
    real: bool,
    sense: bool,
    mic_wav: Option<std::path::PathBuf>,
    affect: Option<String>,
    log_path: Option<std::path::PathBuf>,
}

fn flag_val(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let headless_mode = args.first().map(String::as_str) == Some("--headless");
    let live = has("--live");
    let log_path = if has("--no-persist") {
        None
    } else if let Some(p) = flag_val(&args, "--answers-log") {
        Some(p.into())
    } else if live && !headless_mode {
        live::default_answers_path()
    } else {
        None
    };
    let real = match flag_val(&args, "--feed").as_deref() {
        None | Some("stub") => false,
        Some("real") => true,
        Some(other) => {
            eprintln!("galion-console: unknown --feed {other:?} (use stub|real)");
            std::process::exit(2);
        }
    };
    match flag_val(&args, "--mic").as_deref() {
        None | Some("synth") => {}
        Some("live") => {
            eprintln!("galion-console: {}", galion_console::LiveMic::open().unwrap_err());
            std::process::exit(2);
        }
        Some(other) => {
            eprintln!("galion-console: unknown --mic {other:?} (use synth|live, or --mic-wav PATH)");
            std::process::exit(2);
        }
    }
    let mic_wav = flag_val(&args, "--mic-wav").map(std::path::PathBuf::from);
    let affect = flag_val(&args, "--affect");
    let real = real || mic_wav.is_some() || affect.is_some();
    let opts = Opts { live: live || real, real, sense: !has("--sense-off"), log_path, mic_wav, affect };
    if headless_mode {
        let n: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
        let answers = flag_val(&args, "--answers").unwrap_or_default();
        return headless(n, &answers, &opts);
    }
    interactive(&opts)
}

/// Shared setup: state (colour, restored answers), source, optional answer log.
fn setup(opts: &Opts) -> std::io::Result<(ConsoleState, Box<dyn MetricsSource>, Option<AnswerLog>)> {
    let mut state = ConsoleState::default();
    state.color = live::color_enabled();
    let src: Box<dyn MetricsSource> = if opts.real {
        real_source(opts)?
    } else if opts.live {
        Box::new(LiveSource { feed: StubFeed::new(opts.sense), inner: SyntheticSource::default() })
    } else {
        Box::new(SyntheticSource::default())
    };
    let log = match &opts.log_path {
        Some(p) => {
            for ev in AnswerLog::load(p)? {
                state.yes_no.restore(ev);
            }
            Some(AnswerLog::open(p)?)
        }
        None => None,
    };
    Ok((state, src, log))
}

#[cfg(all(feature = "tensor-feed", feature = "emotion-feed"))]
fn real_source(opts: &Opts) -> std::io::Result<Box<dyn MetricsSource>> {
    use galion_console::feeds::{real_feed, real_feed_with, AffectSource};
    use galion_console::{MicSource, PcmMic, SynthMic};
    let bad = |e: String| std::io::Error::new(std::io::ErrorKind::InvalidInput, e);
    if opts.mic_wav.is_none() && opts.affect.is_none() {
        return Ok(Box::new(LiveSource { feed: real_feed(opts.sense), inner: SyntheticSource::default() }));
    }
    let affect = match &opts.affect {
        Some(s) => AffectSource::parse(s).map_err(bad)?,
        None => AffectSource::Loudness,
    };
    let (mic, label): (Box<dyn MicSource>, &'static str) = match &opts.mic_wav {
        Some(p) => (Box::new(PcmMic::from_wav(p, 24_000).map_err(|e| bad(e.to_string()))?), "real+wav"),
        None => (Box::new(SynthMic::default()), "real"),
    };
    Ok(Box::new(LiveSource { feed: real_feed_with(opts.sense, mic, affect, label), inner: SyntheticSource::default() }))
}

#[cfg(not(all(feature = "tensor-feed", feature = "emotion-feed")))]
fn real_source(opts: &Opts) -> std::io::Result<Box<dyn MetricsSource>> {
    let _ = (&opts.mic_wav, &opts.affect); // only used by the real-feed build
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!(
            "--feed real needs features tensor-feed + emotion-feed (compiled: {})",
            galion_console::feeds::compiled_feeds()
        ),
    ))
}

fn headless(ticks: u64, answers: &str, opts: &Opts) -> std::io::Result<()> {
    let (mut state, mut src, mut log) = setup(opts)?;
    let width = if opts.live { 120 } else { 80 };
    let mut term = Terminal::new(TestBackend::new(width, 24))?;
    let mut script = answers.chars();
    for _ in 0..ticks.max(1) {
        if let Some(a) = script.next().and_then(Answer::from_char) {
            state.yes_no.press(a);
        }
        src.poll(&mut state);
        term.draw(|f| ui::draw(f, &mut state))?;
        if let Some(ev) = state.advance() {
            if let Some(l) = log.as_mut() {
                l.append(&ev)?;
            }
            eprintln!("heartbeat t={} {} at={}ms presses={}", ev.tick, ev.kind.label(), ev.at_ms, ev.presses);
        }
    }
    // Final frame reflects the last tick's commit.
    term.draw(|f| ui::draw(f, &mut state))?;
    let buf = term.backend().buffer();
    for y in 0..buf.area.height {
        let line: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        println!("{}", line.trim_end());
    }
    Ok(())
}

fn interactive(opts: &Opts) -> std::io::Result<()> {
    let setup = setup(opts)?; // before raw mode so I/O errors print normally
    let mut terminal = ratatui::init(); // raw mode + alt screen + panic hook restore
    let res = run(&mut terminal, setup);
    ratatui::restore();
    res
}

type Setup = (ConsoleState, Box<dyn MetricsSource>, Option<AnswerLog>);

fn run(terminal: &mut ratatui::DefaultTerminal, (mut state, mut src, mut log): Setup) -> std::io::Result<()> {
    let mut last = Instant::now();
    loop {
        terminal.draw(|f| ui::draw(f, &mut state))?;
        let timeout = TICK.saturating_sub(last.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    if matches!(k.code, KeyCode::Char('q') | KeyCode::Esc)
                        || (k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        return Ok(());
                    }
                    // Arm only; the answer commits on the next heartbeat tick below.
                    let answer = match k.code {
                        KeyCode::Left => Some(Answer::Yes),
                        KeyCode::Right => Some(Answer::No),
                        KeyCode::Char(c) => Answer::from_char(c),
                        _ => None,
                    };
                    if let Some(a) = answer {
                        state.yes_no.press(a);
                    }
                }
            }
        }
        if last.elapsed() >= TICK {
            src.poll(&mut state);
            if let (Some(ev), Some(l)) = (state.advance(), log.as_mut()) {
                l.append(&ev)?;
            }
            last = Instant::now();
        }
    }
}
