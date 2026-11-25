// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Phase 5 gate. Times are virtual frames. A sleep in one test shows the
//! counter does not follow the wall.

use galion_conductor::{
    is_listed_backchannel, stamp_log_is_lengths_only, stand_in_listen_intent, synthetic_clip,
    Conductor, Ear, FakeBrain, Floor, ReactionKind, TickEvent, BARGE_IN_MAX_MS, FRAME_MS,
    REACTION_SAMPLES,
};
use galion_mne_contract::{validate, DocKind};

fn to_speaking(conductor: &mut Conductor<FakeBrain>, line: &str) -> galion_conductor::TickOut {
    let _ = conductor.tick(Ear::Speech(line), None);
    let _ = conductor.tick(Ear::Speech(line), None);
    let eager = conductor.tick(Ear::Quiet, None);
    assert_eq!(eager.event, TickEvent::EagerEndOfTurn);
    let commit = conductor.tick(Ear::Quiet, None);
    assert_eq!(commit.event, TickEvent::Committed);
    commit
}

#[test]
fn barge_in_is_at_most_200ms_on_the_virtual_clock() {
    let mut conductor = Conductor::new(FakeBrain::new());
    let commit = to_speaking(&mut conductor, "where is the kettle");
    assert!(commit.clause.is_some());
    let epoch = commit.epoch;

    let first = conductor.tick(Ear::Speech("stop talking now"), None);
    let onset = first.t_ms;
    assert!(!first.me_silent, "the arming frame still plays");
    assert!(!first.cancel);

    let mut silence_at = None;
    let mut last = first;
    for _ in 0..6 {
        if last.me_silent {
            silence_at = Some(last.t_ms);
            break;
        }
        last = conductor.tick(Ear::Speech("stop talking now"), None);
    }
    let silence_at = silence_at.expect("me band went silent");
    let barge_ms = silence_at - onset;
    assert!(
        barge_ms <= u64::from(BARGE_IN_MAX_MS),
        "barge_in_ms={barge_ms} limit={BARGE_IN_MAX_MS}"
    );
    assert_eq!(
        barge_ms, 160,
        "silence starts on the frame that contains 200 ms"
    );
    assert!(barge_ms % u64::from(FRAME_MS) == 0);
    assert!(last.cancel);
    assert_eq!(last.event, TickEvent::BargeIn);
    assert!(last.epoch > epoch);
    assert!(last.clause.is_none());
    assert_eq!(last.floor, Floor::Paused);
    println!(
        "phase5 gate: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} clock=virtual frame_ms={FRAME_MS} result=pass"
    );
}

#[test]
fn listed_backchannel_does_not_cancel() {
    let mut conductor = Conductor::new(FakeBrain::new());
    let commit = to_speaking(&mut conductor, "where is the kettle");
    let epoch = commit.epoch;
    let t_commit = commit.t_ms;
    let clause = commit
        .clause
        .as_ref()
        .expect("committed clause")
        .clause
        .clone();
    assert!(is_listed_backchannel("yeah"));
    assert!(is_listed_backchannel("Mm-hm."));

    let mut last = commit;
    for _ in 0..6 {
        last = conductor.tick(Ear::Speech("yeah"), None);
        assert!(!last.cancel, "t_ms={}", last.t_ms);
        assert!(!last.me_silent, "t_ms={}", last.t_ms);
        assert_eq!(last.epoch, epoch);
        assert_eq!(last.event, TickEvent::Backchannel);
        assert_eq!(last.clause.as_ref().expect("clause held").clause, clause);
    }
    let elapsed = last.t_ms - t_commit;
    assert!(elapsed > u64::from(BARGE_IN_MAX_MS));
    println!(
        "phase5 gate: backchannel=yeah frames=6 frame_ms={FRAME_MS} span_ms={elapsed} cancel=false epoch_unchanged=true result=pass"
    );
}

#[test]
fn eager_end_of_turn_emits_silence_and_a_later_resume_rolls_the_epoch_back() {
    let mut conductor = Conductor::new(FakeBrain::new());
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let eager = conductor.tick(Ear::Quiet, None);
    assert_eq!(eager.event, TickEvent::EagerEndOfTurn);
    assert!(eager.speculative);
    assert!(eager.clause.is_none(), "the clause stays behind the gate");
    assert_eq!(eager.brain_calls, 1);
    let reaction = eager.reaction.expect("wordless reaction");
    assert_eq!(reaction.kind, ReactionKind::Silence);
    assert_eq!(reaction.samples, REACTION_SAMPLES);
    assert!(reaction.pcm().iter().all(|sample| *sample == 0));
    let silence = eager.stamps.t_vad_silence_start.expect("silence stamp");
    let eou = eager.stamps.t_eou_decided.expect("eou stamp");
    assert_eq!(eou - silence, u64::from(FRAME_MS) * 1_000);
    assert!(eager.stamps.t_llm_req.is_some());
    assert!(eager.stamps.t_llm_first_token.is_some());
    assert!(eager.stamps.t_first_media_frame_sent.is_some());
    assert!(eager.stamps.t_first_clause_flushed.is_none());
    assert!(eager.stamps.t_tts_first_pcm.is_none());
    assert!(eager.stamps.t_first_audio_heard.is_none());
    let line = eager.stamps.log_line();
    assert!(stamp_log_is_lengths_only(&line), "{line}");
    assert!(!line.contains("kettle"));
    assert!(!line.contains("RIFF"));

    let epoch = eager.epoch;
    let resumed = conductor.tick(Ear::Speech("wait the other one"), None);
    assert_eq!(resumed.event, TickEvent::TurnResumed);
    assert_eq!(resumed.epoch, epoch + 1);
    assert_eq!(resumed.rollbacks, 1);
    assert!(!resumed.cancel);
    assert!(!resumed.speculative);
    assert!(resumed.clause.is_none());
    assert_eq!(resumed.brain_calls, 1);
    assert!(resumed.stamps.t_eou_decided.is_none());
    assert!(resumed.stamps.t_first_clause_flushed.is_none());
    assert!(stamp_log_is_lengths_only(&resumed.stamps.log_line()));
}

#[test]
fn a_held_silence_commits_the_fixture_clause_and_a_miss_speaks_the_persona_line() {
    let mut hit = Conductor::new(FakeBrain::new());
    let commit = to_speaking(&mut hit, "where is the kettle");
    let clause = commit.clause.as_ref().expect("hit");
    assert_eq!(clause.clause, "The kettle is on the hob.");
    assert_eq!(clause.lead, "The kettle");
    assert_eq!(clause.rest, "is on the hob.");
    assert!(clause.hit);
    assert!(commit.stamps.t_first_clause_flushed.is_some());
    assert_eq!(
        commit.stamps.reply_chars,
        clause.clause.chars().count() as u32
    );
    let line = commit.stamps.log_line();
    assert!(stamp_log_is_lengths_only(&line), "{line}");
    assert!(!line.contains("kettle"));
    assert!(!line.contains("hob"));

    let mut miss = Conductor::new(FakeBrain::new());
    let commit = to_speaking(&mut miss, "what is the weather");
    let clause = commit.clause.as_ref().expect("miss");
    assert_eq!(clause.id, "horbm-say-i-dont-know");
    assert_eq!(clause.clause, "I don't know.");
    assert!(!clause.hit);
    assert!(stamp_log_is_lengths_only(&commit.stamps.log_line()));
    assert!(!commit.stamps.log_line().contains("know"));
}

#[test]
fn synthetic_clip_is_a_bounded_sine_and_not_a_file() {
    let mut conductor = Conductor::with_reaction(FakeBrain::new(), ReactionKind::Synthetic);
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let eager = conductor.tick(Ear::Quiet, None);
    let reaction = eager.reaction.expect("synthetic reaction");
    assert_eq!(reaction.kind, ReactionKind::Synthetic);
    let pcm = reaction.pcm();
    assert_eq!(pcm, synthetic_clip());
    assert!(pcm.iter().any(|sample| *sample != 0));
    assert!(pcm.iter().all(|sample| sample.abs() <= 1000));
}

#[test]
fn mne_intent_comes_back_as_a_readback() {
    let intent = stand_in_listen_intent();
    validate(&intent).expect("stand-in intent");
    let mut conductor = Conductor::new(FakeBrain::new());
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let _ = conductor.tick(Ear::Speech("where is the kettle"), None);
    let eager = conductor.tick(Ear::Quiet, Some(&intent));
    let doc = eager
        .readback
        .expect("read-back present")
        .expect("read-back valid");
    validate(&doc).expect("read-back validates");
    assert_eq!(doc.kind, DocKind::Readback);
    assert!(doc.intent_frames.is_empty());
    assert_eq!(doc.readback_frames.len(), 1);
    assert_eq!(doc.readback_frames[0].body.nose.nasalance, Some(0.35));
    assert!(doc.text.is_empty());

    let mut readback_as_intent = doc.clone();
    readback_as_intent.kind = DocKind::Readback;
    let rejected = conductor.tick(Ear::Quiet, Some(&readback_as_intent));
    assert!(rejected.readback.expect("attempted").is_err());
}

#[test]
fn a_wall_sleep_does_not_move_the_virtual_clock() {
    let mut conductor = Conductor::new(FakeBrain::new());
    let first = conductor.tick(Ear::Quiet, None);
    assert_eq!(first.t_ms, 0);
    std::thread::sleep(std::time::Duration::from_millis(30));
    let second = conductor.tick(Ear::Quiet, None);
    assert_eq!(second.t_ms, u64::from(FRAME_MS));
}

#[test]
fn locks_file_matches_the_conductor_gate() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(&path).expect("LOCKS.toml");
    assert!(
        text.contains("barge_in_max_ms = 200"),
        "barge-in lock stays 200 ms"
    );
    assert!(
        text.contains("backchannel_cancels = false"),
        "a listed backchannel does not cancel"
    );
    assert!(
        text.contains("side_band_cap_ms = 3000"),
        "side band cap stays 3000 ms"
    );
    assert!(text.contains("frame_ms = 80"));
    assert!(
        text.contains("default = \"deny\""),
        "consent default stays deny"
    );
    assert_eq!(BARGE_IN_MAX_MS, 200);
    assert_eq!(FRAME_MS, 80);
}

#[test]
fn crate_sources_do_not_call_the_frame_step_or_read_a_wall_clock() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = 0u32;
    for entry in std::fs::read_dir(&root).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        files += 1;
        for banned in [
            "Heartbeat::step",
            "std::time::Instant",
            "SystemTime",
            "tokio",
            "TcpStream",
            "std::net",
            "target_feature",
            "Hermes",
            "hermes",
            "SidebandClient",
        ] {
            assert!(
                !text.contains(banned),
                "{} contains {banned}",
                path.display()
            );
        }
    }
    assert!(files >= 6);
}
