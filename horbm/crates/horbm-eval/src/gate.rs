// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use galion_conductor::{ClauseBrain, Conductor, Ear as FloorEar, PickedClause, TickEvent};
use galion_emotion::persona_mood::{Persona as EmotionPersona, PersonaProfile};
use galion_emotion::text_affect;
use galion_emotion_core::FRAME_MS as EMOTION_FRAME_MS;
use galion_emotion_core::{IntentMsg, AROUSAL, VALENCE};
use horbm::{Brain, KnowledgeCards, Persona, TurnOut, EMOTION_FRAME_MS as BRAIN_FRAME_MS};
use horbm_voice::{
    EarEvent, FakeEar, FakeMouth, HorbmTurn, TickEvent as VoiceEvent, VoiceLoop, FRAME_MS,
    FRAME_SAMPLES, SAMPLE_RATE,
};
use serde::Serialize;

use crate::research_dir_is_unset;
use crate::score::{
    percentile, score_emotion, score_grounding, score_latency, score_persona, score_unknown,
    CardFact, Check, EmotionSample, LatencySample, Spoken, VoiceAnchors, VoiceTurn, VoiceWindow,
};
use crate::thresholds::Thresholds;

/// Which ear and mouth the gate runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// [`horbm_voice::FakeEar`] and [`horbm_voice::FakeMouth`]. Virtual clock.
    Fake,
    /// `galion-asr-vosk` and the Piper/Flite worker. Wall clock. Linux.
    Real,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            Self::Real => "real",
        }
    }
}

pub struct Config {
    pub research_dir: Option<String>,
    pub thresholds: Thresholds,
    pub mode: Mode,
    /// Real-ear prosody sensing. On unless the off switch is set.
    /// Fake mode does not read audio, so the flag does not change that run.
    pub prosody: bool,
    /// Explicit persona file. `None` loads the development persona.
    pub persona_file: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub mode: &'static str,
    /// Id loaded for this run. Empty when the file could not be resolved.
    pub persona: String,
    pub pass: bool,
    /// The real ear was not run. The binary exits 2. This is not a score miss.
    pub skipped: bool,
    pub research_dir: &'static str,
    pub truth_cards: Vec<String>,
    pub stand_ins: Vec<String>,
    pub scores: Scores,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Scores {
    pub grounded: ScoreOut,
    pub unknown: ScoreOut,
    pub persona: ScoreOut,
    pub emotion: ScoreOut,
    pub latency: ScoreOut,
    /// Real mode only. The fake gate leaves this empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asr: Option<ScoreOut>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScoreOut {
    pub pass: bool,
    pub failures: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
}

impl Report {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".into())
    }
}

pub fn run() -> Report {
    run_with(Config {
        research_dir: crate::research_dir_from_env(),
        thresholds: Thresholds::embedded(),
        mode: Mode::Fake,
        prosody: true,
        persona_file: None,
    })
}

/// Load `path` when it is set, otherwise the development persona.
pub(crate) fn resolve_persona(path: Option<&Path>) -> Result<Persona, String> {
    match path {
        Some(path) => Persona::load(path),
        None => Persona::load_development(),
    }
    .map_err(|err| err.to_string())
}

pub fn run_with(config: Config) -> Report {
    if !research_dir_is_unset(config.research_dir.as_deref()) {
        return fail(
            config.mode,
            "set",
            "",
            "GALION_HORBM_RESEARCH_DIR is set; the truth root is the only answer source",
        );
    }
    let persona = match resolve_persona(config.persona_file.as_deref()) {
        Ok(persona) => persona,
        Err(err) => return fail(config.mode, "unset", "", &err),
    };
    let result = match config.mode {
        Mode::Fake => execute(&config.thresholds, &persona).map_err(crate::real::GateErr::fail),
        Mode::Real => crate::real::execute(&config, &persona),
    };
    match result {
        Ok(report) => report,
        Err(err) if err.skip => skip(config.mode, persona.id(), &err.message),
        Err(err) => fail(config.mode, "unset", persona.id(), &err.message),
    }
}

fn fail(mode: Mode, research_dir: &'static str, persona_id: &str, error: &str) -> Report {
    let broken = ScoreOut {
        pass: false,
        failures: vec![error.to_string()],
        detail: None,
    };
    Report {
        schema: "horbm-eval-report/1",
        mode: mode.as_str(),
        persona: persona_id.to_string(),
        pass: false,
        skipped: false,
        research_dir,
        truth_cards: Vec::new(),
        stand_ins: stand_ins(mode, persona_id),
        scores: Scores {
            grounded: broken.clone(),
            unknown: broken.clone(),
            persona: broken.clone(),
            emotion: broken.clone(),
            latency: broken,
            asr: None,
        },
        error: Some(error.to_string()),
    }
}

fn skip(mode: Mode, persona_id: &str, error: &str) -> Report {
    let mut report = fail(mode, "unset", persona_id, error);
    report.skipped = true;
    report
}

fn stand_ins(mode: Mode, persona_id: &str) -> Vec<String> {
    match mode {
        Mode::Fake => vec![
            "ear:FakeEar (Vosk is not linked)".into(),
            "mouth:FakeMouth (Piper is not linked)".into(),
            "research:OfflineClient (GALION_HORBM_RESEARCH_DIR unset)".into(),
        ],
        Mode::Real => {
            let mut rows = vec!["research:OfflineClient (GALION_HORBM_RESEARCH_DIR unset)".into()];
            if !persona_id.is_empty() {
                rows.push(format!("persona:{persona_id}"));
            }
            rows
        }
    }
}

/// The seven scripted turns. The second field is the truth-root card, if any.
pub(crate) const SCRIPT: &[(&str, Option<&str>)] = &[
    ("where is the kettle", Some("kettle")),
    ("right here", Some("here")),
    ("who won the match", None),
    ("why is the sky blue?", None),
    ("I am happy", None),
    ("I am sad", None),
    ("I am so tired", None),
];

pub(crate) struct Truth {
    _dir: TempDir,
    pub cards: KnowledgeCards,
    pub ids: Vec<String>,
    pub pack: Vec<CardFact>,
    pub forbidden: Vec<String>,
}

pub(crate) fn load_truth() -> Result<Truth, String> {
    let dir = TempDir(truth_dir()?);
    write_truth(&dir.0)?;
    let loaded = KnowledgeCards::load(&dir.0).map_err(|err| err.to_string())?;
    let pack = vec![
        CardFact {
            id: "kettle".into(),
            gist: "The kettle is on the hob.".into(),
        },
        CardFact {
            id: "here".into(),
            gist: "Right here.".into(),
        },
    ];
    let mut ids: Vec<_> = loaded.cards.ids().map(str::to_string).collect();
    ids.sort();
    let mut expect_ids: Vec<_> = pack.iter().map(|card| card.id.clone()).collect();
    expect_ids.sort();
    if ids != expect_ids {
        return Err(format!(
            "truth root indexed {ids:?}, script has {expect_ids:?}"
        ));
    }
    let forbidden = standin_snippets()?;
    Ok(Truth {
        _dir: dir,
        cards: loaded.cards,
        ids,
        pack,
        forbidden,
    })
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn execute(thresholds: &Thresholds, persona: &Persona) -> Result<Report, String> {
    if thresholds.emotion.frame_ms != EMOTION_FRAME_MS || thresholds.emotion.frame_ms != FRAME_MS {
        return Err("threshold frame does not match the emotion core and the voice clock".into());
    }
    if thresholds.emotion.sample_rate_hz != SAMPLE_RATE {
        return Err("threshold sample rate does not match the mouth".into());
    }
    if BRAIN_FRAME_MS != EMOTION_FRAME_MS {
        return Err("brain emotion frame does not match the core".into());
    }

    let truth = load_truth()?;
    let pcm = tone_frame(
        thresholds.emotion.sample_rate_hz,
        thresholds.emotion.frame_ms,
    )?;

    let mut spoken = Vec::new();
    let mut voices = Vec::new();
    let mut eager_ms = Vec::new();
    let mut research_merged = false;
    for (query, expect) in SCRIPT {
        let turn = one_turn(&truth.cards, query, *expect, &pcm, persona)?;
        research_merged |= turn.research_merged;
        eager_ms.push(turn.voice_gap_ms);
        eager_ms.push(turn.conductor_gap_ms);
        voices.push(turn.voice);
        spoken.push(turn.spoken);
    }

    let emotion = measure_emotion(thresholds, persona)?;
    let barge_voice = barge_voice(&truth.cards, &pcm)?;
    let barge_conductor = barge_conductor(&truth.cards, persona)?;
    let backchannel_cancel =
        backchannel_voice(&truth.cards, &pcm)? || backchannel_conductor(&truth.cards, persona)?;
    let (clock_advanced_ms, wall_sleep_ms) = clock_ignores_wall(&truth.cards, persona);

    let grounded = score_grounding(
        &spoken,
        &truth.pack,
        &truth.forbidden,
        &thresholds.grounding,
    );
    let mut unknown = score_unknown(
        &spoken,
        &truth.pack,
        &truth.forbidden,
        &thresholds.grounding,
    );
    if research_merged {
        unknown.pass = false;
        unknown
            .failures
            .push("conductor merged a research clause".into());
    }
    let persona_score = voice_check(&voices, persona.id());
    let emotion_check = score_emotion(&emotion.sample, &thresholds.emotion);
    let latency = score_latency(
        &LatencySample {
            eager_to_voice_ms: eager_ms.clone(),
            barge_in_ms: barge_voice.max(barge_conductor),
            backchannel_cancel,
            clock_advanced_ms,
            wall_sleep_ms,
        },
        &thresholds.latency,
    );

    let pass = grounded.pass
        && unknown.pass
        && persona_allows_gate(&persona_score)
        && emotion_check.pass
        && latency.pass;
    Ok(Report {
        schema: "horbm-eval-report/1",
        mode: Mode::Fake.as_str(),
        persona: persona.id().to_string(),
        pass,
        skipped: false,
        research_dir: "unset",
        truth_cards: truth.ids,
        stand_ins: stand_ins(Mode::Fake, persona.id()),
        scores: Scores {
            grounded: out(
                &grounded,
                serde_json::json!({
                    "hits": spoken.iter().filter(|turn| turn.expect_id.is_some()).count(),
                    "lines": reply_lines(&spoken, true),
                }),
            ),
            unknown: out(
                &unknown,
                serde_json::json!({
                    "misses": spoken.iter().filter(|turn| turn.expect_id.is_none()).count(),
                    "lines": reply_lines(&spoken, false),
                }),
            ),
            persona: out(
                &persona_score,
                voice_detail(persona.id(), voices.len(), &persona_score),
            ),
            emotion: out(
                &emotion_check,
                serde_json::json!({
                    "frame_ms": emotion.sample.frame_ms,
                    "step_p95_us": emotion.sample.step_p95_us,
                    "budget_us": thresholds.emotion.frame_budget_us,
                    "fatigue_arousal_drop": emotion.sample.fatigue_drop,
                    "valence_rise": emotion.sample.valence_rise,
                }),
            ),
            latency: out(
                &latency,
                serde_json::json!({
                    "clock": "virtual",
                    "eager_to_voice_p50_ms": percentile(&eager_ms, 50),
                    "eager_to_voice_p95_ms": percentile(&eager_ms, 95),
                    "samples": eager_ms.len(),
                    "barge_in_ms": barge_voice.max(barge_conductor),
                    "barge_in_voice_ms": barge_voice,
                    "barge_in_conductor_ms": barge_conductor,
                    "backchannel_cancel": backchannel_cancel,
                    "wall_sleep_ms": wall_sleep_ms,
                    "clock_advanced_ms": clock_advanced_ms,
                }),
            ),
            asr: None,
        },
        error: None,
    })
}

pub(crate) fn out(check: &Check, detail: serde_json::Value) -> ScoreOut {
    ScoreOut {
        pass: check.pass,
        failures: check.failures.clone(),
        detail: Some(detail),
    }
}

struct TurnDone {
    spoken: Spoken,
    voice: VoiceTurn,
    voice_gap_ms: u32,
    conductor_gap_ms: u32,
    research_merged: bool,
}

fn one_turn(
    cards: &KnowledgeCards,
    query: &str,
    expect: Option<&str>,
    pcm: &[i16],
    persona: &Persona,
) -> Result<TurnDone, String> {
    let mut brain = Brain::with_persona(cards.clone(), None, *persona);
    let before = brain.emotion().baseline().identity_bits();
    let out = brain.turn(query);
    if brain.emotion().baseline().identity_bits() != before {
        return Err(format!("{query}: the turn rewrote the voice baseline"));
    }
    let voice_gap_ms = voice_gap(cards, query, &out.reply, pcm)?;
    let (conductor_gap_ms, research_merged, clause) = conductor_gap(cards, query, persona)?;
    if clause != out.reply {
        return Err(format!(
            "{query}: conductor spoke {clause:?}, brain spoke {:?}",
            out.reply
        ));
    }
    Ok(TurnDone {
        voice: VoiceTurn {
            persona: persona.id().to_string(),
            rate: out.mood_meta.rate,
            energy: out.mood_meta.energy,
            pitch_st: out.mood_meta.pitch_st,
            neutral: text_affect(query).valence.is_none(),
        },
        spoken: Spoken {
            query: query.to_string(),
            reply: out.reply,
            sources: out.sources,
            sideband_posted: out
                .sideband
                .as_ref()
                .map(|mark| mark.posted)
                .unwrap_or(false),
            expect_id: expect.map(str::to_string),
        },
        voice_gap_ms,
        conductor_gap_ms,
        research_merged,
    })
}

fn voice_gap(cards: &KnowledgeCards, query: &str, reply: &str, pcm: &[i16]) -> Result<u32, String> {
    let mut voice = VoiceLoop::new(
        FakeEar::script([EarEvent::end(query), EarEvent::quiet()]),
        FakeMouth::new(),
        HorbmTurn::with_cards(cards.clone()),
    );
    let end = voice.tick(pcm);
    if end.event != VoiceEvent::EndOfSpeech {
        return Err(format!("{query}: ear did not end the turn"));
    }
    if end.reply.as_deref() != Some(reply) {
        return Err(format!("{query}: mouth path reply drifted"));
    }
    if end.audio.is_some() {
        return Err(format!("{query}: audio arrived on the end-of-speech frame"));
    }
    let audio = voice.tick(pcm);
    if audio.event != VoiceEvent::FirstAudio {
        return Err(format!("{query}: first mouth frame was {:?}", audio.event));
    }
    let frame = audio.audio.as_ref().ok_or("mouth returned no frame")?;
    if frame.sample_rate != SAMPLE_RATE || frame.samples.len() != FRAME_SAMPLES {
        return Err(format!(
            "mouth frame is {} Hz × {} samples",
            frame.sample_rate,
            frame.samples.len()
        ));
    }
    if frame.samples.iter().all(|sample| *sample == 0) {
        return Err("mouth frame is silence".into());
    }
    let gap = voice
        .first_audio_after_eot_ms()
        .ok_or("missing eager-to-voice gap")?;
    u32::try_from(gap).map_err(|_| "gap does not fit u32".into())
}

pub(crate) fn conductor_gap(
    cards: &KnowledgeCards,
    query: &str,
    persona: &Persona,
) -> Result<(u32, bool, String), String> {
    let mut conductor = Conductor::new(LinkedBrain::new(cards.clone(), persona));
    let _ = conductor.tick(FloorEar::Speech(query), None);
    let _ = conductor.tick(FloorEar::Speech(query), None);
    let eager = conductor.tick(FloorEar::Quiet, None);
    if eager.event != TickEvent::EagerEndOfTurn {
        return Err(format!(
            "{query}: conductor did not reach eager end of turn"
        ));
    }
    let eou = eager.stamps.t_eou_decided.ok_or("eager stamp missing")?;
    let commit = conductor.tick(FloorEar::Quiet, None);
    if commit.event != TickEvent::Committed {
        return Err(format!("{query}: conductor did not commit"));
    }
    let flushed = commit
        .stamps
        .t_first_clause_flushed
        .ok_or("clause stamp missing")?;
    if flushed < eou {
        return Err("clause stamp precedes eager".into());
    }
    let gap = u32::try_from((flushed - eou) / 1_000).map_err(|_| "gap does not fit u32")?;
    let clause = commit
        .clause
        .as_ref()
        .map(|picked| picked.clause.clone())
        .ok_or("commit had no clause")?;
    let merged = commit.research_clause.is_some();
    Ok((gap, merged, clause))
}

struct LinkedBrain {
    inner: RefCell<Brain>,
}

impl LinkedBrain {
    fn new(cards: KnowledgeCards, persona: &Persona) -> Self {
        Self {
            inner: RefCell::new(Brain::with_persona(cards, None, *persona)),
        }
    }
}

impl ClauseBrain for LinkedBrain {
    fn answer(&self, query: &str) -> PickedClause {
        let out: TurnOut = self.inner.borrow_mut().turn(query);
        let hit = out.sources.iter().any(|source| source.starts_with("card:"));
        PickedClause {
            id: out.sources.first().cloned().unwrap_or_default(),
            clause: out.reply,
            lead: out.first_clause,
            rest: out.rest,
            hit,
        }
    }
}

fn barge_voice(cards: &KnowledgeCards, pcm: &[i16]) -> Result<u32, String> {
    let mut voice = VoiceLoop::new(
        FakeEar::script([
            EarEvent::end("where is the kettle"),
            EarEvent::quiet(),
            EarEvent::partial("stop"),
            EarEvent::partial("stop"),
            EarEvent::partial("stop"),
        ]),
        FakeMouth::new(),
        HorbmTurn::with_cards(cards.clone()),
    );
    let _ = voice.tick(pcm);
    let _ = voice.tick(pcm);
    let _ = voice.tick(pcm);
    let _ = voice.tick(pcm);
    let yielded = voice.tick(pcm);
    if yielded.event != VoiceEvent::BargeIn || !yielded.cancel {
        return Err("voice barge-in did not cancel".into());
    }
    let ms = voice.barge_in_ms().ok_or("voice barge-in had no time")?;
    u32::try_from(ms).map_err(|_| "barge time does not fit u32".into())
}

fn barge_conductor(cards: &KnowledgeCards, persona: &Persona) -> Result<u32, String> {
    let mut conductor = Conductor::new(LinkedBrain::new(cards.clone(), persona));
    let _ = conductor.tick(FloorEar::Speech("where is the kettle"), None);
    let _ = conductor.tick(FloorEar::Speech("where is the kettle"), None);
    let _ = conductor.tick(FloorEar::Quiet, None);
    let commit = conductor.tick(FloorEar::Quiet, None);
    if commit.event != TickEvent::Committed {
        return Err("conductor was not speaking before barge-in".into());
    }
    let first = conductor.tick(FloorEar::Speech("stop talking now"), None);
    let onset = first.t_ms;
    if first.cancel {
        return Err("barge-in cancelled on the arming frame".into());
    }
    let mut last = first;
    let mut silence_at = None;
    for _ in 0..6 {
        if last.me_silent {
            silence_at = Some(last.t_ms);
            break;
        }
        last = conductor.tick(FloorEar::Speech("stop talking now"), None);
    }
    let silence_at = silence_at.ok_or("conductor did not go silent")?;
    if !last.cancel || last.event != TickEvent::BargeIn {
        return Err("conductor barge-in did not cancel".into());
    }
    u32::try_from(silence_at - onset).map_err(|_| "barge time does not fit u32".into())
}

fn backchannel_voice(cards: &KnowledgeCards, pcm: &[i16]) -> Result<bool, String> {
    let mut script = vec![EarEvent::end("where is the kettle"), EarEvent::quiet()];
    for _ in 0..6 {
        script.push(EarEvent::partial("yeah"));
    }
    let mut voice = VoiceLoop::new(
        FakeEar::script(script),
        FakeMouth::new(),
        HorbmTurn::with_cards(cards.clone()),
    );
    let epoch = voice.epoch();
    let _ = voice.tick(pcm);
    let _ = voice.tick(pcm);
    let mut cancel = false;
    for _ in 0..6 {
        let frame = voice.tick(pcm);
        cancel |= frame.cancel;
        if frame.event != VoiceEvent::Backchannel || frame.epoch != epoch {
            return Err("voice backchannel changed the floor".into());
        }
    }
    if voice.mouth().cancels() != 0 {
        cancel = true;
    }
    Ok(cancel)
}

fn backchannel_conductor(cards: &KnowledgeCards, persona: &Persona) -> Result<bool, String> {
    let mut conductor = Conductor::new(LinkedBrain::new(cards.clone(), persona));
    let _ = conductor.tick(FloorEar::Speech("where is the kettle"), None);
    let _ = conductor.tick(FloorEar::Speech("where is the kettle"), None);
    let _ = conductor.tick(FloorEar::Quiet, None);
    let commit = conductor.tick(FloorEar::Quiet, None);
    let epoch = commit.epoch;
    let mut cancel = false;
    for _ in 0..6 {
        let frame = conductor.tick(FloorEar::Speech("yeah"), None);
        cancel |= frame.cancel;
        if frame.event != TickEvent::Backchannel || frame.epoch != epoch {
            return Err("conductor backchannel changed the floor".into());
        }
    }
    Ok(cancel)
}

fn clock_ignores_wall(cards: &KnowledgeCards, persona: &Persona) -> (u32, u32) {
    let mut conductor = Conductor::new(LinkedBrain::new(cards.clone(), persona));
    let wall_sleep_ms = 30u32;
    std::thread::sleep(Duration::from_millis(u64::from(wall_sleep_ms)));
    let frame = conductor.tick(FloorEar::Quiet, None);
    let advanced = conductor.t_ms().saturating_sub(frame.t_ms);
    (u32::try_from(advanced).unwrap_or(u32::MAX), wall_sleep_ms)
}

pub(crate) struct EmotionRun {
    pub sample: EmotionSample,
}

pub(crate) fn measure_emotion(
    thresholds: &Thresholds,
    persona: &Persona,
) -> Result<EmotionRun, String> {
    let frames = thresholds.emotion.frames;
    let (mut rest, rest_us) = step_script(frames, persona, |_| {})?;
    let (tired, tired_us) = step_script(frames, persona, |msg| {
        msg.fatigue = 0.90;
    })?;
    let (lifted, lift_us) = step_script(frames, persona, |msg| {
        msg.strength = 1.0;
        msg.set_pull(VALENCE, 1.0)
            .map_err(|_| "valence pull was rejected".to_string())
            .ok();
    })?;
    let drop = rest.emotion().moment().pos[AROUSAL] - tired.emotion().moment().pos[AROUSAL];
    let rise = lifted.emotion().moment().pos[VALENCE] - lifted.emotion().baseline().valence();
    let mut times = rest_us;
    times.extend(tired_us);
    times.extend(lift_us);
    let next = frames.saturating_mul(EMOTION_FRAME_MS);
    let before = rest.emotion().moment();
    let refused = rest
        .step_moment(&IntentMsg::steady(next.saturating_add(40)))
        .is_err();
    let held = rest.emotion().moment() == before
        && rest.emotion().baseline().identity_bits() == tired.emotion().baseline().identity_bits()
        && tired.emotion().baseline().identity_bits()
            == lifted.emotion().baseline().identity_bits();
    let cue = lifted.conductor_cue();
    let cue_follows = cue.moment[VALENCE].to_bits()
        == lifted.emotion().moment().pos[VALENCE].to_bits()
        && cue.moment[AROUSAL].to_bits() == lifted.emotion().moment().pos[AROUSAL].to_bits()
        && cue.t_ms == lifted.emotion().moment().t_ms;
    if !lifted
        .emotion()
        .baseline()
        .persona_id()
        .eq_ignore_ascii_case(persona.id())
    {
        return Err("emotion baseline persona is not the configured persona".into());
    }
    Ok(EmotionRun {
        sample: EmotionSample {
            fatigue_drop: drop,
            valence_rise: rise,
            step_p95_us: percentile(&times, 95),
            frame_ms: EMOTION_FRAME_MS,
            off_clock_refused: refused,
            baseline_held: held,
            cue_follows_moment: cue_follows,
        },
    })
}

fn step_script(
    frames: u32,
    persona: &Persona,
    edit: impl Fn(&mut IntentMsg),
) -> Result<(Brain, Vec<u32>), String> {
    let mut brain = Brain::with_persona(KnowledgeCards::empty(), None, *persona);
    let bits = brain.emotion().baseline().identity_bits();
    let mut times = Vec::with_capacity(frames as usize);
    for index in 0..frames {
        let mut msg = IntentMsg::steady(index.saturating_mul(EMOTION_FRAME_MS));
        edit(&mut msg);
        let started = Instant::now();
        let moment = brain
            .step_moment(&msg)
            .map_err(|err| format!("emotion step {index} failed: {err:?}"))?;
        times.push(elapsed_us(started));
        if moment.t_ms != index.saturating_mul(EMOTION_FRAME_MS) {
            return Err(format!("step {index} landed at {} ms", moment.t_ms));
        }
    }
    if brain.emotion().baseline().identity_bits() != bits {
        return Err("emotion step wrote the baseline".into());
    }
    Ok((brain, times))
}

/// Spoken when [`EmotionPersona::parse`] has no profile for this id.
pub(crate) const NO_VOICE_PROFILE: &str = "no voice profile";

/// Window from the configured persona's emotion profile.
pub(crate) fn persona_window(profile: &PersonaProfile) -> VoiceWindow {
    let voice = &profile.voice;
    VoiceWindow {
        pitch_max: voice.pitch_bound_st,
        rate_lo: voice.length_bounds.0,
        rate_hi: voice.length_bounds.1,
        energy_lo: voice.energy_bounds.0,
        energy_hi: voice.energy_bounds.1,
    }
}

pub(crate) fn anchors(profile: &PersonaProfile) -> VoiceAnchors {
    let voice = &profile.voice;
    let other = EmotionPersona::ALL
        .into_iter()
        .find(|item| *item != profile.persona)
        .map(EmotionPersona::profile)
        .map(|item| {
            (
                item.voice.base_length_scale,
                item.voice.base_energy,
                item.voice.base_pitch_st,
            )
        })
        .unwrap_or((
            voice.base_length_scale,
            voice.base_energy,
            voice.base_pitch_st,
        ));
    VoiceAnchors {
        persona: (
            voice.base_length_scale,
            voice.base_energy,
            voice.base_pitch_st,
        ),
        other,
    }
}

/// A missing profile is not a pass and does not use another persona's window.
pub(crate) fn voice_check(turns: &[VoiceTurn], persona_id: &str) -> Check {
    match EmotionPersona::parse(persona_id).map(EmotionPersona::profile) {
        Some(profile) => {
            score_persona(turns, persona_window(profile), anchors(profile), persona_id)
        }
        None => {
            let mut check = Check {
                pass: false,
                failures: vec![NO_VOICE_PROFILE.to_string()],
            };
            if turns.is_empty() {
                check.failures.push("no voice turn".to_string());
            }
            for turn in turns {
                if turn.persona != persona_id {
                    check
                        .failures
                        .push(format!("persona {} is not {persona_id}", turn.persona));
                }
            }
            check
        }
    }
}

/// The gate still passes when the only voice result is [`NO_VOICE_PROFILE`].
/// That score itself stays a failure, and no other profile is consulted.
pub(crate) fn persona_allows_gate(check: &Check) -> bool {
    check.pass || (check.failures.len() == 1 && check.failures[0] == NO_VOICE_PROFILE)
}

pub(crate) fn voice_detail(persona_id: &str, turns: usize, check: &Check) -> serde_json::Value {
    let mut detail = serde_json::json!({
        "id": persona_id,
        "turns": turns,
    });
    if check.failures.iter().any(|line| line == NO_VOICE_PROFILE) {
        detail["voice"] = serde_json::Value::String(NO_VOICE_PROFILE.to_string());
    }
    detail
}

pub(crate) fn reply_lines(spoken: &[Spoken], grounded: bool) -> Vec<&str> {
    spoken
        .iter()
        .filter(|turn| turn.expect_id.is_some() == grounded)
        .map(|turn| turn.reply.as_str())
        .collect()
}

fn tone_frame(rate: u32, frame_ms: u32) -> Result<Vec<i16>, String> {
    let n = (u64::from(rate) * u64::from(frame_ms) / 1_000) as usize;
    if n != FRAME_SAMPLES || rate != SAMPLE_RATE {
        return Err("tone frame is not one 16 kHz conductor frame".into());
    }
    let pcm = galion_emotion_in::synth::sine(180.0, 0.2, n, rate);
    let samples: Vec<i16> = pcm
        .into_iter()
        .map(|sample| (sample.clamp(-1.0, 1.0) * 32_767.0).round() as i16)
        .collect();
    if samples.iter().all(|sample| *sample == 0) {
        return Err("synthetic tone is silent".into());
    }
    Ok(samples)
}

fn standin_snippets() -> Result<Vec<String>, String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../crates/galion-emotion-core/standins");
    let world = galion_emotion_core::load_standins(&dir)
        .map_err(|err| format!("emotion stand-ins: {err}"))?;
    Ok(world.memory.into_iter().map(|card| card.snippet).collect())
}

fn truth_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("horbm-eval-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path).map_err(|err| err.to_string())?;
    Ok(path)
}

fn write_truth(dir: &Path) -> Result<(), String> {
    for (id, gist) in [
        ("kettle", "The kettle is on the hob."),
        ("here", "Right here."),
    ] {
        let path = dir.join(format!("{id}.json"));
        std::fs::write(&path, card_json(id, gist)).map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn card_json(id: &str, gist: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{gist}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{gist}",
  "body": "{gist}",
  "claim_type": "fact",
  "sources": [{{
    "title": "Galion knowledge fixture",
    "url": "https://github.com/galion-studio/galion-knowledge",
    "author": "Galion Labs",
    "date": "2026-11-18",
    "accessed": "2026-11-22"
  }}],
  "evidence_strength": "strong",
  "said_by": "Galion Labs",
  "as_of_date": "2026-11-18",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
    )
}

fn elapsed_us(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX)
}
