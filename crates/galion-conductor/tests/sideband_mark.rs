//! Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! The conductor posts the side-band mark when a Galion Research request
//! starts and ends. The cap, a barge-in, and a denied session are judged
//! on the virtual frame clock.

use galion_conductor::{
    Conductor, Ear, FakeBrain, FakeResearch, MarkEdge, MarkKind, ResearchClient, SideBandView,
    SidebandConsent, SidebandMark, TickEvent, TickOut, BARGE_IN_MAX_MS, FRAME_MS, LEARN_IN_BAND,
    LEARN_LOCK, MARK_SOURCE, REMEMBER_LOCK, SIDE_BAND_CAP_MS,
};
use galion_sideband::{FromHermes, Intent, NoKnowledgeReason, ToHermes, TurnId};

const QUESTION: &str = "why is the sky blue?";
const ANSWER: &str = "Shorter wavelengths scatter more.";

fn learning_rides_the_band() -> bool {
    LEARN_IN_BAND
}

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
    for word in ["sky", "blue", "scatter", "wavelength", "Shorter"] {
        assert!(!log.contains(word), "trace leaked a word: {log}");
    }
}

fn start_mark(marks: &[SidebandMark]) -> &SidebandMark {
    marks
        .iter()
        .find(|mark| mark.edge == MarkEdge::Start)
        .expect("start mark")
}

fn end_mark(marks: &[SidebandMark]) -> &SidebandMark {
    marks
        .iter()
        .find(|mark| mark.edge == MarkEdge::End)
        .expect("end mark")
}

#[test]
fn a_lookup_posts_a_start_mark_and_an_end_mark_inside_the_cap() {
    assert_eq!(REMEMBER_LOCK, "deny");
    assert_eq!(LEARN_LOCK, "deny");
    assert!(!learning_rides_the_band());
    assert_eq!(SIDE_BAND_CAP_MS, 3_000);

    let mut conductor = open(2_900);
    assert_eq!(conductor.sideband_consent(), SidebandConsent::Post);
    let (eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert!(matches!(eager.side_band, SideBandView::Pending { .. }));
    assert!(commit.research_clause.is_none());

    let started = start_mark(conductor.marks());
    assert!(started.posted);
    assert_eq!(started.at_ms, eager.t_ms);
    assert_eq!(
        started.kind,
        MarkKind::Lookup {
            query: QUESTION.into(),
        }
    );
    assert_eq!(
        started.outbound(),
        Some(ToHermes::Intent {
            turn: TurnId(started.turn),
            intent: Intent::Lookup {
                query: QUESTION.into(),
            },
        })
    );
    assert!(conductor
        .marks()
        .iter()
        .all(|mark| mark.edge == MarkEdge::Start));

    let settled = until_settled(&mut conductor);
    assert_eq!(
        settled.side_band,
        SideBandView::Merged { elapsed_ms: 2_900 }
    );
    assert_eq!(settled.research_clause.as_deref(), Some(ANSWER));
    assert!(settled.t_ms - start_mark(conductor.marks()).at_ms >= 2_900);

    let marks = conductor.marks();
    assert_eq!(marks.len(), 2);
    let ended = end_mark(marks);
    assert!(ended.posted);
    assert_eq!(ended.turn, start_mark(marks).turn);
    assert_eq!(ended.at_ms, settled.t_ms);
    assert_eq!(
        ended.kind,
        MarkKind::Merged {
            text: ANSWER.into(),
        }
    );
    assert_eq!(
        ended.inbound(),
        Some(FromHermes::Knowledge {
            turn: TurnId(ended.turn),
            text: ANSWER.into(),
            source: Some(MARK_SOURCE.into()),
        })
    );
    assert_trace_has_no_transcript(&conductor);
    println!(
        "phase6 mark: posted=true edge=start+end arrival_ms=2900 cap_ms={SIDE_BAND_CAP_MS} result=pass"
    );
}

#[test]
fn a_late_answer_posts_a_dropped_end_and_the_cap_holds() {
    let mut conductor = open(3_100);
    let (eager, _) = speak_until_commit(&mut conductor, QUESTION);
    let settled = until_settled(&mut conductor);
    let SideBandView::Dropped { elapsed_ms } = settled.side_band else {
        panic!("expected a drop, got {:?}", settled.side_band);
    };
    assert!(elapsed_ms > SIDE_BAND_CAP_MS);
    assert!(settled.research_clause.is_none());
    assert_eq!(conductor.research().cancels(), 1);

    let marks = conductor.marks();
    assert_eq!(marks.len(), 2);
    assert_eq!(start_mark(marks).at_ms, eager.t_ms);
    let ended = end_mark(marks);
    assert_eq!(ended.kind, MarkKind::Dropped);
    assert_eq!(ended.at_ms, settled.t_ms);
    assert_eq!(
        ended.inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(ended.turn),
            reason: NoKnowledgeReason::Timeout,
        })
    );
    assert!(marks
        .iter()
        .all(|mark| !matches!(mark.kind, MarkKind::Merged { .. })));
    assert_trace_has_no_transcript(&conductor);
    println!(
        "phase6 mark: posted=true edge=end kind=dropped elapsed_ms={elapsed_ms} cap_ms={SIDE_BAND_CAP_MS} result=pass"
    );
}

#[test]
fn barge_in_posts_a_cancelled_end_and_nothing_arrives_later() {
    let mut conductor = open(5_000);
    let (_eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert!(matches!(commit.side_band, SideBandView::Pending { .. }));
    assert_eq!(conductor.marks().len(), 1);

    let first = conductor.tick(Ear::Speech("stop talking now"), None);
    let onset = first.t_ms;
    let mut last = first;
    let mut silence_at = None;
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
    assert_eq!(last.event, TickEvent::BargeIn);
    assert_eq!(last.side_band, SideBandView::Cancelled);
    assert_eq!(conductor.research().cancels(), 1);

    let ended = end_mark(conductor.marks());
    assert_eq!(ended.kind, MarkKind::Cancelled);
    assert_eq!(ended.at_ms, last.t_ms);
    assert_eq!(
        ended.inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(ended.turn),
            reason: NoKnowledgeReason::Cancelled,
        })
    );

    for _ in 0..80 {
        let frame = conductor.tick(Ear::Quiet, None);
        assert!(frame.research_clause.is_none());
        assert!(!matches!(frame.side_band, SideBandView::Merged { .. }));
    }
    assert_eq!(conductor.marks().len(), 2);
    assert!(conductor
        .marks()
        .iter()
        .all(|mark| !matches!(mark.kind, MarkKind::Merged { .. })));
    println!("phase6 mark: posted=true edge=end kind=cancelled barge_in_ms={barge_ms} result=pass");
}

#[test]
fn a_denied_session_asks_and_posts_no_mark() {
    let mut conductor = open(2_900);
    conductor.set_sideband_consent(SidebandConsent::Deny);
    assert!(!conductor.sideband_consent().may_post());
    assert_eq!(REMEMBER_LOCK, "deny");
    assert_eq!(LEARN_LOCK, "deny");
    assert!(!learning_rides_the_band());

    let (eager, _) = speak_until_commit(&mut conductor, QUESTION);
    assert!(matches!(eager.side_band, SideBandView::Pending { .. }));
    assert_eq!(conductor.research().asks(), 1);
    assert!(conductor.marks().is_empty());

    let settled = until_settled(&mut conductor);
    assert_eq!(
        settled.side_band,
        SideBandView::Merged { elapsed_ms: 2_900 }
    );
    assert_eq!(settled.research_clause.as_deref(), Some(ANSWER));
    assert!(conductor.marks().is_empty());
    assert_trace_has_no_transcript(&conductor);

    let mut refused = open(0);
    refused.set_sideband_consent(SidebandConsent::Deny);
    let (pay, _) = speak_until_commit(&mut refused, "send money to pat today");
    assert_eq!(pay.side_band, SideBandView::Refused);
    assert_eq!(refused.research().asks(), 0);
    assert!(refused.marks().is_empty());

    let mut allowed_pay = open(0);
    let (pay, _) = speak_until_commit(&mut allowed_pay, "send money to pat today");
    assert_eq!(pay.side_band, SideBandView::Refused);
    assert_eq!(allowed_pay.research().asks(), 0);
    assert!(allowed_pay.marks().is_empty());
    println!("phase6 mark: consent=deny posted=false learn_in_band=false result=pass");
}

#[test]
fn an_offline_lookup_posts_start_and_end_on_the_ask_frame() {
    let mut conductor = Conductor::new(FakeBrain::new());
    let (eager, commit) = speak_until_commit(&mut conductor, QUESTION);
    assert_eq!(eager.side_band, SideBandView::Unavailable);
    assert_eq!(commit.t_ms, eager.t_ms + u64::from(FRAME_MS));
    let marks = conductor.marks();
    assert_eq!(marks.len(), 2);
    assert_eq!(marks[0].at_ms, eager.t_ms);
    assert_eq!(marks[1].at_ms, eager.t_ms);
    assert_eq!(marks[1].kind, MarkKind::Unavailable);
    assert_eq!(
        marks[1].inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(marks[1].turn),
            reason: NoKnowledgeReason::Unreachable,
        })
    );
    assert_eq!(conductor.marks().len(), 2);

    let mut kettle = Conductor::new(FakeBrain::new());
    let (hit, _) = speak_until_commit(&mut kettle, "where is the kettle");
    assert_eq!(hit.side_band, SideBandView::Idle);
    assert!(kettle.marks().is_empty());
}

#[test]
fn the_lock_file_still_denies_remember_and_learn() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(path).expect("LOCKS.toml");
    assert!(text.contains("remember = \"deny\""));
    assert!(text.contains("learn = \"deny\""));
    assert!(text.contains("default = \"deny\""));
    assert!(!text.contains("learn_in_band = true"));
    assert_eq!(REMEMBER_LOCK, "deny");
    assert_eq!(LEARN_LOCK, "deny");
    assert!(!learning_rides_the_band());
}
