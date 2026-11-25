//! Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Galion Labs.

use std::time::Duration;

use galion_sideband::mark::{MarkEdge, MarkKind, MarkLog, SidebandConsent, MARK_SOURCE};
use galion_sideband::{
    within_cap, FromHermes, Intent, NoKnowledgeReason, ToHermes, TurnId, LEARN_IN_BAND, LEARN_LOCK,
    REMEMBER_LOCK, RETRIEVAL_CAP,
};

fn learning_rides_the_band() -> bool {
    LEARN_IN_BAND
}

#[test]
fn locks_stay_denied_and_learning_stays_off_the_band() {
    assert_eq!(REMEMBER_LOCK, "deny");
    assert_eq!(LEARN_LOCK, "deny");
    assert!(!learning_rides_the_band());
    assert!(SidebandConsent::Post.may_post());
    assert!(!SidebandConsent::Deny.may_post());
    assert_eq!(SidebandConsent::default(), SidebandConsent::Post);
}

#[test]
fn start_and_end_use_the_existing_wire() {
    let mut log = MarkLog::new();
    log.post_start(SidebandConsent::Post, 7, "why is the sky blue?", 160);
    log.post_end(
        7,
        MarkKind::Merged {
            text: "Shorter wavelengths scatter more.".into(),
        },
        3_120,
    );
    let marks = log.marks();
    assert_eq!(marks.len(), 2);
    assert!(marks.iter().all(|mark| mark.posted));
    assert_eq!(marks[0].edge, MarkEdge::Start);
    assert_eq!(
        marks[0].outbound(),
        Some(ToHermes::Intent {
            turn: TurnId(7),
            intent: Intent::Lookup {
                query: "why is the sky blue?".into(),
            },
        })
    );
    assert!(marks[0].inbound().is_none());
    assert_eq!(
        marks[1].inbound(),
        Some(FromHermes::Knowledge {
            turn: TurnId(7),
            text: "Shorter wavelengths scatter more.".into(),
            source: Some(MARK_SOURCE.into()),
        })
    );
    assert!(marks[1].outbound().is_none());
    assert!(!MARK_SOURCE.contains("http"));
    assert!(!MARK_SOURCE.contains('.'));
}

#[test]
fn cap_ends_are_timeout_cancel_and_unreachable() {
    assert_eq!(RETRIEVAL_CAP, Duration::from_millis(3_000));
    assert!(within_cap(Duration::from_millis(3_000)));
    assert!(!within_cap(Duration::from_millis(3_001)));

    let mut log = MarkLog::new();
    log.post_start(SidebandConsent::Post, 1, "why?", 160);
    log.post_end(1, MarkKind::Dropped, 3_200);
    assert_eq!(
        log.marks()[1].inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(1),
            reason: NoKnowledgeReason::Timeout,
        })
    );

    let mut cancelled = MarkLog::new();
    cancelled.post_start(SidebandConsent::Post, 2, "why?", 160);
    cancelled.post_end(2, MarkKind::Cancelled, 320);
    assert_eq!(
        cancelled.marks()[1].inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(2),
            reason: NoKnowledgeReason::Cancelled,
        })
    );

    let mut offline = MarkLog::new();
    offline.post_start(SidebandConsent::Post, 3, "why?", 160);
    offline.post_end(3, MarkKind::Unavailable, 160);
    assert_eq!(
        offline.marks()[1].inbound(),
        Some(FromHermes::NoKnowledge {
            turn: TurnId(3),
            reason: NoKnowledgeReason::Unreachable,
        })
    );
}

#[test]
fn a_denied_session_posts_nothing_and_an_unopened_end_is_ignored() {
    let mut log = MarkLog::new();
    log.post_start(SidebandConsent::Deny, 4, "why is the sky blue?", 160);
    log.post_end(4, MarkKind::Dropped, 3_200);
    assert!(log.is_empty());

    let mut orphan = MarkLog::new();
    orphan.post_end(
        9,
        MarkKind::Merged {
            text: "no start".into(),
        },
        10,
    );
    assert!(orphan.is_empty());
}
