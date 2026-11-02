use galion_sideband::*;
use core::time::Duration;

#[test]
fn cap_and_policy() {
    assert!(within_cap(Duration::from_millis(3000)));
    assert!(!within_cap(Duration::from_millis(3001)));
    assert!(!Intent::Action { name: "pay".into(), tier: ActionTier::Money }.voice_may_request());
    assert!(Intent::Lookup { query: "q".into() }.voice_may_request());
    let a = Affect { valence: 2.0, arousal: -3.0, confidence: 1.5 }.clamped();
    assert_eq!((a.valence, a.arousal, a.confidence), (1.0, -1.0, 1.0));
    let _ = FromHermes::NoKnowledge { turn: TurnId(1), reason: NoKnowledgeReason::Timeout };
}
