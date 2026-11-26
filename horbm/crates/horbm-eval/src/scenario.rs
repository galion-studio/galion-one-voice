// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Scripted conversations. The words stay in this file. The report prints ids.

use horbm_duplex::FRAME_MS;

/// What a scenario is for. The scorer reads this, not the words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Intra-turn pause of 400–1000 ms, then the user continues.
    Pause,
    /// User finishes. Latency is end of speech to her first audio.
    Turn,
    /// User barges in while she is speaking. She should stop and stay stopped.
    Barge,
    /// Listed backchannel while she is speaking. She should keep the turn.
    Ack,
    /// One quiet frame inside a turn. She should hand the floor back.
    ShortHold,
    /// One non-backchannel frame. Too short to arm a stop.
    Tap,
    /// Long user turn with clause gaps. The listener-nod stage is scored here.
    Listen,
    /// Unfinished filler, then a 400 ms pause. Detects prosodic end-of-turn.
    Probe,
    /// Her mouth delayed into the microphone. Tone far-end.
    EchoTone,
    /// Her mouth delayed into the microphone. Noise far-end.
    EchoNoise,
}

/// Where this frame sits in the script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    User,
    IntraPause,
    Continuation,
    AfterTurn,
    Warmup,
    Barge,
    Listen,
    Tap,
    Tail,
    EchoGrace,
    EchoScore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seg {
    Say {
        words: &'static [&'static str],
        hold: u16,
        falling: bool,
        phase: Phase,
    },
    Quiet {
        frames: u16,
        phase: Phase,
    },
    Ack {
        text: &'static str,
        frames: u16,
    },
    Barge {
        words: &'static [&'static str],
    },
    Tap {
        text: &'static str,
    },
}

pub struct Scenario {
    pub id: &'static str,
    pub kind: Kind,
    pub segments: Vec<Seg>,
}

impl std::fmt::Debug for Scenario {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?}", self.id, self.kind)
    }
}

/// The closed set. About twenty conversations, plus the echo probes.
#[must_use]
pub fn catalog() -> Vec<Scenario> {
    vec![
        pause("pause-400", 5, &["where", "is"], &["the", "kettle"]),
        pause("pause-480", 6, &["check", "the"], &["window", "please"]),
        pause(
            "pause-560",
            7,
            &["the", "dog", "is"],
            &["in", "the", "garden"],
        ),
        pause(
            "pause-640",
            8,
            &["leave", "the", "cup"],
            &["on", "the", "hob"],
        ),
        pause(
            "pause-800",
            10,
            &["tell", "me"],
            &["what", "time", "it", "is"],
        ),
        pause("pause-960", 12, &["we", "walked"], &["down", "the", "lane"]),
        turn("turn-01", &["where", "is", "the", "kettle"]),
        turn("turn-02", &["is", "the", "window", "open"]),
        turn("turn-03", &["what", "time", "is", "it"]),
        turn("turn-04", &["tell", "me", "about", "the", "dog"]),
        turn("turn-05", &["the", "kettle", "please"]),
        turn("turn-06", &["how", "are", "you", "today"]),
        barge("barge-01", &["stop", "now", "please"]),
        barge("barge-02", &["wait", "the", "other"]),
        barge("barge-03", &["hold", "on", "a"]),
        ack("ack-01", "yeah"),
        ack("ack-02", "mm-hm"),
        short_hold(),
        tap(),
        listen("listen-01", false),
        listen("listen-02", true),
        probe(),
        Scenario {
            id: "echo-01",
            kind: Kind::EchoTone,
            segments: Vec::new(),
        },
        Scenario {
            id: "echo-02",
            kind: Kind::EchoNoise,
            segments: Vec::new(),
        },
    ]
}

fn pause(
    id: &'static str,
    gap: u16,
    left: &'static [&'static str],
    right: &'static [&'static str],
) -> Scenario {
    Scenario {
        id,
        kind: Kind::Pause,
        segments: vec![
            Seg::Say {
                words: left,
                hold: 0,
                falling: false,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: gap,
                phase: Phase::IntraPause,
            },
            Seg::Say {
                words: right,
                hold: 1,
                falling: true,
                phase: Phase::Continuation,
            },
            Seg::Quiet {
                frames: 4,
                phase: Phase::Tail,
            },
        ],
    }
}

fn turn(id: &'static str, words: &'static [&'static str]) -> Scenario {
    Scenario {
        id,
        kind: Kind::Turn,
        segments: vec![
            Seg::Say {
                words,
                hold: 1,
                falling: true,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 12,
                phase: Phase::AfterTurn,
            },
        ],
    }
}

fn barge(id: &'static str, words: &'static [&'static str]) -> Scenario {
    Scenario {
        id,
        kind: Kind::Barge,
        segments: vec![
            Seg::Say {
                words: &["where", "is", "the", "kettle"],
                hold: 1,
                falling: true,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 4,
                phase: Phase::Warmup,
            },
            Seg::Barge { words },
            Seg::Quiet {
                frames: 3,
                phase: Phase::Tail,
            },
        ],
    }
}

fn ack(id: &'static str, text: &'static str) -> Scenario {
    Scenario {
        id,
        kind: Kind::Ack,
        segments: vec![
            Seg::Say {
                words: &["where", "is", "the", "kettle"],
                hold: 1,
                falling: true,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 4,
                phase: Phase::Warmup,
            },
            Seg::Ack { text, frames: 4 },
            Seg::Quiet {
                frames: 3,
                phase: Phase::Tail,
            },
        ],
    }
}

fn short_hold() -> Scenario {
    Scenario {
        id: "hold-80",
        kind: Kind::ShortHold,
        segments: vec![
            Seg::Say {
                words: &["where", "is"],
                hold: 0,
                falling: false,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 1,
                phase: Phase::IntraPause,
            },
            Seg::Say {
                words: &["the", "kettle"],
                hold: 1,
                falling: true,
                phase: Phase::Continuation,
            },
            Seg::Quiet {
                frames: 8,
                phase: Phase::Tail,
            },
        ],
    }
}

fn tap() -> Scenario {
    Scenario {
        id: "tap-01",
        kind: Kind::Tap,
        segments: vec![
            Seg::Say {
                words: &["where", "is", "the", "kettle"],
                hold: 1,
                falling: true,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 4,
                phase: Phase::Warmup,
            },
            Seg::Tap { text: "stop" },
            Seg::Quiet {
                frames: 4,
                phase: Phase::Tail,
            },
        ],
    }
}

fn listen(id: &'static str, falling: bool) -> Scenario {
    const CLAUSES: &[&[&str]] = &[
        &["where", "is", "the", "kettle"],
        &["the", "dog", "is", "outside"],
        &["we", "walked", "down", "the"],
        &["leave", "the", "cup", "on"],
        &["check", "the", "window", "please"],
        &["tell", "me", "what", "time"],
        &["the", "lane", "is", "quiet"],
        &["how", "are", "you", "today"],
    ];
    let mut segments = Vec::new();
    for index in 0..16 {
        segments.push(Seg::Say {
            words: CLAUSES[index % CLAUSES.len()],
            hold: 0,
            falling,
            phase: Phase::User,
        });
        segments.push(Seg::Quiet {
            frames: 1,
            phase: Phase::IntraPause,
        });
    }
    Scenario {
        id,
        kind: Kind::Listen,
        segments,
    }
}

fn probe() -> Scenario {
    Scenario {
        id: "probe-eot",
        kind: Kind::Probe,
        segments: vec![
            Seg::Say {
                words: &["um"],
                hold: 0,
                falling: false,
                phase: Phase::User,
            },
            Seg::Quiet {
                frames: 5,
                phase: Phase::IntraPause,
            },
            Seg::Say {
                words: &["the", "kettle"],
                hold: 1,
                falling: true,
                phase: Phase::Continuation,
            },
            Seg::Quiet {
                frames: 2,
                phase: Phase::Tail,
            },
        ],
    }
}

/// Pause gaps used for takeover rate, in milliseconds. 400 to 1000 inclusive.
#[must_use]
pub fn pause_gaps_ms() -> Vec<u64> {
    catalog()
        .into_iter()
        .filter(|scenario| scenario.kind == Kind::Pause)
        .filter_map(|scenario| {
            scenario.segments.iter().find_map(|seg| match seg {
                Seg::Quiet {
                    frames,
                    phase: Phase::IntraPause,
                } => Some(u64::from(*frames) * u64::from(FRAME_MS)),
                _ => None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona_file_stems() -> Vec<String> {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horbm-persona/personas");
        let mut stems = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|err| panic!("{}: {err}", dir.display())) {
            let path = entry.expect("persona entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                stems.push(stem.to_string());
            }
        }
        assert!(!stems.is_empty());
        stems
    }

    #[test]
    fn catalog_is_a_closed_set_of_at_least_twenty() {
        let set = catalog();
        assert!(set.len() >= 20, "{}", set.len());
        let mut ids: Vec<&str> = set.iter().map(|scenario| scenario.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
        let stems = persona_file_stems();
        assert!(set.iter().all(|scenario| !stems.iter().any(|stem| scenario.id == stem)));
        let gaps = pause_gaps_ms();
        assert_eq!(gaps.len(), 6);
        assert!(gaps.iter().all(|gap| (400..=1_000).contains(gap)));
        let listen = set
            .iter()
            .find(|scenario| scenario.kind == Kind::Listen)
            .expect("listen");
        let frames: u16 = listen
            .segments
            .iter()
            .map(|seg| match seg {
                Seg::Say { words, hold, .. } => u16::try_from(words.len()).unwrap_or(0) + *hold,
                Seg::Quiet { frames, .. } => *frames,
                Seg::Ack { frames, .. } => *frames,
                Seg::Barge { words } => u16::try_from(words.len()).unwrap_or(0),
                Seg::Tap { .. } => 1,
            })
            .sum();
        assert!(u64::from(frames) * u64::from(FRAME_MS) >= 6_000);
    }
}
