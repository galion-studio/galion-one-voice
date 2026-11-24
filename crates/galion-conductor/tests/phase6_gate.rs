// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Phase 6 gate. The side band is judged on the virtual frame clock.
//! 2.9 s merges, 3.1 s is dropped, barge-in cancels, a backchannel does not,
//! and an offline engine adds no frame.

use std::cell::RefCell;
use std::rc::Rc;

use galion_conductor::{
    stamp_log_is_lengths_only, AskResult, ClauseBrain, Conductor, Ear, FakeBrain, FakeResearch,
    PickedClause, PollResult, ResearchClient, SideBandView, TickEvent, TickOut, BARGE_IN_MAX_MS,
    FRAME_MS, SIDE_BAND_CAP_MS,
};

const QUESTION: &str = "why is the sky blue?";
const ANSWER: &str = "Shorter wavelengths scatter more.";

fn open(delay_ms: u64) -> Conductor<FakeBrain, FakeResearch> {
    Conductor::with_research(
        FakeBrain::new(),
        FakeResearch::ready_after(delay_ms, ANSWER),
    )
}

fn speak_until_commit<R: ResearchClient>(
    conductor: &mut Conductor<FakeBrain, R>,
    line: &str,
) -> (TickOut, TickOut) {
    let _ = conductor.tick(Ear::Speech(line), None);
    let _ = conductor.tick(Ear::Speech(line), None);
    let eager = conductor.tick(Ear::Quiet, None);
    assert_eq!(eager.event, TickEvent::EagerEndOfTurn);
    assert_eq!(eager.brain_calls, 1);
    assert!(
        eager.clause.is_none(),
        "the local clause stays behind the gate"
    );
    let commit = conductor.tick(Ear::Quiet, None);
    assert_eq!(commit.event, TickEvent::Committed);
    (eager, commit)
}

fn until_settled(conductor: &mut Conductor<FakeBrain, FakeResearch>) -> TickOut {
    for _ in 0..80 {
        let frame = conductor.tick(Ear::Quiet, None);
        match frame.side_band {
            SideBandView::Merged { .. } | SideBandView::Dropped { .. } => return frame,
            _ => {}
        }
    }
    panic!("side band did not settle inside 80 frames");
}

fn assert_trace_has_no_transcript(conductor: &Conductor<FakeBrain, FakeResearch>) {
    let log = conductor.side_log().join("\n");
    for word in ["sky", "blue", "scatter", "wavelength", "Shorter", "know"] {
        assert!(!log.contains(word), "trace leaked a word: {log}");
    }
}

#[test]
fn answer_at_2_9s_is_merged_and_the_local_clause_came_first() {
    let mut conductor = open(2_900);
    let (eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert!(matches!(eager.side_band, SideBandView::Pending { .. }));
    let local = commit.clause.as_ref().expect("local clause").clause.clone();
    assert_eq!(local, "I don't know.");
    assert!(commit.research_clause.is_none());
    assert_eq!(conductor.research().asks(), 1);
    assert_eq!(commit.brain_calls, 1);

    let settled = until_settled(&mut conductor);
    assert_eq!(
        settled.side_band,
        SideBandView::Merged { elapsed_ms: 2_900 }
    );
    assert_eq!(settled.research_clause.as_deref(), Some(ANSWER));
    assert_eq!(
        settled.clause.as_ref().expect("local still held").clause,
        local
    );
    assert!(!settled.cancel);
    assert_eq!(settled.brain_calls, 1);
    assert_eq!(conductor.side_cancels(), 0);
    assert_eq!(conductor.research().cancels(), 0);
    assert!(stamp_log_is_lengths_only(&settled.stamps.log_line()));
    assert_trace_has_no_transcript(&conductor);
    let line = settled.side_log_line.expect("merge line");
    assert!(line.contains("event=merged"));
    assert!(line.contains("elapsed_ms=2900"));
    assert!(!line.contains(ANSWER));
    println!(
        "phase6 gate: arrival_ms=2900 cap_ms={SIDE_BAND_CAP_MS} clock=virtual merged=true result=pass"
    );
}

#[test]
fn answer_at_3_1s_is_dropped_and_the_turn_keeps_the_local_clause() {
    let mut conductor = open(3_100);
    let (_eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert_eq!(
        commit.clause.as_ref().expect("local").clause,
        "I don't know."
    );

    let settled = until_settled(&mut conductor);
    let SideBandView::Dropped { elapsed_ms } = settled.side_band else {
        panic!("expected a drop, got {:?}", settled.side_band);
    };
    assert!(elapsed_ms > SIDE_BAND_CAP_MS);
    assert!(settled.research_clause.is_none());
    assert_eq!(
        settled.clause.as_ref().expect("local still held").clause,
        "I don't know."
    );
    assert_eq!(conductor.research().delay_ms(), 3_100);
    assert_eq!(
        conductor.research().cancels(),
        1,
        "the late ask is abandoned"
    );
    assert_eq!(conductor.side_cancels(), 0, "a drop is not a barge-in");
    assert!(!settled.cancel);
    assert_eq!(settled.brain_calls, 1);
    assert_trace_has_no_transcript(&conductor);
    let line = settled.side_log_line.expect("drop line");
    assert!(line.contains("event=dropped"));
    assert!(line.contains("cap_ms=3000"));
    assert!(!line.contains(QUESTION));
    assert!(!line.contains(ANSWER));
    println!(
        "phase6 gate: arrival_ms=3100 cap_ms={SIDE_BAND_CAP_MS} clock=virtual merged=false dropped=true drop_elapsed_ms={elapsed_ms} result=pass"
    );
}

#[test]
fn answer_at_the_3s_cap_is_merged() {
    let mut conductor = open(3_000);
    let (_eager, _commit) = speak_until_commit(&mut conductor, QUESTION);
    let settled = until_settled(&mut conductor);
    assert_eq!(
        settled.side_band,
        SideBandView::Merged { elapsed_ms: 3_000 }
    );
    assert_eq!(settled.research_clause.as_deref(), Some(ANSWER));
    assert_eq!(conductor.research().cancels(), 0);
}

#[test]
fn barge_in_during_a_pending_ask_yields_within_200ms_and_cancels() {
    let mut conductor = open(5_000);
    let (_eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert!(matches!(commit.side_band, SideBandView::Pending { .. }));
    let epoch = commit.epoch;

    let first = conductor.tick(Ear::Speech("stop talking now"), None);
    let onset = first.t_ms;
    assert!(!first.cancel);
    assert!(!first.me_silent);
    assert_eq!(conductor.side_cancels(), 0);

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
    assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
    assert_eq!(barge_ms, 160);
    assert!(last.cancel);
    assert_eq!(last.event, TickEvent::BargeIn);
    assert_eq!(last.side_band, SideBandView::Cancelled);
    assert!(last.epoch > epoch);
    assert!(last.research_clause.is_none());
    assert!(last.clause.is_none());
    assert_eq!(conductor.side_cancels(), 1);
    assert_eq!(conductor.research().cancels(), 1);

    for _ in 0..80 {
        let frame = conductor.tick(Ear::Quiet, None);
        assert!(
            !matches!(frame.side_band, SideBandView::Merged { .. }),
            "a cancelled ask must not merge"
        );
        assert!(frame.research_clause.is_none());
        assert_eq!(conductor.side_cancels(), 1);
    }
    println!(
        "phase6 gate: barge_in_ms={barge_ms} limit_ms={BARGE_IN_MAX_MS} clock=virtual pending_cancelled=true result=pass"
    );
}

#[test]
fn backchannel_yeah_during_a_pending_ask_does_not_cancel() {
    let mut conductor = open(2_900);
    let (_eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    let epoch = commit.epoch;
    for _ in 0..6 {
        let frame = conductor.tick(Ear::Speech("yeah"), None);
        assert!(!frame.cancel, "t_ms={}", frame.t_ms);
        assert!(!frame.me_silent, "t_ms={}", frame.t_ms);
        assert_eq!(frame.epoch, epoch);
        assert_eq!(frame.event, TickEvent::Backchannel);
        assert!(
            matches!(frame.side_band, SideBandView::Pending { .. }),
            "t_ms={} view={:?}",
            frame.t_ms,
            frame.side_band
        );
        assert_eq!(conductor.side_cancels(), 0);
        assert_eq!(conductor.research().cancels(), 0);
        assert_eq!(
            frame.clause.as_ref().expect("clause held").clause,
            "I don't know."
        );
    }
    let settled = until_settled(&mut conductor);
    assert_eq!(
        settled.side_band,
        SideBandView::Merged { elapsed_ms: 2_900 },
        "the ask stayed alive through the backchannel"
    );
    assert_eq!(conductor.side_cancels(), 0);
    assert_eq!(conductor.research().cancels(), 0);
    assert!(!settled.cancel);
    println!(
        "phase6 gate: backchannel=yeah frames=6 frame_ms={FRAME_MS} cancel=false request_cancelled=false result=pass"
    );
}

#[test]
fn offline_reports_unavailable_with_no_added_latency() {
    let started = std::time::Instant::now();
    let mut deep = Conductor::new(FakeBrain::new());
    let (eager, commit) = speak_until_commit(&mut deep, QUESTION);
    assert_eq!(eager.side_band, SideBandView::Unavailable);
    assert_eq!(deep.research().asks(), 1);
    assert!(eager.research_clause.is_none());
    assert_eq!(
        commit.clause.as_ref().expect("local").clause,
        "I don't know."
    );
    assert!(commit.research_clause.is_none());
    assert!(!commit.cancel);
    assert_eq!(commit.t_ms, eager.t_ms + u64::from(FRAME_MS));
    let log = deep.side_log().join("\n");
    assert!(log.contains("event=unavailable"));
    assert!(log.contains("elapsed_ms=0"));
    assert!(!log.contains("sky"));

    let mut kettle = Conductor::new(FakeBrain::new());
    let (kettle_eager, kettle_commit) = speak_until_commit(&mut kettle, "where is the kettle");
    assert_eq!(kettle_eager.side_band, SideBandView::Idle);
    assert_eq!(kettle.research().asks(), 0);
    assert_eq!(
        kettle_commit.clause.as_ref().expect("hit").clause,
        "The kettle is on the hob."
    );
    let added = i64::try_from(commit.t_ms).unwrap() - i64::try_from(kettle_commit.t_ms).unwrap();
    assert_eq!(added, 0, "offline lookup must not insert a frame");
    assert_eq!(eager.t_ms, kettle_eager.t_ms);
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "the offline turn waited on the wall clock"
    );
    println!("phase6 gate: offline=unavailable added_latency_ms={added} clock=virtual result=pass");
}

#[test]
fn a_local_hit_does_not_ask_and_a_payment_is_refused() {
    let mut hit = Conductor::with_research(
        FakeBrain::new(),
        FakeResearch::ready_after(0, "should not be asked"),
    );
    let (eager, commit) = speak_until_commit(&mut hit, "where is the kettle");
    assert_eq!(eager.side_band, SideBandView::Idle);
    assert_eq!(hit.research().asks(), 0);
    assert!(commit.research_clause.is_none());
    assert_eq!(
        commit.clause.as_ref().expect("hit").clause,
        "The kettle is on the hob."
    );

    let mut pay = Conductor::with_research(
        FakeBrain::new(),
        FakeResearch::ready_after(0, "should not be asked"),
    );
    let (eager, commit) = speak_until_commit(&mut pay, "send money to pat today");
    assert_eq!(eager.side_band, SideBandView::Refused);
    assert_eq!(pay.research().asks(), 0);
    assert!(commit.research_clause.is_none());
    assert_eq!(
        commit.clause.as_ref().expect("local").clause,
        "I don't know."
    );
    let log = pay.side_log().join("\n");
    assert!(log.contains("event=refused"));
    assert!(!log.contains("pat"));
    assert!(!log.contains("send"));
}

#[test]
fn resume_during_speculation_cancels_the_pending_ask() {
    let mut conductor = open(2_900);
    let _ = conductor.tick(Ear::Speech(QUESTION), None);
    let _ = conductor.tick(Ear::Speech(QUESTION), None);
    let eager = conductor.tick(Ear::Quiet, None);
    assert!(matches!(eager.side_band, SideBandView::Pending { .. }));
    let resumed = conductor.tick(Ear::Speech("wait the other one"), None);
    assert_eq!(resumed.event, TickEvent::TurnResumed);
    assert_eq!(resumed.side_band, SideBandView::Cancelled);
    assert_eq!(conductor.side_cancels(), 1);
    assert_eq!(conductor.research().cancels(), 1);
    assert!(resumed.research_clause.is_none());
    for _ in 0..80 {
        let frame = conductor.tick(Ear::Quiet, None);
        assert!(frame.research_clause.is_none());
        assert!(!matches!(frame.side_band, SideBandView::Merged { .. }));
    }
    assert_eq!(conductor.research().asks(), 1);
}

struct OrderBrain {
    log: Rc<RefCell<Vec<&'static str>>>,
}

struct OrderClient {
    log: Rc<RefCell<Vec<&'static str>>>,
}

impl ClauseBrain for OrderBrain {
    fn answer(&self, query: &str) -> PickedClause {
        self.log.borrow_mut().push("brain");
        FakeBrain::new().answer(query)
    }
}

impl ResearchClient for OrderClient {
    fn ask(&mut self, _query: &str, _now_ms: u64) -> AskResult {
        self.log.borrow_mut().push("research");
        AskResult::Unavailable
    }

    fn poll(&mut self, _id: u64, _now_ms: u64) -> PollResult {
        PollResult::Gone
    }

    fn cancel(&mut self, _id: u64) {}
}

#[test]
fn the_local_brain_answers_before_the_side_band_is_asked() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut conductor = Conductor::with_research(
        OrderBrain {
            log: Rc::clone(&log),
        },
        OrderClient {
            log: Rc::clone(&log),
        },
    );
    let (eager, commit) = {
        let _ = conductor.tick(Ear::Speech(QUESTION), None);
        let _ = conductor.tick(Ear::Speech(QUESTION), None);
        let eager = conductor.tick(Ear::Quiet, None);
        let commit = conductor.tick(Ear::Quiet, None);
        (eager, commit)
    };
    assert_eq!(log.borrow().as_slice(), ["brain", "research"]);
    assert_eq!(eager.side_band, SideBandView::Unavailable);
    assert_eq!(
        commit.clause.as_ref().expect("local").clause,
        "I don't know."
    );
    assert_eq!(commit.t_ms - eager.t_ms, u64::from(FRAME_MS));
}

#[test]
fn locks_name_the_side_band_cap() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(&path).expect("LOCKS.toml");
    assert!(text.contains("side_band_cap_ms = 3000"));
    assert!(text.contains("backchannel_cancels = false"));
    assert!(text.contains("barge_in_max_ms = 200"));
    assert!(text.contains("default = \"deny\""));
    assert_eq!(SIDE_BAND_CAP_MS, 3_000);
}
