// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Inner life against the moment port, the stand-in seed, and GEMI.

use std::path::PathBuf;

use galion_emotion::consent_bits;
use galion_emotion_core::{
    learning_allowed, InnerLife, IntentMsg, IntentSource, VoiceBaseline, AROUSAL,
    BELIEF_AFTER_MISS, EXPECT_LEVEL, FRAME_MS, INTENT_BYTES, LEARNED_SLOTS, LEARN_LOCK,
    SURPRISE_AROUSAL,
};

fn standins() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("standins")
}

fn fresh() -> (InnerLife, galion_emotion_core::EmotionCore) {
    (
        InnerLife::seeded(),
        galion_emotion_core::EmotionCore::at_rest(),
    )
}

#[test]
fn same_dialogue_yields_the_same_life_and_the_same_gemi_bytes() {
    let run = || {
        let (mut life, mut core) = fresh();
        let mut bytes = Vec::new();
        for line in [b"pause".as_slice(), b"what?", b"kettle please"] {
            let turn = life.nudge(&mut core, line, None).unwrap();
            bytes.extend_from_slice(&turn.outbound.encode().unwrap());
        }
        (
            life,
            core.moment().pos,
            core.baseline().identity_bits(),
            bytes,
        )
    };
    let (a, a_pos, a_bits, a_bytes) = run();
    let (b, b_pos, b_bits, b_bytes) = run();
    assert_eq!(a, b);
    assert_eq!(a_pos, b_pos);
    assert_eq!(a_bits, b_bits);
    assert_eq!(a_bytes, b_bytes);
}

#[test]
fn expectation_mismatch_surprises_and_a_repeat_does_not() {
    let (mut life, mut core) = fresh();
    let baseline = core.baseline();
    let bits = baseline.identity_bits();

    let (mut quiet_life, mut quiet) = fresh();
    let quiet_turn = quiet_life.nudge(&mut quiet, b"pause", None).unwrap();
    assert_eq!(quiet_turn.surprise.to_bits(), 0.0f32.to_bits());
    assert_eq!(quiet_turn.outbound.verb_str(), "answer");
    assert_eq!(quiet.baseline(), baseline);

    let first = life.nudge(&mut core, b"what?", None).unwrap();
    assert_eq!(first.surprise.to_bits(), EXPECT_LEVEL.to_bits());
    assert_eq!(first.outbound.verb_str(), "surprise");
    assert_eq!(first.outbound.source, IntentSource::Own);
    assert_eq!(
        first.outbound.pull[AROUSAL].to_bits(),
        SURPRISE_AROUSAL.to_bits()
    );
    assert_eq!(core.baseline(), baseline);
    assert_eq!(core.baseline().identity_bits(), bits);
    assert_eq!(core.baseline().persona_id(), "standin-speaker");
    assert!(core.moment().pos[AROUSAL] > quiet.moment().pos[AROUSAL]);
    assert_eq!(life.expect(0).unwrap().token_str(), "question");
    assert_eq!(
        life.expect(0).unwrap().level().to_bits(),
        BELIEF_AFTER_MISS.to_bits()
    );
    assert_eq!(life.learned_count(), 0);

    let second = life.nudge(&mut core, b"what?", None).unwrap();
    assert_eq!(second.surprise.to_bits(), 0.0f32.to_bits());
    assert_eq!(core.baseline().identity_bits(), bits);
}

#[test]
fn the_baseline_stays_put_when_surprise_moves_the_moment() {
    let (mut life, mut core) = fresh();
    let before = core.baseline();
    let mut opener = IntentMsg::steady(0);
    opener.consent = consent_bits::SHARE;
    core.step(&opener).unwrap();
    assert_eq!(core.consent(), consent_bits::SHARE);

    let turn = life.nudge(&mut core, b"what time is it?", None).unwrap();
    assert!(turn.surprise > 0.0);
    assert_eq!(core.baseline(), before);
    assert_eq!(core.baseline().identity_bits(), before.identity_bits());
    assert_eq!(core.consent(), consent_bits::SHARE);
    assert_eq!(turn.outbound.consent, consent_bits::SHARE);
    assert_ne!(
        core.moment().pos[AROUSAL].to_bits(),
        before.pos[AROUSAL].to_bits()
    );
}

#[test]
fn gemi_roundtrip_own_then_inbound() {
    let (mut life, mut core) = fresh();
    let own = life.nudge(&mut core, b"what?", None).unwrap();
    let own_bytes = own.outbound.encode().unwrap();
    assert_eq!(own_bytes.len(), INTENT_BYTES);
    assert_eq!(&own_bytes[0..4], b"GEMI");
    assert_eq!(IntentMsg::decode(&own_bytes).unwrap(), own.outbound);
    assert_eq!(life.emit_gemi().unwrap(), own_bytes);
    assert_eq!(own.outbound.source, IntentSource::Own);

    let mut inbound = IntentMsg::hold(core.next_ms());
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("rush").unwrap();
    let in_bytes = inbound.encode().unwrap();
    assert_eq!(IntentMsg::decode(&in_bytes).unwrap(), inbound);

    let bits = core.baseline().identity_bits();
    let back = life.nudge(&mut core, b"", Some(&inbound)).unwrap();
    assert!(back.surprise > 0.0);
    assert_eq!(back.outbound.source, IntentSource::Own);
    let reply = back.outbound.encode().unwrap();
    assert_eq!(&reply[0..4], b"GEMI");
    assert_eq!(reply.len(), INTENT_BYTES);
    assert_eq!(IntentMsg::decode(&reply).unwrap(), back.outbound);
    assert_eq!(life.expect(0).unwrap().token_str(), "rush");
    assert_eq!(life.learned_count(), 0);
    assert_eq!(core.baseline().identity_bits(), bits);

    let consumed = {
        let mut reader = InnerLife::seeded();
        reader
            .consume_gemi(&in_bytes, 0, b"", core.moment().pos, 0)
            .unwrap()
    };
    assert_eq!(consumed.outbound.encode().unwrap()[0..4], *b"GEMI");
    assert!(consumed.surprise > 0.0);
}

#[test]
fn no_new_motive_while_learn_is_denied() {
    assert_eq!(LEARN_LOCK, "deny");
    assert!(!learning_allowed());
    let (mut life, mut core) = fresh();
    let care = life.care(0).unwrap();
    let want = life.want(0).unwrap();
    let expect = life.expect(0).unwrap();
    for _ in 0..6 {
        let turn = life.nudge(&mut core, b"kettle please", None).unwrap();
        assert_eq!(turn.surprise.to_bits(), 0.0f32.to_bits());
        assert!(!turn.learned);
        assert_eq!(turn.outbound.verb_str(), "answer");
    }
    assert_eq!(life.learned_count(), 0);
    assert!(life.learn_refused() >= 6);
    assert_eq!(life.care(0).unwrap().token_str(), care.token_str());
    assert_eq!(life.want(0).unwrap().token_str(), want.token_str());
    assert_eq!(life.expect(0).unwrap().token_str(), expect.token_str());
    for i in 0..LEARNED_SLOTS {
        assert!(life.learned(i).is_none());
    }
    assert_eq!(core.baseline(), VoiceBaseline::standin());
    assert_eq!(core.frames(), 6);
    assert_eq!(core.next_ms(), 6 * FRAME_MS);
}

#[test]
fn the_standin_file_seeds_the_layer_and_persona_stays_a_fixture() {
    let world = galion_emotion_core::load_standins(&standins()).unwrap();
    assert_eq!(world.inner_life(), InnerLife::seeded());
    assert_eq!(world.baseline.persona_id(), "standin-speaker");
    assert_eq!(world.memory.len(), 2);
    assert!(world.memory.iter().any(|card| card.id == "mem-desk"));
    let bits = world.baseline.identity_bits();
    let core = world.replay().unwrap();
    assert_eq!(core.baseline().identity_bits(), bits);
}
