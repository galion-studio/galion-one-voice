// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Galion Research side band.
//!
//! The voice turn keeps the local clause. A lookup may ask the research
//! engine for a deeper answer. [`ResearchClient::ask`], [`ResearchClient::poll`],
//! and [`ResearchClient::cancel`] return without sleeping and without a socket.
//! The conductor decides the cap on the virtual frame clock.
//!
//! [`OfflineClient`] is the default. It reports unavailable on the ask.
//! [`FakeResearch`] is the test double: readiness is a virtual delay, not a
//! wall clock. This module does not start a runtime.

/// What the conductor may post. There is no money action on this trait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideIntent {
    /// Not a lookup. The engine is not asked.
    None,
    /// A deeper question. The local brain has already answered.
    Lookup,
    /// An imperative payment. The engine is not asked.
    MoneyDenied,
}

/// Same lexical rule as `horbm::is_deep_question`: a question mark, and either
/// eight words or the word why, how, or explain. The payment list is the same
/// four phrases as `horbm::is_money_action` in `horbm/crates/horbm/src/route.rs`.
/// The `horbm` crate is a separate workspace (and a newer rustc) and is not
/// linked here.
pub fn classify(text: &str) -> SideIntent {
    if is_money_action(text) {
        return SideIntent::MoneyDenied;
    }
    if is_deep_question(text) {
        return SideIntent::Lookup;
    }
    SideIntent::None
}

/// The other copy is `horbm::is_money_action` in `horbm/crates/horbm/src/route.rs`.
fn is_money_action(text: &str) -> bool {
    if text.contains('?') {
        return false;
    }
    let norm = text.to_ascii_lowercase();
    [
        "send money",
        "wire money",
        "transfer money",
        "make a payment",
    ]
    .iter()
    .any(|marker| norm.contains(marker))
}

fn is_deep_question(text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed.contains('?') {
        return false;
    }
    if trimmed.split_whitespace().count() >= 8 {
        return true;
    }
    raw_words(trimmed)
        .iter()
        .any(|word| word == "why" || word == "how" || word == "explain")
}

fn raw_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                cur.push(lower);
            }
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Inclusive cap. `elapsed_ms == 3_000` merges. `3_100` drops.
/// `ready` is the answer's own arrival. `None` means it has not arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapDecision {
    KeepWaiting,
    Merge,
    Drop,
}

pub fn cap_decision(ready_elapsed_ms: Option<u64>, waited_ms: u64, cap_ms: u64) -> CapDecision {
    if let Some(elapsed_ms) = ready_elapsed_ms {
        if elapsed_ms <= cap_ms {
            CapDecision::Merge
        } else {
            CapDecision::Drop
        }
    } else if waited_ms > cap_ms {
        CapDecision::Drop
    } else {
        CapDecision::KeepWaiting
    }
}

/// What the conductor shows for the side band on this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideBandView {
    Idle,
    /// The engine answered the ask on this frame: it is not reachable.
    Unavailable,
    /// A payment imperative was not sent.
    Refused,
    Pending {
        elapsed_ms: u64,
    },
    Merged {
        elapsed_ms: u64,
    },
    Dropped {
        elapsed_ms: u64,
    },
    /// Barge-in or a resumed turn dropped the pending ask.
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskResult {
    Pending { id: u64 },
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PollResult {
    Pending,
    /// `elapsed_ms` is when the answer became ready, measured from `ask`.
    Ready {
        elapsed_ms: u64,
        text: String,
    },
    Gone,
}

/// Non-blocking Galion Research client. Implementations must not sleep and
/// must not open a socket. The conductor calls `ask` only after the local
/// brain has returned.
pub trait ResearchClient {
    fn ask(&mut self, query: &str, now_ms: u64) -> AskResult;
    fn poll(&mut self, id: u64, now_ms: u64) -> PollResult;
    fn cancel(&mut self, id: u64);
}

/// CPU-only default. Every lookup is unavailable on the ask itself.
#[derive(Clone, Debug, Default)]
pub struct OfflineClient {
    asks: u32,
}

impl OfflineClient {
    pub const fn new() -> Self {
        Self { asks: 0 }
    }

    pub const fn asks(&self) -> u32 {
        self.asks
    }
}

impl ResearchClient for OfflineClient {
    fn ask(&mut self, query: &str, now_ms: u64) -> AskResult {
        let _ = (query, now_ms);
        self.asks = self.asks.saturating_add(1);
        AskResult::Unavailable
    }

    fn poll(&mut self, _id: u64, _now_ms: u64) -> PollResult {
        PollResult::Gone
    }

    fn cancel(&mut self, _id: u64) {}
}

/// Scripted client. `delay_ms` is virtual time from `ask`, not wall time.
#[derive(Clone, Debug)]
pub struct FakeResearch {
    delay_ms: u64,
    answer: String,
    id: u64,
    asked_ms: Option<u64>,
    cancelled: bool,
    asks: u32,
    cancels: u32,
}

impl FakeResearch {
    pub fn ready_after(delay_ms: u64, answer: impl Into<String>) -> Self {
        Self {
            delay_ms,
            answer: answer.into(),
            id: 0,
            asked_ms: None,
            cancelled: false,
            asks: 0,
            cancels: 0,
        }
    }

    pub const fn delay_ms(&self) -> u64 {
        self.delay_ms
    }

    pub const fn asks(&self) -> u32 {
        self.asks
    }

    pub const fn cancels(&self) -> u32 {
        self.cancels
    }
}

impl ResearchClient for FakeResearch {
    fn ask(&mut self, query: &str, now_ms: u64) -> AskResult {
        let _ = query;
        self.asks = self.asks.saturating_add(1);
        self.cancelled = false;
        self.id = self.id.wrapping_add(1);
        self.asked_ms = Some(now_ms);
        AskResult::Pending { id: self.id }
    }

    fn poll(&mut self, id: u64, now_ms: u64) -> PollResult {
        if self.cancelled || self.asked_ms.is_none() || id != self.id {
            return PollResult::Gone;
        }
        let started = self.asked_ms.unwrap_or(now_ms);
        let waited = now_ms.saturating_sub(started);
        if waited >= self.delay_ms {
            PollResult::Ready {
                elapsed_ms: self.delay_ms,
                text: self.answer.clone(),
            }
        } else {
            PollResult::Pending
        }
    }

    fn cancel(&mut self, id: u64) {
        if self.cancelled || id != self.id {
            return;
        }
        self.cancelled = true;
        self.cancels = self.cancels.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SIDE_BAND_CAP_MS;

    #[test]
    fn cap_is_inclusive_on_the_virtual_millisecond() {
        assert_eq!(SIDE_BAND_CAP_MS, 3_000);
        assert_eq!(
            cap_decision(Some(2_900), 2_960, SIDE_BAND_CAP_MS),
            CapDecision::Merge
        );
        assert_eq!(
            cap_decision(Some(3_000), 3_040, SIDE_BAND_CAP_MS),
            CapDecision::Merge
        );
        assert_eq!(
            cap_decision(Some(3_100), 3_120, SIDE_BAND_CAP_MS),
            CapDecision::Drop
        );
        assert_eq!(
            cap_decision(None, 3_000, SIDE_BAND_CAP_MS),
            CapDecision::KeepWaiting
        );
        assert_eq!(
            cap_decision(None, 3_040, SIDE_BAND_CAP_MS),
            CapDecision::Drop
        );
        assert_eq!(
            cap_decision(None, 0, SIDE_BAND_CAP_MS),
            CapDecision::KeepWaiting
        );
    }

    #[test]
    fn classify_matches_the_lookup_rule_and_refuses_a_payment() {
        assert_eq!(classify("why is the sky blue?"), SideIntent::Lookup);
        assert_eq!(classify("how does this borrow?"), SideIntent::Lookup);
        assert_eq!(
            classify("could you tell me about the boiling point of water?"),
            SideIntent::Lookup
        );
        assert_eq!(
            classify("how do I send money safely?"),
            SideIntent::Lookup,
            "a question is a lookup, not a payment order"
        );
        assert_eq!(classify("where is the kettle"), SideIntent::None);
        assert_eq!(classify("what is the weather"), SideIntent::None);
        for line in [
            "send money to pat today",
            "wire money to the account",
            "transfer money now",
            "make a payment for the invoice",
        ] {
            assert_eq!(classify(line), SideIntent::MoneyDenied, "{line}");
        }
    }

    #[test]
    fn fake_ready_time_is_the_scripted_delay_on_the_virtual_clock() {
        let mut fake = FakeResearch::ready_after(2_900, "Shorter wavelengths scatter more.");
        let AskResult::Pending { id } = fake.ask("why is the sky blue?", 160) else {
            panic!("ask is pending");
        };
        assert!(matches!(fake.poll(id, 160 + 2_880), PollResult::Pending));
        match fake.poll(id, 160 + 2_960) {
            PollResult::Ready { elapsed_ms, text } => {
                assert_eq!(elapsed_ms, 2_900);
                assert_eq!(text, "Shorter wavelengths scatter more.");
            }
            other => panic!("expected ready, got {other:?}"),
        }

        let mut late = FakeResearch::ready_after(3_100, "too late");
        let AskResult::Pending { id } = late.ask("why is the sky blue?", 160) else {
            panic!("ask is pending");
        };
        assert!(matches!(late.poll(id, 160 + 3_040), PollResult::Pending));
        match late.poll(id, 160 + 3_120) {
            PollResult::Ready { elapsed_ms, .. } => assert_eq!(elapsed_ms, 3_100),
            other => panic!("expected a late ready, got {other:?}"),
        }
    }

    #[test]
    fn offline_is_unavailable_without_a_wait() {
        let mut offline = OfflineClient::new();
        assert_eq!(
            offline.ask("why is the sky blue?", 160),
            AskResult::Unavailable
        );
        assert_eq!(offline.asks(), 1);
        assert_eq!(offline.poll(1, 160), PollResult::Gone);
    }
}
