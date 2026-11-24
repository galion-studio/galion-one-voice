// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Empathy, then the layer 5 route, then the mood the voice speaks with.

use std::path::Path;
use std::time::Instant;

use galion_emotion_core::{ConductorCue, EmotionCore, InnerLife, IntentMsg, MomentState, TurnIn};
use galion_sam_empathy::{Empathy, PronounClass, TickIn, FRAME_MS};

use crate::route::{accept_line, route_heard, CardLookup, Gap, ResearchAsk, Speakable};
use crate::{Pad, TurnOut};

/// Mood and the prosody knobs that belong on the turn. Not a sentence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoodMeta {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
    pub e: f32,
    pub intent: &'static str,
    pub rate: f32,
    pub energy: f32,
    pub pitch_st: f32,
    pub pause_ms: f32,
    pub named: Option<&'static str>,
}

impl MoodMeta {
    pub const RESTING: MoodMeta = MoodMeta {
        arousal: Pad::RESTING.arousal,
        valence: Pad::RESTING.valence,
        dominance: Pad::RESTING.dominance,
        e: 0.0,
        intent: "chat",
        rate: 1.0,
        energy: 1.0,
        pitch_st: 0.0,
        pause_ms: 0.0,
        named: None,
    };

    pub fn pad(self) -> Pad {
        Pad {
            arousal: self.arousal,
            valence: self.valence,
            dominance: self.dominance,
        }
    }
}

struct EmptyCards;

impl CardLookup for EmptyCards {
    fn lookup(&self, _query: &str) -> Option<crate::CardHit> {
        None
    }
}

/// The linked brain. `open` reads the knowledge directory when it already
/// exists and does not create it. Memory and case paths are not opened.
/// The live server passes the same read-only loader.
pub struct Brain {
    empathy: Empathy,
    cards: Box<dyn CardLookup + Send>,
    research: Option<Box<dyn ResearchAsk + Send>>,
    speakable: Speakable,
    gaps: Vec<Gap>,
    meta: MoodMeta,
    emotion: EmotionCore,
    inner: InnerLife,
}

impl Brain {
    pub fn with_cards(
        cards: impl CardLookup + Send + 'static,
        research: Option<Box<dyn ResearchAsk + Send>>,
    ) -> Self {
        Self {
            empathy: Empathy::new(),
            cards: Box::new(cards),
            research,
            speakable: accept_line,
            gaps: Vec::new(),
            meta: resting_meta(),
            emotion: EmotionCore::at_rest(),
            inner: InnerLife::at_rest(),
        }
    }

    /// Read `knowledge_dir` when it is already a directory. A missing path is
    /// an empty card set. Nothing is created. Memory and case paths are not
    /// opened.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let _ = (memory_db, cases);
        match crate::KnowledgeCards::load(knowledge_dir) {
            Ok(loaded) => {
                eprintln!("{}", loaded.report.log_line());
                Self::with_cards(loaded.cards, None)
            }
            Err(crate::CardError::NotADirectory(_)) => Self::with_cards(EmptyCards, None),
            Err(err) => {
                eprintln!("horbm cards: {err}");
                Self::with_cards(EmptyCards, None)
            }
        }
    }

    pub fn set_speakable(&mut self, speakable: Speakable) {
        self.speakable = speakable;
    }

    pub fn note_name(&mut self, name: &str, class: PronounClass) {
        self.empathy.note_name(name, class);
    }

    pub fn gaps(&self) -> &[Gap] {
        &self.gaps
    }

    pub fn mood_meta(&self) -> MoodMeta {
        self.meta
    }

    /// The speaker's moment. Empathy mood is [`Self::mood_meta`].
    pub fn emotion(&self) -> &EmotionCore {
        &self.emotion
    }

    /// One 80 ms emotion frame. Does not change the empathy mood.
    pub fn step_moment(
        &mut self,
        intent: &IntentMsg,
    ) -> Result<MomentState, galion_emotion_core::CoreError> {
        self.emotion.step(intent)
    }

    /// Cares, wants, and expectations. The moment core does not own them.
    pub fn inner(&self) -> &InnerLife {
        &self.inner
    }

    /// One dialogue turn. Updates the inner life from `line` and the current
    /// emotion point, then steps the moment with the GEMI intent that turn
    /// emitted. The baseline is the core's. This does not rewrite the mood.
    pub fn inner_turn(&mut self, line: &str) -> Result<IntentMsg, galion_emotion_core::CoreError> {
        let mut next = self.inner;
        let out = next.turn(TurnIn {
            line,
            point: self.emotion.moment().pos,
            t_ms: self.emotion.next_ms(),
            consent: self.emotion.consent(),
        });
        self.emotion.step(&out.intent)?;
        self.inner = next;
        Ok(out.intent)
    }

    /// GEMI message from the MnE side. Does not step the moment.
    pub fn take_mne_intent(
        &mut self,
        msg: &IntentMsg,
    ) -> Result<(), galion_emotion_core::CoreError> {
        self.inner.take_intent(msg)
    }

    /// GEMI bytes from the MnE side. Does not step the moment.
    pub fn take_mne_bytes(&mut self, bytes: &[u8]) -> Result<(), galion_emotion_core::CoreError> {
        self.inner.take_bytes(bytes)
    }

    /// Copy live cares, wants, and the expectation into the kept weights.
    /// False while `LOCKS.toml` `[consent] learn` is not `allow`.
    pub fn learn_inner(&mut self) -> bool {
        self.inner.learn()
    }

    /// Snapshot for the conductor. The conductor does not own this state.
    pub fn conductor_cue(&self) -> ConductorCue {
        self.emotion.conductor_cue()
    }

    /// One 80 ms frame with no new words. Does not speak. Does not step the moment.
    pub fn tick_frame(&mut self) -> MoodMeta {
        let tick = self.empathy.tick(TickIn {
            inbound: None,
            asr: "",
            asr_final: false,
            invite_name: false,
            frame_ms: FRAME_MS,
        });
        self.meta = meta_of(&tick);
        self.meta
    }

    /// Empathy mood, rewritten lookup, cards, then the gateway, then
    /// [`crate::UNKNOWN_REPLY`].
    pub fn turn(&mut self, text: &str) -> TurnOut {
        let started = Instant::now();
        let heard = text.trim();
        let tick = self.empathy.tick(TickIn {
            inbound: None,
            asr: heard,
            asr_final: !heard.is_empty(),
            invite_name: false,
            frame_ms: FRAME_MS,
        });
        self.meta = meta_of(&tick);
        let prep = self.empathy.prepare_lookup(heard);
        let query = if prep.query.trim().is_empty() {
            heard
        } else {
            prep.query.as_str()
        };
        let routed = match self.research.as_mut() {
            Some(port) => route_heard(
                query,
                self.cards.as_ref(),
                Some(port.as_mut()),
                self.speakable,
            ),
            None => route_heard(query, self.cards.as_ref(), None, self.speakable),
        };
        if let Some(gap) = routed.gap.clone() {
            self.gaps.push(gap);
        }
        TurnOut {
            reply: routed.spoken,
            intent: self.meta.intent.to_string(),
            arm: "none".to_string(),
            propensity: 0.0,
            evidence: routed.evidence,
            sources: routed.sources,
            violations: 0,
            lookup_us: routed.lookup_us,
            took_us: elapsed_us(started.elapsed()),
            mood: self.meta.pad(),
            mood_meta: self.meta,
        }
    }
}

fn resting_meta() -> MoodMeta {
    let tick = Empathy::new().tick(TickIn {
        inbound: None,
        asr: "",
        asr_final: false,
        invite_name: false,
        frame_ms: FRAME_MS,
    });
    meta_of(&tick)
}

fn meta_of(tick: &galion_sam_empathy::TickOut) -> MoodMeta {
    MoodMeta {
        arousal: tick.mood.arousal,
        valence: tick.mood.valence,
        dominance: tick.mood.dominance,
        e: tick.e,
        intent: tick.intent.as_str(),
        rate: tick.prosody.speaking.rate,
        energy: tick.prosody.speaking.energy,
        pitch_st: tick.prosody.speaking.pitch_shift_st,
        pause_ms: tick.prosody.speaking.pre_pause_ms,
        named: tick.named.map(|name| name.word()),
    }
}

fn elapsed_us(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AskOutcome, CardHit, CardLookup, GapReason, ResearchAsk, UNKNOWN_REPLY};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    struct Cards {
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl CardLookup for Cards {
        fn lookup(&self, query: &str) -> Option<CardHit> {
            self.seen.lock().expect("seen").push(query.to_string());
            if query.to_ascii_lowercase().contains("kettle") {
                Some(CardHit {
                    id: "kettle".into(),
                    gist: "The kettle is on the hob.".into(),
                })
            } else {
                None
            }
        }
    }

    fn cards() -> Cards {
        Cards {
            seen: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    struct Rec {
        calls: Arc<AtomicU32>,
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl ResearchAsk for Rec {
        fn ask(&mut self, question: &str, cap: Duration) -> AskOutcome {
            assert_eq!(cap, crate::RESEARCH_CAP);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.seen.lock().expect("seen").push(question.to_string());
            AskOutcome::Timeout
        }
    }

    #[test]
    fn open_does_not_create_a_directory() {
        let missing = std::env::temp_dir().join(format!(
            "horbm-open-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        assert!(!missing.exists());
        let mut brain = Brain::open(&missing, &missing.join("knowledge"), &missing.join("cases"));
        let out = brain.turn("where is the kettle");
        assert_eq!(out.reply, UNKNOWN_REPLY);
        assert!(!missing.exists());
    }

    #[test]
    fn tired_miss_keeps_the_exact_line_and_passes_the_mood() {
        let mut brain = Brain::with_cards(cards(), None);
        let out = brain.turn("I am so tired");
        assert_eq!(out.reply, "I don't know.");
        assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
        assert!(out.mood_meta.e > 0.0);
        assert!(out.mood.valence > 0.0);
        assert_eq!(out.mood.arousal, out.mood_meta.arousal);
        assert!(out.mood_meta.named.is_none());
        assert!(!out.reply.to_ascii_lowercase().contains("tired"));
        assert!(!out.reply.to_ascii_lowercase().contains("sorry"));
        assert_eq!(brain.gaps()[0].reason, GapReason::NoCard);
    }

    #[test]
    fn a_card_hit_keeps_the_mood_and_does_not_ask() {
        let calls = Arc::new(AtomicU32::new(0));
        let mut brain = Brain::with_cards(
            cards(),
            Some(Box::new(Rec {
                calls: Arc::clone(&calls),
                seen: Arc::new(std::sync::Mutex::new(Vec::new())),
            })),
        );
        let out = brain.turn("I am so tired where is the kettle");
        assert_eq!(out.reply, "The kettle is on the hob.");
        assert_eq!(out.sources, vec!["card:kettle".to_string()]);
        assert!(out.mood_meta.e > 0.0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(brain.gaps().is_empty());
    }

    #[test]
    fn a_name_is_rewritten_before_lookup() {
        let looked = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut brain = Brain::with_cards(
            Cards {
                seen: Arc::clone(&looked),
            },
            None,
        );
        brain.note_name("Maciej", PronounClass::He);
        let out = brain.turn("why did he move the kettle?");
        assert_eq!(out.reply, "The kettle is on the hob.");
        let queries = looked.lock().unwrap().clone();
        assert!(queries.iter().any(|q| q.contains("Maciej")), "{queries:?}");
        assert!(queries.iter().all(|q| !q.contains(" he ")));
    }
}
