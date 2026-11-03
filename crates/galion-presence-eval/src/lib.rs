// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Virtual-user presence eval for the phone path.
//!
//! The user is in-process. It injects text or a WAV, PhoneLane answers with
//! CannedLlm and Piper or flite when one is installed, and the harness listens
//! to the captured reply WAV. The report is JSON. Nothing here dials a network
//! address that is not `127.0.0.1`, and the report has no engagement metric.
//!
//! Tiers, kept small enough for every PR:
//! - T0: canned task facts, interim label, metric denylist, supersede cancel
//! - T1: the same phone lane over loopback (text, fixture WAV, interim voice)
//! - T2: one loopback capture. TTFA is filled only when a reply WAV comes back.
//!   PSTN is `not_run`.
//!
//! Run: `scripts/eval/virtual-user.sh`. Doc: `docs/eval/PRESENCE-EVAL.md`.

use std::fs;
use std::path::{Path, PathBuf};

use galion_cascade::phone::wav_to_pcm16k;
use galion_cascade::{AsrMode, AsrSource, CannedLlm, Emotion, LlmMode, PhoneConfig, PhoneLane};
use galion_presence::json::Value;

mod barge;
mod inject;
mod listen;
mod loopback;
mod score;

use crate::inject::{fixture_wav, voice_wav};
use crate::listen::peak;
use crate::loopback::{post_turn, Loopback, TurnView};
use crate::score::{claims_recognition, TASK_FACTS};

pub const SCHEMA: &str = "galion-presence-eval/v1";
pub const TTFA_KIND: &str = "loopback-full-utterance";
pub const TTFA_NOTE: &str = "Wall time from the end of inject until the reply WAV is captured, plus leading silence inside that WAV. PhoneLane returns the whole utterance, so this is not a streaming first-frame time and not a PSTN mouth-to-ear time.";

#[derive(Debug, Clone)]
struct Case {
    id: String,
    tier: &'static str,
    inject: &'static str,
    input: String,
    transcript: String,
    reply: String,
    expected_reply: String,
    task_fact: &'static str,
    asr: String,
    claims_recognition: bool,
    fixture_decoded: bool,
    wav: Option<String>,
    wav_peak: Option<u32>,
    ttfa_ms: Option<u32>,
    status: &'static str,
    note: String,
}

/// Where a run writes, and the commit recorded in the receipt.
pub struct RunArgs<'a> {
    pub report: &'a Path,
    pub wav_dir: &'a Path,
    pub commit: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub verdict: &'static str,
    pub cases: usize,
    pub failed: usize,
    pub ttfa_status: String,
    pub ttfa_ms: Option<u32>,
    pub audio_captured: bool,
    pub barge_in_cancel: String,
    pub report: PathBuf,
}

/// Run T0–T2 and write the JSON report. `Ok` is returned for a red verdict too;
/// the summary says which. `Err` is usage, IO, or a harness fault.
pub fn run(args: RunArgs<'_>) -> Result<RunSummary, String> {
    if let Some(parent) = args.report.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(args.wav_dir).map_err(|e| e.to_string())?;

    let cfg = PhoneConfig {
        asr: AsrMode::Interim,
        llm: LlmMode::Canned,
    };
    let mut lane = PhoneLane::new(cfg);
    let (speech_backend, speech_label) = {
        let (id, label) = lane.speech_backend();
        (id.to_string(), label)
    };
    let can_speak = lane.can_speak();

    let mut cases = Vec::new();
    cases.extend(t0_facts(&mut lane));
    cases.push(t0_interim(&mut lane));
    cases.push(t0_stub(&mut lane));
    cases.push(t0_denylist());
    let barge = barge::probe_supersede();
    cases.push(Case {
        id: "T0-barge-in-supersede".into(),
        tier: "T0",
        inject: "supersede",
        input: "turn-1-then-100".into(),
        transcript: String::new(),
        reply: String::new(),
        expected_reply: String::new(),
        task_fact: "n/a",
        asr: "none".into(),
        claims_recognition: false,
        fixture_decoded: false,
        wav: None,
        wav_peak: None,
        ttfa_ms: None,
        status: barge.cancel,
        note: format!(
            "{} stop_ms={}",
            barge.note,
            barge
                .stop_ms
                .map(|n| n.to_string())
                .unwrap_or_else(|| "null".into())
        ),
    });

    let mut server = Loopback::bind(args.wav_dir.to_path_buf())?;
    let port = server.port();
    let hello = fact("hello");
    cases.push(loop_text(
        port,
        "T1-text-hello",
        "T1",
        "hello",
        hello,
        false,
        can_speak,
        args.wav_dir,
        "",
    )?);
    let unknown = fact("zebra quantum");
    cases.push(loop_text(
        port,
        "T1-text-unknown",
        "T1",
        "zebra quantum",
        unknown,
        false,
        can_speak,
        args.wav_dir,
        "",
    )?);
    let thanks = fact("thanks");
    cases.push(loop_wav(
        port,
        "T1-wav-thanks",
        "T1",
        "fixture:thanks",
        &fixture_wav("thanks"),
        "thanks",
        thanks,
        "stub-fixture",
        true,
        true,
        can_speak,
        args.wav_dir,
        "",
    )?);
    cases.push(loop_wav(
        port,
        "T1-interim-voice",
        "T1",
        "synthetic-voice-0.5s",
        &voice_wav(0.5),
        "hello",
        hello,
        "interim-placeholder",
        false,
        false,
        can_speak,
        args.wav_dir,
        "duration placeholder; the words were not heard",
    )?);
    let hear = fact("can you hear me");
    cases.push(loop_text(
        port,
        "T2-loopback-capture",
        "T2",
        "can you hear me",
        hear,
        true,
        can_speak,
        args.wav_dir,
        "",
    )?);
    server.close();

    let audio_captured = cases.iter().any(|c| c.wav.is_some());
    let t2_ttfa = cases
        .iter()
        .find(|c| c.id == "T2-loopback-capture")
        .and_then(|c| c.ttfa_ms);
    let ttfa_status = if t2_ttfa.is_some() {
        "measured"
    } else {
        "unmeasured"
    };
    let failed = cases.iter().filter(|c| c.status != "pass").count();
    let any_claim = cases.iter().any(|c| c.claims_recognition);
    let verdict = if failed == 0 && !any_claim && barge.cancel == "pass" && barge.supersede {
        "green"
    } else {
        "red"
    };

    let case_values = cases
        .iter()
        .map(case_value)
        .collect::<Result<Vec<_>, _>>()?;
    let tiers = tier_counts(&cases)?;
    let report = score::obj(vec![
        ("schema", Value::Str(SCHEMA.into())),
        ("program", Value::Str("Galion Voice".into())),
        ("owner", Value::Str("Maciej Grajczyk".into())),
        ("commit", Value::Str(args.commit.into())),
        ("transport", Value::Str("loopback".into())),
        ("bind", Value::Str("127.0.0.1".into())),
        ("network", Value::Bool(false)),
        ("pstn", Value::Str("not_run".into())),
        ("asr_mode", Value::Str("interim".into())),
        ("llm", Value::Str("canned".into())),
        ("reply_model", Value::Str("canned-llm".into())),
        ("speech_backend", Value::Str(speech_backend)),
        ("speech_label", Value::Str(speech_label)),
        ("endpointing", Value::Str("scripted-inject".into())),
        ("vad", Value::Str("not-on-this-path".into())),
        ("audio_captured", Value::Bool(audio_captured)),
        ("ttfa_ms", opt_num(t2_ttfa)),
        ("ttfa_status", Value::Str(ttfa_status.into())),
        ("ttfa_kind", Value::Str(TTFA_KIND.into())),
        ("ttfa_note", Value::Str(TTFA_NOTE.into())),
        ("supersede", Value::Bool(barge.supersede)),
        ("barge_in_cancel", Value::Str(barge.cancel.into())),
        ("barge_in_stop_ms", opt_num(barge.stop_ms)),
        ("barge_in_kind", Value::Str("worker-supersede".into())),
        ("claims_recognition", Value::Bool(any_claim)),
        ("tiers", tiers),
        ("cases", Value::Arr(case_values)),
        ("verdict", Value::Str(verdict.into())),
    ])?;
    score::walk_keys(&report)?;
    let body = report.to_json();
    fs::write(args.report, format!("{body}\n")).map_err(|e| e.to_string())?;

    Ok(RunSummary {
        verdict,
        cases: cases.len(),
        failed,
        ttfa_status: ttfa_status.into(),
        ttfa_ms: t2_ttfa,
        audio_captured,
        barge_in_cancel: barge.cancel.into(),
        report: args.report.to_path_buf(),
    })
}

fn fact(text: &str) -> &'static str {
    TASK_FACTS
        .iter()
        .find(|(k, _)| *k == text)
        .map(|(_, v)| *v)
        .expect("task fact")
}

fn slug(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn t0_facts(lane: &mut PhoneLane) -> Vec<Case> {
    let llm = CannedLlm::new();
    TASK_FACTS
        .iter()
        .map(|(text, expect)| {
            let id = format!("T0-fact-{}", slug(text));
            let body = llm.body(text, Emotion::Neutral);
            match lane.inject_text(text) {
                Ok((transcript, asr, reply)) => {
                    let fact_ok = reply == *expect && body == *expect;
                    let ok = transcript == *text
                        && fact_ok
                        && asr == AsrSource::TextInject
                        && !asr.is_recognised()
                        && !claims_recognition(asr);
                    Case {
                        id,
                        tier: "T0",
                        inject: "text",
                        input: (*text).into(),
                        transcript,
                        reply,
                        expected_reply: (*expect).into(),
                        task_fact: if fact_ok { "pass" } else { "fail" },
                        asr: asr.id().into(),
                        claims_recognition: claims_recognition(asr),
                        fixture_decoded: false,
                        wav: None,
                        wav_peak: None,
                        ttfa_ms: None,
                        status: if ok { "pass" } else { "fail" },
                        note: "text inject; not recognition".into(),
                    }
                }
                Err(e) => blank_fail(id, "T0", "text", text, expect, e.to_string()),
            }
        })
        .collect()
}

fn t0_interim(lane: &mut PhoneLane) -> Case {
    let expect = fact("hello");
    let wav = voice_wav(0.5);
    let id = "T0-interim-label".to_string();
    match wav_to_pcm16k(&wav).and_then(|pcm| lane.reply_text(&pcm)) {
        Ok((transcript, asr, reply)) => {
            let fact_ok = reply == expect;
            let ok = transcript == "hello"
                && fact_ok
                && asr == AsrSource::InterimPlaceholder
                && !asr.is_recognised()
                && !claims_recognition(asr)
                && asr.id() == "interim-placeholder";
            Case {
                id,
                tier: "T0",
                inject: "wav",
                input: "synthetic-voice-0.5s".into(),
                transcript,
                reply,
                expected_reply: expect.into(),
                task_fact: if fact_ok { "pass" } else { "fail" },
                asr: asr.id().into(),
                claims_recognition: claims_recognition(asr),
                fixture_decoded: false,
                wav: None,
                wav_peak: None,
                ttfa_ms: None,
                status: if ok { "pass" } else { "fail" },
                note: "duration placeholder; the words were not heard".into(),
            }
        }
        Err(e) => blank_fail(
            id,
            "T0",
            "wav",
            "synthetic-voice-0.5s",
            expect,
            e.to_string(),
        ),
    }
}

fn t0_stub(lane: &mut PhoneLane) -> Case {
    let expect = fact("how are you");
    let wav = fixture_wav("how are you");
    let id = "T0-stub-fixture".to_string();
    match wav_to_pcm16k(&wav).and_then(|pcm| lane.reply_text(&pcm)) {
        Ok((transcript, asr, reply)) => {
            let fact_ok = reply == expect;
            let ok = transcript == "how are you"
                && fact_ok
                && asr == AsrSource::StubFixture
                && asr.is_recognised()
                && !claims_recognition(asr);
            Case {
                id,
                tier: "T0",
                inject: "wav",
                input: "fixture:how are you".into(),
                transcript,
                reply,
                expected_reply: expect.into(),
                task_fact: if fact_ok { "pass" } else { "fail" },
                asr: asr.id().into(),
                claims_recognition: claims_recognition(asr),
                fixture_decoded: true,
                wav: None,
                wav_peak: None,
                ttfa_ms: None,
                status: if ok { "pass" } else { "fail" },
                note: "stub fixture decoded; the report does not call that recognition".into(),
            }
        }
        Err(e) => blank_fail(
            id,
            "T0",
            "wav",
            "fixture:how are you",
            expect,
            e.to_string(),
        ),
    }
}

fn t0_denylist() -> Case {
    let poisoned = Value::Obj(vec![("dwell".into(), Value::Num(1.0))]);
    let rejected = score::walk_keys(&poisoned).is_err();
    let spares =
        score::forbidden_word("ttfa_ms").is_none() && score::forbidden_word("onset_ms").is_none();
    Case {
        id: "T0-metric-denylist".into(),
        tier: "T0",
        inject: "none",
        input: String::new(),
        transcript: String::new(),
        reply: String::new(),
        expected_reply: String::new(),
        task_fact: "n/a",
        asr: "none".into(),
        claims_recognition: false,
        fixture_decoded: false,
        wav: None,
        wav_peak: None,
        ttfa_ms: None,
        status: if rejected && spares { "pass" } else { "fail" },
        note: "denylist rejects a poisoned key; report keys are checked before write".into(),
    }
}

fn blank_fail(
    id: String,
    tier: &'static str,
    inject: &'static str,
    input: &str,
    expect: &str,
    note: String,
) -> Case {
    Case {
        id,
        tier,
        inject,
        input: input.into(),
        transcript: String::new(),
        reply: String::new(),
        expected_reply: expect.into(),
        task_fact: "fail",
        asr: "none".into(),
        claims_recognition: false,
        fixture_decoded: false,
        wav: None,
        wav_peak: None,
        ttfa_ms: None,
        status: "fail",
        note,
    }
}

fn loop_text(
    port: u16,
    id: &str,
    tier: &'static str,
    text: &str,
    expect: &str,
    speak: bool,
    can_speak: bool,
    wav_dir: &Path,
    note: &str,
) -> Result<Case, String> {
    let view = post_turn(port, "text/plain", id, speak, text.as_bytes())?;
    Ok(score_view(
        id,
        tier,
        "text",
        text,
        text,
        expect,
        "text-inject",
        false,
        speak,
        can_speak,
        &view,
        wav_dir,
        note,
    ))
}

fn loop_wav(
    port: u16,
    id: &str,
    tier: &'static str,
    input: &str,
    wav: &[u8],
    expected_transcript: &str,
    expect: &str,
    expected_asr: &str,
    expect_fixture: bool,
    speak: bool,
    can_speak: bool,
    wav_dir: &Path,
    note: &str,
) -> Result<Case, String> {
    let view = post_turn(port, "audio/wav", id, speak, wav)?;
    Ok(score_view(
        id,
        tier,
        "wav",
        input,
        expected_transcript,
        expect,
        expected_asr,
        expect_fixture,
        speak,
        can_speak,
        &view,
        wav_dir,
        note,
    ))
}

fn score_view(
    id: &str,
    tier: &'static str,
    inject: &'static str,
    input: &str,
    expected_transcript: &str,
    expect: &str,
    expected_asr: &str,
    expect_fixture: bool,
    speak: bool,
    can_speak: bool,
    view: &TurnView,
    wav_dir: &Path,
    note: &str,
) -> Case {
    let mut notes = Vec::new();
    if !note.is_empty() {
        notes.push(note.to_string());
    }
    let mut ok = view.ok;
    if !view.ok {
        notes.push(view.error.clone());
    }
    let task_fact = if view.reply == expect { "pass" } else { "fail" };
    if task_fact != "pass" {
        ok = false;
        notes.push(format!("reply {:?} != {:?}", view.reply, expect));
    }
    if view.transcript != expected_transcript {
        ok = false;
        notes.push(format!(
            "transcript {:?} != {expected_transcript:?}",
            view.transcript
        ));
    }
    if view.asr != expected_asr {
        ok = false;
        notes.push(format!("asr {} != {expected_asr}", view.asr));
    }
    if view.claims_recognition {
        ok = false;
        notes.push("label claimed recognition".into());
    }
    if view.fixture_decoded != expect_fixture {
        ok = false;
        notes.push("fixture_decoded mismatch".into());
    }
    if view.asr == "interim-placeholder" && (view.claims_recognition || view.fixture_decoded) {
        ok = false;
        notes.push("interim must stay a placeholder".into());
    }
    let mut wav_peak = view.wav_peak;
    if speak && can_speak {
        match view.wav.as_deref() {
            Some(name) if safe_wav(name) => match fs::read(wav_dir.join(name)) {
                Ok(bytes) => match listen::parse_pcm16(&bytes) {
                    Ok(pcm) => {
                        let got = peak(&pcm.samples);
                        if pcm.rate != 24_000 {
                            ok = false;
                            notes.push(format!("reply rate {}", pcm.rate));
                        }
                        if got < 1_500 {
                            ok = false;
                            notes.push(format!("reply too quiet ({got})"));
                        }
                        if view.wav_peak != Some(got) {
                            ok = false;
                            notes.push("peak mismatch between header and file".into());
                        }
                        if view.ttfa_ms.is_none() {
                            ok = false;
                            notes.push("ttfa not measurable on a captured wav".into());
                        }
                        wav_peak = Some(got);
                        notes.push("reply wav captured".into());
                    }
                    Err(e) => {
                        ok = false;
                        notes.push(e);
                    }
                },
                Err(e) => {
                    ok = false;
                    notes.push(format!("could not read reply wav: {e}"));
                }
            },
            Some(_) => {
                ok = false;
                notes.push("reply wav name rejected".into());
            }
            None => {
                ok = false;
                notes.push("speech backend present but no reply wav".into());
            }
        }
    } else if speak && !can_speak {
        if view.wav.is_some() || view.ttfa_ms.is_some() {
            ok = false;
            notes.push("wav reported without a speech backend".into());
        } else {
            notes.push("speech backend absent; ttfa unmeasured".into());
        }
    } else if view.wav.is_some() || view.ttfa_ms.is_some() {
        ok = false;
        notes.push("audio returned when speak was off".into());
    }
    Case {
        id: id.into(),
        tier,
        inject,
        input: input.into(),
        transcript: view.transcript.clone(),
        reply: view.reply.clone(),
        expected_reply: expect.into(),
        task_fact,
        asr: view.asr.clone(),
        claims_recognition: view.claims_recognition,
        fixture_decoded: view.fixture_decoded,
        wav: view.wav.clone(),
        wav_peak,
        ttfa_ms: view.ttfa_ms,
        status: if ok { "pass" } else { "fail" },
        note: notes.join("; "),
    }
}

fn safe_wav(name: &str) -> bool {
    name.ends_with(".wav")
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

fn opt_num(n: Option<u32>) -> Value {
    match n {
        Some(n) => Value::Num(n as f64),
        None => Value::Null,
    }
}

fn opt_str(s: &Option<String>) -> Value {
    match s {
        Some(s) => Value::Str(s.clone()),
        None => Value::Null,
    }
}

fn case_value(c: &Case) -> Result<Value, String> {
    score::obj(vec![
        ("id", Value::Str(c.id.clone())),
        ("tier", Value::Str(c.tier.into())),
        ("inject", Value::Str(c.inject.into())),
        ("input", Value::Str(c.input.clone())),
        ("transcript", Value::Str(c.transcript.clone())),
        ("reply", Value::Str(c.reply.clone())),
        ("expected_reply", Value::Str(c.expected_reply.clone())),
        ("task_fact", Value::Str(c.task_fact.into())),
        ("asr", Value::Str(c.asr.clone())),
        ("claims_recognition", Value::Bool(c.claims_recognition)),
        ("fixture_decoded", Value::Bool(c.fixture_decoded)),
        ("wav", opt_str(&c.wav)),
        ("wav_peak", opt_num(c.wav_peak)),
        ("ttfa_ms", opt_num(c.ttfa_ms)),
        ("status", Value::Str(c.status.into())),
        ("note", Value::Str(c.note.clone())),
    ])
}

fn tier_counts(cases: &[Case]) -> Result<Value, String> {
    let mut fields = Vec::new();
    for tier in ["T0", "T1", "T2"] {
        let pass = cases
            .iter()
            .filter(|c| c.tier == tier && c.status == "pass")
            .count() as f64;
        let fail = cases
            .iter()
            .filter(|c| c.tier == tier && c.status != "pass")
            .count() as f64;
        fields.push((
            tier,
            score::obj(vec![("pass", Value::Num(pass)), ("fail", Value::Num(fail))])?,
        ));
    }
    let kv = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    Ok(Value::Obj(kv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_presence::json;

    #[test]
    fn virtual_user_writes_a_green_loopback_report() {
        // The loopback scores the Piper (A21) lane. On a host that has fetched the
        // Chatterbox clone weights, `auto` picks the out-of-process clone worker,
        // whose cold turn exceeds the 60 s loopback read timeout. Pin the lane the
        // report is about; an explicit caller value still wins.
        if std::env::var_os("GALION_SPEECH").is_none() {
            std::env::set_var("GALION_SPEECH", "a21");
        }
        let dir = std::env::temp_dir().join(format!("galion-presence-eval-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let report = dir.join("report.json");
        let wav_dir = dir.join("wav");
        let summary = run(RunArgs {
            report: &report,
            wav_dir: &wav_dir,
            commit: "test",
        })
        .unwrap();
        assert_eq!(summary.verdict, "green", "{summary:?}");
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.barge_in_cancel, "pass");
        let text = fs::read_to_string(&report).unwrap();
        let value = json::parse(&text).unwrap();
        assert_eq!(value.get("schema").and_then(|v| v.as_str()), Some(SCHEMA));
        assert_eq!(
            value.get("transport").and_then(|v| v.as_str()),
            Some("loopback")
        );
        assert_eq!(
            value.get("bind").and_then(|v| v.as_str()),
            Some("127.0.0.1")
        );
        assert_eq!(value.get("network"), Some(&Value::Bool(false)));
        assert_eq!(value.get("pstn").and_then(|v| v.as_str()), Some("not_run"));
        assert_eq!(value.get("claims_recognition"), Some(&Value::Bool(false)));
        assert_eq!(value.get("verdict").and_then(|v| v.as_str()), Some("green"));
        score::walk_keys(&value).unwrap();
        let cases = value.get("cases").and_then(|v| v.as_arr()).unwrap();
        assert!(cases.len() >= 10, "{}", cases.len());
        for case in cases {
            assert_eq!(case.get("claims_recognition"), Some(&Value::Bool(false)));
            assert_eq!(case.get("status").and_then(|v| v.as_str()), Some("pass"));
        }
        let interim = cases
            .iter()
            .find(|c| c.get("id").and_then(|v| v.as_str()) == Some("T1-interim-voice"))
            .unwrap();
        assert_eq!(
            interim.get("asr").and_then(|v| v.as_str()),
            Some("interim-placeholder")
        );
        assert_eq!(interim.get("fixture_decoded"), Some(&Value::Bool(false)));
        if summary.audio_captured {
            assert_eq!(summary.ttfa_status, "measured");
            assert!(summary.ttfa_ms.is_some());
            let name = cases
                .iter()
                .find(|c| c.get("id").and_then(|v| v.as_str()) == Some("T2-loopback-capture"))
                .and_then(|c| c.get("wav"))
                .and_then(|v| v.as_str())
                .unwrap();
            let bytes = fs::read(wav_dir.join(name)).unwrap();
            assert!(bytes.starts_with(b"RIFF"));
        } else {
            assert_eq!(summary.ttfa_status, "unmeasured");
            assert_eq!(summary.ttfa_ms, None);
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
