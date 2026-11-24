// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Inner life: same input, same output; surprise nudges the moment; the
//! baseline stays; GEMI travels both ways; learning stays off.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use galion_emotion_core::{
    load_standins, EmotionCore, InnerLife, IntentMsg, IntentSource, TurnIn, VoiceBaseline, AROUSAL,
    FRAME_MS, INTENT_BYTES, SEED_CARE, SEED_EXPECT, SEED_STRENGTH, SEED_WANT, SURPRISE_MIN,
    TURN_ALLOC_BUDGET, VALENCE,
};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn rest_point() -> [f32; 4] {
    InnerLife::at_rest().expect().point
}

fn jumped(rest: [f32; 4]) -> [f32; 4] {
    let mut point = rest;
    point[AROUSAL] = (rest[AROUSAL] + 0.5).min(1.0);
    point
}

fn run_script() -> (InnerLife, [[u8; INTENT_BYTES]; 3]) {
    let mut life = InnerLife::at_rest();
    let rest = life.expect().point;
    let inputs = [
        TurnIn {
            line: "speaking",
            point: rest,
            t_ms: 0,
            consent: 0,
        },
        TurnIn {
            line: "thanks speaking",
            point: rest,
            t_ms: FRAME_MS,
            consent: 0,
        },
        TurnIn {
            line: "refused",
            point: jumped(rest),
            t_ms: 2 * FRAME_MS,
            consent: 0,
        },
    ];
    let mut bytes = [[0u8; INTENT_BYTES]; 3];
    for (i, input) in inputs.into_iter().enumerate() {
        let out = life.turn(input);
        bytes[i] = out.intent.encode().expect("outbound GEMI");
    }
    (life, bytes)
}

fn step_for(intent: IntentMsg, frames: u32) -> EmotionCore {
    let mut core = EmotionCore::at_rest();
    for i in 0..frames {
        let mut msg = intent;
        msg.t_ms = i * FRAME_MS;
        core.step(&msg).unwrap();
    }
    core
}

#[test]
fn same_dialogue_and_point_produce_the_same_bytes() {
    let (a, bytes_a) = run_script();
    let (b, bytes_b) = run_script();
    assert_eq!(a, b);
    assert_eq!(bytes_a, bytes_b);
    assert_eq!(a.turns(), 3);
    assert_eq!(bytes_a[0].len(), INTENT_BYTES);
    assert_eq!(&bytes_a[2][0..4], b"GEMI");
}

#[test]
fn expectation_mismatch_surprises_and_leaves_the_baseline() {
    let baseline = VoiceBaseline::standin();
    let bits = baseline.identity_bits();
    let rest = rest_point();

    let mut matched_life = InnerLife::at_rest();
    let matched = matched_life.turn(TurnIn {
        line: "speaking",
        point: rest,
        t_ms: 0,
        consent: 0,
    });
    assert_eq!(matched.surprise, 0.0);
    assert_eq!(matched.word_gap, 0.0);
    assert_eq!(matched.intent.verb_str(), "hold");
    assert_eq!(matched.intent.strength, 0.0);
    assert_eq!(matched.intent.pull_mask, 0);
    let calm = step_for(matched.intent, 40);
    assert_eq!(calm.baseline().identity_bits(), bits);
    for i in 0..4 {
        assert_eq!(
            calm.moment().pos[i].to_bits(),
            calm.baseline().pos[i].to_bits()
        );
    }

    let mut point_life = InnerLife::at_rest();
    let point = point_life.turn(TurnIn {
        line: "speaking",
        point: jumped(rest),
        t_ms: 0,
        consent: 0,
    });
    assert!(point.surprise >= SURPRISE_MIN);
    assert!(point.surprise > matched.surprise);
    assert!(point.point_gap > 0.0);
    assert_eq!(point.word_gap, 0.0);
    assert_eq!(point.intent.verb_str(), "surprise");
    assert_eq!(point.intent.source, IntentSource::Own);
    assert!(point.intent.pull[AROUSAL] > 0.0);
    assert!(point.intent.strength > 0.0);

    let mut word_life = InnerLife::at_rest();
    let word = word_life.turn(TurnIn {
        line: "refused",
        point: rest,
        t_ms: 0,
        consent: 0,
    });
    assert!(word.surprise >= SURPRISE_MIN);
    assert_eq!(word.word_gap, 1.0);
    assert_eq!(word.intent.verb_str(), "surprise");
    assert!(word.intent.pull[AROUSAL] > 0.0);

    let nudged = step_for(word.intent, 40);
    assert_eq!(nudged.baseline().identity_bits(), bits);
    assert_eq!(nudged.baseline(), baseline);
    assert_eq!(nudged.baseline().persona_id(), "standin-speaker");
    assert!(nudged.moment().pos[AROUSAL] > nudged.baseline().arousal());
    assert!(nudged.moment().pos[AROUSAL] > calm.moment().pos[AROUSAL]);
    assert_eq!(baseline.identity_bits(), bits);
}

#[test]
fn dialogue_moves_the_next_expectation() {
    let rest = rest_point();
    let mut plain = InnerLife::at_rest();
    plain.turn(TurnIn {
        line: "speaking",
        point: rest,
        t_ms: 0,
        consent: 0,
    });
    let mut warm = InnerLife::at_rest();
    warm.turn(TurnIn {
        line: "thanks speaking",
        point: rest,
        t_ms: 0,
        consent: 0,
    });
    assert!(warm.expect().point[VALENCE] > plain.expect().point[VALENCE]);
    assert_eq!(warm.expect().cue_str(), "thanks");
    assert_eq!(plain.expect().cue_str(), "speaking");
}

#[test]
fn gemi_roundtrip_both_directions() {
    let mut life = InnerLife::at_rest();
    let rest = life.expect().point;
    let out = life.turn(TurnIn {
        line: "speaking",
        point: jumped(rest),
        t_ms: 0,
        consent: 0,
    });
    assert_eq!(out.intent.source, IntentSource::Own);
    let to_mne = out.intent.encode().unwrap();
    assert_eq!(&to_mne[0..4], b"GEMI");
    assert_eq!(to_mne.len(), INTENT_BYTES);
    let heard = IntentMsg::decode(&to_mne).unwrap();
    assert_eq!(heard, out.intent);
    assert_eq!(heard.encode().unwrap(), to_mne);

    let mut inbound = IntentMsg::steady(FRAME_MS);
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("ease").unwrap();
    inbound.strength = 0.4;
    inbound.set_pull(VALENCE, -0.2).unwrap();
    let to_life = inbound.encode().unwrap();
    assert_ne!(to_life, to_mne);
    assert_eq!(IntentMsg::decode(&to_life).unwrap(), inbound);

    let kept_cares = *life.kept_cares();
    let kept_expect = life.kept_expect();
    let before = life.expect().point[VALENCE];
    life.take_bytes(&to_life).unwrap();
    let back = life.last_inbound().unwrap();
    assert_eq!(back.source, IntentSource::Inbound);
    assert_eq!(back.verb_str(), "ease");
    assert_eq!(back.encode().unwrap(), to_life);
    assert_eq!(life.inbound_frames(), 1);
    assert!(life.expect().point[VALENCE] < before);
    assert_eq!(*life.kept_cares(), kept_cares);
    assert_eq!(life.kept_expect(), kept_expect);

    let mut other = InnerLife::at_rest();
    other.take_intent(&heard).unwrap();
    assert_eq!(other.last_inbound().unwrap().encode().unwrap(), to_mne);
    assert_eq!(other.last_inbound().unwrap().source, IntentSource::Own);
}

#[test]
fn no_learning_when_consent_is_denied() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("learn = \"deny\""));
    assert!(text.contains("[\"sense\", \"adapt\", \"remember\", \"share\"]"));
    assert!(!galion_emotion_core::learn_permitted(&text));

    let world = load_standins(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("standins")).unwrap();
    assert_eq!(world.inner.care, SEED_CARE);
    assert_eq!(world.inner.want, SEED_WANT);
    assert_eq!(world.inner.expect, SEED_EXPECT);
    assert_eq!(world.memory.len(), 2);

    let mut life = InnerLife::from_standin(world.baseline, &world.inner);
    assert!(!life.learn_allowed());
    assert_eq!(life.cares()[0].cue_str(), "heard");
    let kept_cares = *life.kept_cares();
    let kept_wants = *life.kept_wants();
    let kept_expect = life.kept_expect();
    let rest = life.expect().point;
    life.turn(TurnIn {
        line: "heard speaking",
        point: rest,
        t_ms: 0,
        consent: 0,
    });
    assert!(life.cares()[0].strength > SEED_STRENGTH);
    assert_eq!(life.wants()[0].met, 0.0);
    assert_ne!(life.expect().cue_str(), kept_expect.cue_str());
    assert!(!life.learn());
    assert_eq!(*life.kept_cares(), kept_cares);
    assert_eq!(*life.kept_wants(), kept_wants);
    assert_eq!(life.kept_expect(), kept_expect);
    assert_eq!(life.kept_cares()[0].strength, SEED_STRENGTH);
    assert_eq!(world.baseline.persona_id(), "standin-speaker");
}

#[test]
fn the_turn_does_not_allocate() {
    assert_eq!(TURN_ALLOC_BUDGET, 0);
    ARMED.with(|armed| armed.set(true));
    let mut probe = Vec::<u8>::new();
    probe.push(1);
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    drop(probe);

    let mut life = InnerLife::at_rest();
    let rest = life.expect().point;
    let mut inbound = IntentMsg::steady(0);
    inbound.source = IntentSource::Inbound;
    inbound.set_verb("ease").unwrap();
    inbound.strength = 0.25;
    inbound.set_pull(VALENCE, 0.25).unwrap();
    life.turn(TurnIn {
        line: "speaking",
        point: rest,
        t_ms: 0,
        consent: 0,
    });

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for i in 1..1000u32 {
        let out = life.turn(TurnIn {
            line: "speaking",
            point: rest,
            t_ms: i * FRAME_MS,
            consent: 0,
        });
        let bytes = out.intent.encode().unwrap();
        let decoded = IntentMsg::decode(&bytes).unwrap();
        life.take_intent(&inbound).unwrap();
        black_box((decoded, life.surprise(), life.inbound_frames()));
    }
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        TURN_ALLOC_BUDGET,
        "InnerLife::turn allocated"
    );
}
