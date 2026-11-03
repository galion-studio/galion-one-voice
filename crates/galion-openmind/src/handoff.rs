// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Cortex handoff: analytical takeover on the Hermes side band.
//!
//! The voice engine keeps speaking from local state (canned reply, filler,
//! backchannel). [`CortexHandoff::offer`] only `try_send`s
//! [`ToHermes::Intent`], [`ToHermes::Turn`], and [`ToHermes::Affect`] onto the
//! side-band thread that already exists in `galion-sideband`. It does not
//! open a socket of its own, and it must not be called from `Heartbeat::step`.
//!
//! Retrieval still dies at [`RETRIEVAL_CAP`] (3 s). [`CortexHandoff::poll`] is
//! the same non-blocking poll as the side-band client: past the cap the
//! answer is `NoKnowledge { reason: Timeout }`, and the audio path uses its
//! local filler instead of waiting.
//!
//! Money actions are not offered. [`Intent::voice_may_request`] is the rule
//! this hook enforces.

use galion_sideband::client::SidebandClient;

pub use galion_sideband::client::{HermesTransport, NullTransport, SubmitError};
pub use galion_sideband::{
    ActionTier, Affect, FromHermes, Intent, NoKnowledgeReason, Speaker, ToHermes, Turn, TurnId,
    RETRIEVAL_CAP,
};

/// One turn-boundary ask. Text only. No PCM, no sample buffers.
#[derive(Debug, Clone, PartialEq)]
pub struct TakeoverOffer {
    pub turn: Turn,
    pub intent: Intent,
    pub affect: Affect,
}

/// What happened to the intent half of an offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentDisposition {
    /// Queued on the side band. The 3 s clock started.
    Sent,
    /// `Intent::Action { tier: Money, .. }` is dropped. Turn and affect may
    /// still have been queued so Hermes has context, not a payment order.
    RefusedMoney,
    /// The side-band queue did not take it. The voice path continues.
    Dropped(SubmitError),
}

/// Which of the three events made it into the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandoffReceipt {
    pub turn: TurnId,
    pub intent: IntentDisposition,
    pub turn_sent: bool,
    pub affect_sent: bool,
}

/// Owns one [`SidebandClient`]. Drop joins the side thread; do that at
/// session end, not inside a frame.
pub struct CortexHandoff {
    client: SidebandClient,
}

impl CortexHandoff {
    /// `queue` bounds outstanding events. A full queue drops; it never waits.
    pub fn spawn<T: HermesTransport>(transport: T, queue: usize) -> Self {
        Self {
            client: SidebandClient::spawn(transport, queue),
        }
    }

    /// No Hermes process. Lookups come back `NoKnowledge::Unreachable`.
    pub fn local() -> Self {
        Self::spawn(NullTransport, 8)
    }

    /// Turn boundary only. Intent is offered first so the retrieval clock
    /// starts when the ask is accepted. Never blocks.
    pub fn offer(&mut self, offer: TakeoverOffer) -> HandoffReceipt {
        let turn_id = offer.turn.id;
        let intent = if offer.intent.voice_may_request() {
            match self.client.submit(ToHermes::Intent {
                turn: turn_id,
                intent: offer.intent,
            }) {
                Ok(()) => IntentDisposition::Sent,
                Err(e) => IntentDisposition::Dropped(e),
            }
        } else {
            IntentDisposition::RefusedMoney
        };
        let turn_sent = self.client.submit(ToHermes::Turn(offer.turn)).is_ok();
        let affect_sent = self
            .client
            .submit(ToHermes::Affect {
                turn: turn_id,
                affect: offer.affect.clamped(),
            })
            .is_ok();
        HandoffReceipt {
            turn: turn_id,
            intent,
            turn_sent,
            affect_sent,
        }
    }

    /// Non-blocking. `None` means keep talking from local state.
    /// Past [`RETRIEVAL_CAP`] this is `NoKnowledge::Timeout`.
    pub fn poll(&mut self, turn: TurnId) -> Option<FromHermes> {
        self.client.poll(turn)
    }
}

/// The cap the audio path is not allowed to wait out inside a frame.
pub fn retrieval_cap_ms() -> u64 {
    RETRIEVAL_CAP.as_millis() as u64
}
