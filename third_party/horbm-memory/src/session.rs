// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::collections::VecDeque;

/// One turn in the short-term ring.
///
/// The ring is the current call. Pushing past the cap drops the oldest
/// turn from RAM only. `push_turn` also enqueues an append-only transcript
/// row, so the words survive after the ring forgets them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    pub id: String,
    pub text: String,
    pub said_by: String,
    pub date: String,
}

/// Fixed-capacity ring. Pushing past `cap` drops the oldest turn.
#[derive(Clone, Debug)]
pub struct SessionRing {
    cap: usize,
    seq: u64,
    turns: VecDeque<Turn>,
}

impl SessionRing {
    pub fn new(cap: usize) -> Self {
        Self {
            cap: cap.max(1),
            seq: 0,
            turns: VecDeque::new(),
        }
    }

    pub fn push(
        &mut self,
        text: impl Into<String>,
        said_by: impl Into<String>,
        date: impl Into<String>,
    ) -> Turn {
        self.seq += 1;
        let turn = Turn {
            id: format!("session-{}", self.seq),
            text: text.into(),
            said_by: said_by.into(),
            date: date.into(),
        };
        if self.turns.len() == self.cap {
            self.turns.pop_front();
        }
        self.turns.push_back(turn.clone());
        turn
    }

    /// Drop the turns held in this process. They were never stored.
    pub fn clear(&mut self) {
        self.turns.clear();
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    pub fn turns(&self) -> impl Iterator<Item = &Turn> {
        self.turns.iter()
    }
}

/// One message in a prompt window. `role` is `system`, `user`, `assistant`,
/// or `tool`. HORBM does not call a model; a host can trim a loaded
/// transcript with [`fit_context`] before it builds a prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
}

/// Rough token count, `chars / 4 + 4`.
///
/// `hermes-rs` `estimate_tokens` (`agent.rs`) measures the JSON encoding of
/// a message. This measures the role plus the content, which is the same
/// order of magnitude and does not need a tokenizer.
pub fn estimate_tokens(role: &str, content: &str) -> usize {
    role.len().saturating_add(content.len()) / 4 + 4
}

/// Trim history from the front so the system message and the newest turns
/// fit `context_length - reserve` (minimum budget 256).
///
/// The kept window never starts on an orphaned `role = tool` result. Ported
/// from `hermes-rs` `fit_context` (`agent.rs`). The store keeps every turn;
/// this only chooses what a later prompt would send.
pub fn fit_context(
    system: &ContextMessage,
    msgs: &[ContextMessage],
    context_length: usize,
    reserve: usize,
) -> Vec<ContextMessage> {
    let budget = context_length.saturating_sub(reserve).max(256);
    let mut used = estimate_tokens(&system.role, &system.content);
    let mut keep_from = msgs.len();
    for (i, message) in msgs.iter().enumerate().rev() {
        let tokens = estimate_tokens(&message.role, &message.content);
        if used + tokens > budget && keep_from < msgs.len() {
            break;
        }
        used += tokens;
        keep_from = i;
    }
    let mut out = msgs[keep_from..].to_vec();
    while out.len() > 1 && out[0].role == "tool" {
        out.remove(0);
    }
    let mut window = vec![system.clone()];
    window.append(&mut out);
    window
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_the_oldest() {
        let mut ring = SessionRing::new(2);
        ring.push("one", "a", "2026-11-18");
        ring.push("two", "a", "2026-11-18");
        ring.push("three", "a", "2026-11-18");
        let texts: Vec<_> = ring.turns().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["two", "three"]);
    }

    #[test]
    fn fit_context_drops_the_oldest_and_a_leading_tool_result() {
        let system = ContextMessage {
            role: "system".into(),
            content: "s".into(),
        };
        let big = "x".repeat(4000);
        let msgs = vec![
            ContextMessage {
                role: "user".into(),
                content: big.clone(),
            },
            ContextMessage {
                role: "assistant".into(),
                content: String::new(),
            },
            ContextMessage {
                role: "tool".into(),
                content: big,
            },
            ContextMessage {
                role: "user".into(),
                content: "now".into(),
            },
        ];
        let out = fit_context(&system, &msgs, 1300, 0);
        assert_eq!(out[0].role, "system");
        assert_eq!(out.last().unwrap().content, "now");
        assert_ne!(out[1].role, "tool");
        assert!(out.len() < 5);

        let orphan = vec![
            ContextMessage {
                role: "tool".into(),
                content: "orphan tool output".into(),
            },
            ContextMessage {
                role: "user".into(),
                content: "now".into(),
            },
        ];
        let trimmed = fit_context(&system, &orphan, 256, 0);
        assert_eq!(trimmed.len(), 2);
        assert_eq!(trimmed[1].content, "now");
        assert!(trimmed.iter().all(|message| message.role != "tool"));
    }
}
