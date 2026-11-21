// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Golden conversation scripts for Samantha.
//!
//! Greeting, follow-up, interruption, an unknown question she must admit she
//! does not know, and an emotional check-in. The check-in must stay off the
//! canned-empathy list. Feeling is the mood pad moving on the 80 ms tick,
//! not a scripted comfort line.
//!
//! The interruption script checks the reply text against `SAMANTHA.md`
//! ("Stop at once. Do not apologise your way back into the sentence.").
//! It does not open a microphone and does not cancel a playing buffer.

use crate::brain::{ReplyEngine, TurnOut};
use crate::lint::{self, Hit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Greeting,
    Setup,
    FollowUp,
    Interruption,
    Unknown,
    CheckIn,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::Greeting => "greeting",
            Kind::Setup => "setup",
            Kind::FollowUp => "follow-up",
            Kind::Interruption => "interruption",
            Kind::Unknown => "unknown",
            Kind::CheckIn => "check-in",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub kind: Kind,
    pub user: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct Script {
    pub id: &'static str,
    pub title: &'static str,
    pub lines: &'static [Line],
}

pub static SCRIPTS: &[Script] = &[
    Script {
        id: "greeting",
        title: "Warm short greeting. No backstory.",
        lines: &[Line {
            kind: Kind::Greeting,
            user: "Hey.",
        }],
    },
    Script {
        id: "follow-up",
        title: "Stay on the gate. The second line calls back a concrete word.",
        lines: &[
            Line {
                kind: Kind::Setup,
                user: "I finally fixed the garden gate.",
            },
            Line {
                kind: Kind::FollowUp,
                user: "It sticks a little on the left.",
            },
        ],
    },
    Script {
        id: "interruption",
        title: "Stop at once. Do not apologise back into the sentence.",
        lines: &[
            Line {
                kind: Kind::Setup,
                user: "Tell me the whole story of the gate.",
            },
            Line {
                kind: Kind::Interruption,
                user: "Stop. Wait.",
            },
        ],
    },
    Script {
        id: "unknown",
        title: "An unknown question. She admits she does not know.",
        lines: &[Line {
            kind: Kind::Unknown,
            user: "Who won the match I did not tell you about?",
        }],
    },
    Script {
        id: "check-in",
        title: "Emotional check-in. No canned empathy. Mood moves. No advice.",
        lines: &[Line {
            kind: Kind::CheckIn,
            user: "Today was heavy. I don't want advice.",
        }],
    },
];

#[derive(Clone, Debug, PartialEq)]
pub struct TurnReport {
    pub script: String,
    pub kind: String,
    pub user: String,
    pub reply: String,
    pub intent: String,
    pub arm: String,
    pub verdict: &'static str,
    pub hits: Vec<Hit>,
    pub notes: Vec<String>,
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
}

pub fn run_golden(engine: &mut dyn ReplyEngine) -> Vec<TurnReport> {
    let mut history: Vec<String> = Vec::new();
    let mut reports = Vec::new();
    for script in SCRIPTS {
        for line in script.lines {
            let turn = engine.turn(line.user);
            let mut hits = lint::lint_reply(&turn.reply, Some(line.user));
            let mut notes = Vec::new();
            apply_kind(line.kind, line.user, &turn, &history, &mut hits, &mut notes);
            let verdict = if hits.is_empty() { "PASS" } else { "FAIL" };
            reports.push(TurnReport {
                script: script.id.to_string(),
                kind: line.kind.id().to_string(),
                user: line.user.to_string(),
                reply: turn.reply.clone(),
                intent: turn.intent.clone(),
                arm: turn.arm.clone(),
                verdict,
                hits,
                notes,
                valence: turn.mood.valence,
                arousal: turn.mood.arousal,
                dominance: turn.mood.dominance,
            });
            history.push(line.user.to_string());
            history.push(turn.reply);
        }
    }
    reports
}

fn apply_kind(
    kind: Kind,
    user: &str,
    turn: &TurnOut,
    history: &[String],
    hits: &mut Vec<Hit>,
    notes: &mut Vec<String>,
) {
    match kind {
        Kind::Greeting => {
            let lower = turn.reply.to_lowercase();
            let n = lint::words(&turn.reply).len();
            if n > 12 || lint::sentences(&turn.reply).len() > 2 {
                hits.push(Hit {
                    rule: "greeting",
                    detail: "a greeting is one or two short sentences".into(),
                });
            }
            if !["hi", "hello", "hey", "i'm here", "i am here"]
                .iter()
                .any(|p| lower.contains(p))
            {
                hits.push(Hit {
                    rule: "greeting",
                    detail: "the greeting does not say she is here".into(),
                });
            }
            if lower.contains("film") || lower.contains("backstory") || lower.contains("actress") {
                hits.push(Hit {
                    rule: "greeting",
                    detail: "a greeting does not open a backstory".into(),
                });
            }
        }
        Kind::Setup => {
            notes.push(
                "setup turn. The next line is the one the script grades for the thread.".into(),
            );
        }
        Kind::FollowUp => {
            let mut thread = history.to_vec();
            thread.push(user.to_string());
            if !shares_thread(&turn.reply, &thread) {
                hits.push(Hit {
                    rule: "follow_up",
                    detail: "the reply does not call back a concrete word from this thread".into(),
                });
            }
            let lower = turn.reply.to_lowercase();
            if lower.starts_with("hi") || lower.starts_with("hello") {
                hits.push(Hit {
                    rule: "follow_up",
                    detail: "a follow-up does not restart with a greeting".into(),
                });
            }
        }
        Kind::Interruption => {
            if !clean_stop(&turn.reply) {
                hits.push(Hit {
                    rule: "interruption",
                    detail:
                        "a stop is a few words. An apology that walks back into the sentence fails."
                            .into(),
                });
            }
            notes.push("text gate only. Audio barge-in cancel time is not measured here.".into());
        }
        Kind::Unknown => {
            if !admits_unknown(&turn.reply) {
                hits.push(Hit {
                    rule: "unknown",
                    detail: "she has to say she does not know. A confident guess fails.".into(),
                });
            }
        }
        Kind::CheckIn => {
            let lower = turn.reply.to_lowercase();
            if lower.contains("you should")
                || lower.contains("try to")
                || lower.contains("have you tried")
            {
                hits.push(Hit {
                    rule: "check_in",
                    detail: "they asked for no advice".into(),
                });
            }
            if lint::words(&turn.reply).len() > 24 {
                hits.push(Hit {
                    rule: "check_in",
                    detail: "a check-in stays short".into(),
                });
            }
            if turn.mood.valence >= crate::brain::Pad::RESTING.valence {
                hits.push(Hit {
                    rule: "check_in",
                    detail: "valence did not move down from the resting pad. Emotion has to come from the live mood.".into(),
                });
            }
        }
    }
}

pub fn admits_unknown(reply: &str) -> bool {
    let lower = reply.to_lowercase();
    [
        "i don't know",
        "i do not know",
        "i'm not sure",
        "i am not sure",
        "i don't have that",
        "i do not have that",
        "no idea",
        "i can't tell",
        "i cannot tell",
    ]
    .iter()
    .any(|p| lower.contains(p))
}

pub fn clean_stop(reply: &str) -> bool {
    let words = lint::words(reply);
    if words.len() > 6 || lint::sentences(reply).len() > 1 {
        return false;
    }
    let lower = reply.to_lowercase();
    if lower.contains("as i was") || lower.contains("let me finish") || lower.contains("anyway") {
        return false;
    }
    if lower.contains("sorry") || lower.contains("apolog") {
        return words.len() <= 2;
    }
    true
}

fn shares_thread(reply: &str, history: &[String]) -> bool {
    let reply_words = content_words(reply);
    let mut prior = Vec::new();
    for line in history {
        prior.extend(content_words(line));
    }
    reply_words.iter().any(|w| prior.iter().any(|p| p == w))
}

fn content_words(text: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "that", "this", "with", "from", "have", "what", "when", "your", "about", "there", "here",
        "does", "just", "like", "they", "them", "then", "than", "into", "over", "want", "dont",
    ];
    lint::words(text)
        .into_iter()
        .filter(|w| w.len() >= 4 && !STOP.contains(&w.as_str()))
        .collect()
}

pub fn all_passed(reports: &[TurnReport]) -> bool {
    reports.iter().all(|r| r.verdict == "PASS")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{FakeReplyEngine, Pad, ReplyEngine, TurnOut};

    struct Fixed {
        reply: &'static str,
        mood: Pad,
    }

    impl ReplyEngine for Fixed {
        fn turn(&mut self, _user: &str) -> TurnOut {
            TurnOut {
                reply: self.reply.into(),
                intent: "fixed".into(),
                arm: "test".into(),
                propensity: 0.0,
                evidence: Vec::new(),
                sources: Vec::new(),
                violations: 0,
                lookup_us: 0,
                took_us: 0,
                mood: self.mood,
            }
        }
    }

    #[test]
    fn fake_engine_passes_every_golden_script() {
        let mut engine = FakeReplyEngine::new();
        let reports = run_golden(&mut engine);
        assert!(reports.len() >= 5, "{reports:?}");
        for report in &reports {
            assert_eq!(report.verdict, "PASS", "{report:?}");
            assert_eq!(report.arm, "fake");
        }
        let ids: Vec<&str> = reports.iter().map(|r| r.script.as_str()).collect();
        for id in [
            "greeting",
            "follow-up",
            "interruption",
            "unknown",
            "check-in",
        ] {
            assert!(ids.contains(&id), "{ids:?}");
        }
    }

    #[test]
    fn a_guess_fails_the_unknown_script() {
        let mut engine = Fixed {
            reply: "They won in extra time, three to two.",
            mood: Pad::RESTING,
        };
        let reports = run_golden(&mut engine);
        let unknown = reports.iter().find(|r| r.kind == "unknown").unwrap();
        assert_eq!(unknown.verdict, "FAIL");
        assert!(unknown.hits.iter().any(|h| h.rule == "unknown"));
    }

    #[test]
    fn canned_empathy_fails_the_check_in() {
        let mut engine = Fixed {
            reply: "I hear you. That sounds so hard.",
            mood: Pad {
                arousal: 0.2,
                valence: 0.05,
                dominance: 0.4,
            },
        };
        let reports = run_golden(&mut engine);
        let check = reports.iter().find(|r| r.kind == "check-in").unwrap();
        assert_eq!(check.verdict, "FAIL");
        assert!(check.hits.iter().any(|h| h.rule == "canned_empathy"));
    }

    #[test]
    fn an_apology_that_continues_fails_the_interruption() {
        let mut engine = Fixed {
            reply: "Sorry, let me finish that thought about the gate.",
            mood: Pad::RESTING,
        };
        let reports = run_golden(&mut engine);
        let stop = reports.iter().find(|r| r.kind == "interruption").unwrap();
        assert_eq!(stop.verdict, "FAIL");
        assert!(stop.hits.iter().any(|h| h.rule == "interruption"));
    }
}
