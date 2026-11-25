// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Live duplex loop. Default input and default output, one 80 ms tick.
//!
//! The host is cpal. On Linux that host is ALSA (`galion-sam-audio-io`
//! selects it). PulseAudio and PipeWire are reached only when the ALSA
//! default plugin is installed. This process does not bind a socket.
//!
//! `--self-test` skips the devices and runs the closed script on synthetic
//! tones. That script is not an acoustic path, so echo removal is bypassed
//! there. `--no-prosody` turns ear sensing off. `--no-aec` is the echo
//! bypass on the device path. `--json` prints one JSON object per voice
//! onset and per barge-in. With `--features piper`, `--json` also prints
//! the warm-mouth measurements when a stock pack is already on disk.
//! `--piper` selects that mouth. The default stays the tone.
//!
//! Build: `cargo run --manifest-path horbm/Cargo.toml -p horbm-duplex --features live --bin horbm-duplex-live`
//! The recognizer is the stand-in unless this bin is also built with `--features vosk`.

use std::collections::VecDeque;
use std::process::ExitCode;
use std::time::Duration;

use galion_sam_audio_io::{open, Direction, SessionConfig};
use horbm::{CardHit, CardLookup};
#[cfg(not(feature = "vosk"))]
use horbm_duplex::SilentRecognizer;
use horbm_duplex::{
    CoreAffect, Duplex, FramePump, Hypothesis, HypothesisKind, LoopEvent, NativeMouth, Parts,
    PlaybackEcho, RecogError, Recognizer, SamanthaPersona, SpeakingBreath, FRAME_SAMPLES,
    SAMPLE_RATE,
};
use horbm_voice::{Mouth, MouthPull, SpokenLine, VoiceControls};

#[cfg(feature = "piper")]
use horbm_duplex::PiperMouth;

const HELP: &str = "\
horbm-duplex-live

  Opens the default input and the default output. Both bands step on each
  80 ms of captured audio. Nothing is bound to the network.

  --self-test     synthetic tones, no device, echo bypassed
  --no-prosody    ear sensing off (any non-zero sample is voiced)
  --no-aec        echo bypass (the microphone frame reaches the ear)
  --json          one JSON object per voice onset and per barge-in
  --help          this text

Linux: the audio host is ALSA through cpal. There is no Windows CI job.
The piper mouth is a separate feature. This build keeps the tone mouth
unless that feature is on and --piper is passed.
";

struct NoCards;

impl CardLookup for NoCards {
    fn lookup(&self, _query: &str) -> Option<CardHit> {
        None
    }

    fn len(&self) -> usize {
        0
    }
}

fn main() -> ExitCode {
    let mut json = false;
    let mut prosody = true;
    let mut aec = true;
    let mut self_test = false;
    let mut piper = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json = true,
            "--no-prosody" => prosody = false,
            "--no-aec" => aec = false,
            "--self-test" => self_test = true,
            "--piper" => piper = true,
            "--help" | "-h" => {
                println!("{HELP}");
                #[cfg(feature = "piper")]
                println!(
                    "  --piper         stock en_US-ljspeech mouth. Off unless this flag is set."
                );
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown argument {other}");
                eprintln!("{HELP}");
                return ExitCode::from(2);
            }
        }
    }
    if self_test {
        return run_self_test(json, prosody);
    }
    #[cfg(not(feature = "piper"))]
    if piper {
        eprintln!("duplex s34 --piper needs the piper feature");
        return ExitCode::from(2);
    }
    #[cfg(feature = "vosk")]
    {
        match horbm_duplex::AsrRecognizer::from_env() {
            Ok(recognizer) => run_devices(json, prosody, aec, recognizer, piper),
            Err(err) => {
                eprintln!("duplex s28 recognizer {err}");
                ExitCode::from(1)
            }
        }
    }
    #[cfg(not(feature = "vosk"))]
    {
        run_devices(json, prosody, aec, SilentRecognizer::new(), piper)
    }
}

fn run_devices<R: Recognizer>(
    json: bool,
    prosody: bool,
    aec: bool,
    recognizer: R,
    piper: bool,
) -> ExitCode {
    let name = recognizer.name().to_string();
    let mouth = match open_live(piper) {
        Ok(mouth) => mouth,
        Err(err) => {
            eprintln!("duplex s34 {err}");
            return ExitCode::from(1);
        }
    };
    let mouth_name = mouth.name().to_string();
    let mut duplex = match build(recognizer, prosody, aec, mouth) {
        Ok(duplex) => duplex,
        Err(err) => {
            eprintln!("duplex s28 {err}");
            return ExitCode::from(1);
        }
    };
    let session = match open(&SessionConfig {
        input_name: None,
        output_name: None,
        direction: Direction::Duplex,
    }) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("duplex s28 {err}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "duplex s28 host={} recognizer={name} emotion=galion-emotion-core prosody={} aec={} persona=samantha mouth={mouth_name} breath=stand-in network=none",
        session.info().host,
        on_off(duplex.prosody_enabled()),
        aec_label(duplex.aec_enabled()),
    );
    let playback = session.playback();
    let capture = session.capture();
    let mut pump = FramePump::new();
    loop {
        let Some(frame) = capture.pop() else {
            // The duplex clock does not move while the queue is empty.
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        for chunk in pump.push(&frame.pcm) {
            let out = match duplex.tick(&chunk) {
                Ok(out) => out,
                Err(err) => {
                    eprintln!("duplex s28 {err}");
                    return ExitCode::from(1);
                }
            };
            publish(json, out.event, duplex.report());
            if out.me_silent || out.cancel {
                playback.barge_in();
            } else if out.me_audible {
                let pcm = duplex.mouth().last();
                if !pcm.is_empty() {
                    if let Err(err) = playback.push_pcm16(pcm, SAMPLE_RATE) {
                        eprintln!("duplex s28 {err}");
                        return ExitCode::from(1);
                    }
                }
            }
        }
    }
}

fn run_self_test(json: bool, prosody: bool) -> ExitCode {
    let mut duplex = match build(
        Script::new(&["where is the kettle", "yeah", "stop now"]),
        prosody,
        false,
        NativeMouth::new(),
    ) {
        Ok(duplex) => duplex,
        Err(err) => {
            eprintln!("duplex s28 {err}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "duplex s28 host=self-test recognizer=stand-in emotion=galion-emotion-core prosody={} aec={} persona=samantha mouth=native breath=stand-in network=none",
        on_off(duplex.prosody_enabled()),
        aec_label(duplex.aec_enabled()),
    );
    let mut pump = FramePump::new();
    let mut feed =
        |duplex: &mut Duplex<Script, CoreAffect, SpeakingBreath, SamanthaPersona, NativeMouth>,
         pcm: &[i16],
         json: bool|
         -> bool {
            let mut failed = false;
            for chunk in pump.push(pcm) {
                match duplex.tick(&chunk) {
                    Ok(out) => publish(json, out.event, duplex.report()),
                    Err(err) => {
                        eprintln!("duplex s28 {err}");
                        failed = true;
                    }
                }
            }
            failed
        };
    for _ in 0..5 {
        if feed(&mut duplex, &tone(), json) {
            return ExitCode::from(1);
        }
    }
    for _ in 0..3 {
        if feed(&mut duplex, &silence(), json) {
            return ExitCode::from(1);
        }
    }
    for _ in 0..6 {
        if feed(&mut duplex, &tone(), json) {
            return ExitCode::from(1);
        }
    }
    if feed(&mut duplex, &silence(), json) {
        return ExitCode::from(1);
    }
    for _ in 0..3 {
        if feed(&mut duplex, &tone(), json) {
            return ExitCode::from(1);
        }
    }
    for _ in 0..2 {
        if feed(&mut duplex, &silence(), json) {
            return ExitCode::from(1);
        }
    }
    let report = duplex.report();
    let tone_ok = report.voice_ms == Some(160)
        && report.barge_ms == Some(160)
        && report.backchannel_cancels == 0;
    if !tone_ok {
        eprintln!("{}", report.turn_text("barge"));
        return ExitCode::from(1);
    }
    if !publish_piper(json) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn build<R: Recognizer, M: Mouth>(
    recognizer: R,
    prosody: bool,
    aec: bool,
    mouth: M,
) -> Result<Duplex<R, CoreAffect, SpeakingBreath, SamanthaPersona, M>, String> {
    let mut duplex = Duplex::new(
        Parts {
            recognizer,
            emotion: CoreAffect::samantha(),
            breath: SpeakingBreath,
            persona: SamanthaPersona,
            mouth,
        },
        NoCards,
    )
    .map_err(|err| err.to_string())?;
    if !prosody {
        duplex.set_prosody(false);
    }
    duplex.set_aec(aec);
    Ok(duplex)
}

fn publish_piper(json: bool) -> bool {
    #[cfg(not(feature = "piper"))]
    {
        let _ = json;
        true
    }
    #[cfg(feature = "piper")]
    {
        match horbm_duplex::discover_piper() {
            Some(paths) => match horbm_duplex::measure_piper(&paths) {
                Ok(stats) => {
                    if json {
                        println!("{}", stats.json());
                    } else {
                        println!("{}", stats.text());
                    }
                    if stats.barge_to_silence_ms > 160 {
                        eprintln!(
                            "duplex s34 barge_to_silence_ms={} over 160",
                            stats.barge_to_silence_ms
                        );
                        false
                    } else {
                        true
                    }
                }
                Err(err) => {
                    eprintln!("duplex s34 piper {err}");
                    false
                }
            },
            None => {
                if json {
                    println!("{}", horbm_duplex::piper_skipped_json());
                } else {
                    println!("duplex s34 piper skipped=stock-voice-absent cpu=1");
                }
                true
            }
        }
    }
}

fn publish(json: bool, event: LoopEvent, report: &horbm_duplex::LatencyReport) {
    let Some(name) = event_name(event) else {
        return;
    };
    if json {
        println!("{}", report.turn_json(name));
    } else {
        println!("{}", report.turn_text(name));
    }
}

fn event_name(event: LoopEvent) -> Option<&'static str> {
    match event {
        LoopEvent::FirstAudio => Some("voice"),
        LoopEvent::BargeIn => Some("barge"),
        LoopEvent::None
        | LoopEvent::EagerEndOfTurn
        | LoopEvent::Committed
        | LoopEvent::Backchannel
        | LoopEvent::ListenBackchannel
        | LoopEvent::TurnResumed => None,
    }
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

enum LiveMouth {
    Native(NativeMouth),
    #[cfg(feature = "piper")]
    Piper(Box<PiperMouth>),
}

impl LiveMouth {
    fn name(&self) -> &'static str {
        match self {
            Self::Native(_) => "native",
            #[cfg(feature = "piper")]
            Self::Piper(_) => "piper",
        }
    }

    fn last(&self) -> &[i16] {
        match self {
            Self::Native(mouth) => mouth.last(),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.last(),
        }
    }
}

impl Mouth for LiveMouth {
    fn id(&self) -> &str {
        self.name()
    }

    fn start(&mut self, line: &SpokenLine) {
        match self {
            Self::Native(mouth) => mouth.start(line),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.start(line),
        }
    }

    fn pull(&mut self) -> MouthPull {
        match self {
            Self::Native(mouth) => mouth.pull(),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.pull(),
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Native(mouth) => mouth.cancel(),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.cancel(),
        }
    }

    fn speaking(&self) -> bool {
        match self {
            Self::Native(mouth) => mouth.speaking(),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.speaking(),
        }
    }

    fn feel(&mut self, controls: VoiceControls) {
        match self {
            Self::Native(mouth) => mouth.feel(controls),
            #[cfg(feature = "piper")]
            Self::Piper(mouth) => mouth.feel(controls),
        }
    }
}

fn open_live(piper: bool) -> Result<LiveMouth, String> {
    if !piper {
        return Ok(LiveMouth::Native(NativeMouth::new()));
    }
    open_piper()
}

#[cfg(feature = "piper")]
fn open_piper() -> Result<LiveMouth, String> {
    let paths = horbm_duplex::discover_piper().ok_or("stock piper voice is not on disk")?;
    let mut mouth = PiperMouth::open(&paths)?;
    mouth.prepare()?;
    Ok(LiveMouth::Piper(Box::new(mouth)))
}

#[cfg(not(feature = "piper"))]
fn open_piper() -> Result<LiveMouth, String> {
    Err("piper mouth is not in this build".into())
}

fn aec_label(on: bool) -> &'static str {
    if !PlaybackEcho::compiled() {
        "off"
    } else if on {
        "on"
    } else {
        "bypass"
    }
}

fn tone() -> Vec<i16> {
    let mut pcm = vec![0i16; FRAME_SAMPLES];
    for (index, sample) in pcm.iter_mut().enumerate() {
        *sample = if index % 2 == 0 { 800 } else { -800 };
    }
    pcm
}

fn silence() -> Vec<i16> {
    vec![0i16; FRAME_SAMPLES]
}

/// Closed-test recognizer for `--self-test`. One scripted word per voiced
/// frame. Sample values are ignored. This is not the live engine.
struct Script {
    lines: VecDeque<Vec<String>>,
    words: Vec<String>,
    shown: usize,
    frames: u32,
    open: bool,
}

impl Script {
    fn new(lines: &[&str]) -> Self {
        Self {
            lines: lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect(),
            words: Vec::new(),
            shown: 0,
            frames: 0,
            open: false,
        }
    }
}

impl Recognizer for Script {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        if !self.open {
            self.words = self.lines.pop_front().unwrap_or_default();
            self.shown = 0;
            self.frames = 0;
            self.open = true;
        }
        self.frames = self.frames.saturating_add(1);
        if self.shown < self.words.len() {
            self.shown += 1;
            let text = self.words[..self.shown].join(" ");
            return Ok(vec![Hypothesis {
                kind: HypothesisKind::Partial,
                text,
                start_ms: 0,
                end_ms: self.audio_ms(),
            }]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.words.clear();
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        self.frames = 0;
        self.words.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(horbm_duplex::FRAME_MS)
    }

    fn name(&self) -> &str {
        "stand-in"
    }
}
