// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! `AffectStore<S>` — consent enforced by the type system (A9).
//!
//! The store is the **only** place inbound affect may live after detection.
//! Its state parameter `S` is a zero-sized marker:
//!
//! * [`Unconsented`] — the default. Has no `record`, no `adapt_target`, no
//!   `share_frame`, no baseline. You *cannot* write caller affect into it:
//!   that is a compile error, not a runtime check (default-deny by type).
//! * [`Consented`] — reachable only via [`AffectStore::grant`], which requires a
//!   [`ConsentGate`] returning `Allow` **and** the in-band `SENSE` bit.
//!
//! Every way out of `Consented` (withdraw, revoking SENSE, gate re-check
//! failing, drop at hang-up) **wipes** the ring buffer and the calibration
//! baseline, zeroes the memory, and runs every registered [`WipeHook`] so
//! hosts (Galion OS, Hermes sideband, Galion One) can delete their copies.
//! GDPR Art. 7(3): withdrawal must be as easy as consent and take effect
//! immediately -> wipe is synchronous, inside the call that withdraws.
//!
//! Per-bit semantics (A4 consent bits, all default OFF):
//! * `SENSE`    — required to be `Consented` at all. Revoking it = full wipe.
//! * `ADAPT`    — [`AffectStore::adapt_target`] returns `Some` only with ADAPT;
//!                otherwise Galion speaks neutral-warm.
//! * `REMEMBER` — calibration baseline may be exported past the session.
//!                Revoking REMEMBER wipes the baseline (hook reason
//!                [`WipeReason::RememberRevoked`]), keeps the live ring.
//! * `SHARE`    — frames may leave the voice runtime. Requires an explicit
//!                **spoken** confirmation (see [`spoken`]), not just the bit.
//!
//! Default-deny is a compile error, not a runtime check:
//! ```compile_fail
//! use galion_emotion::{AffectStore, EmotionFrame};
//! let mut s = AffectStore::new(8); // AffectStore<Unconsented>
//! s.record(&EmotionFrame::neutral(0, 0, 80)); // no such method -> does not compile
//! ```
//! The consented path:
//! ```
//! use galion_emotion::{consent_bits::{SENSE, ADAPT}, AffectStore, DefaultConsentGate, EmotionFrame, Purpose};
//! let mut gate = DefaultConsentGate::default();
//! gate.grant(0, "hr", Purpose::PersonalAssistant);
//! let mut s = AffectStore::new(8).grant(&gate, Purpose::PersonalAssistant, SENSE | ADAPT).unwrap();
//! s.record(&EmotionFrame::neutral(0, 0, 80));
//! let (_s, receipt) = s.withdraw(1); // synchronous wipe + hooks
//! assert_eq!(receipt.frames_wiped, 1);
//! ```
//!
//! Zero allocation in the frame loop: the ring is pre-allocated at
//! construction; `record` only overwrites slots.

use core::marker::PhantomData;

use crate::band::consent_bits::{ADAPT, REMEMBER, SENSE, SHARE};
use crate::consent::{ConsentDecision, ConsentGate, Purpose};
use crate::frame::EmotionFrame;

/// Marker: no consent (default). Write methods do not exist in this state.
#[derive(Debug, Clone, Copy)]
pub enum Unconsented {}
/// Marker: consent proven by a gate `Allow` + `SENSE` bit.
#[derive(Debug, Clone, Copy)]
pub enum Consented {}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Unconsented {}
    impl Sealed for super::Consented {}
}
/// Sealed: hosts cannot invent a third state that bypasses the gate.
pub trait ConsentState: sealed::Sealed {}
impl ConsentState for Unconsented {}
impl ConsentState for Consented {}

/// Why a wipe happened. Hooks receive this; nothing else (never frames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeReason {
    /// User withdrew consent (spoken "stop" / UI / API). GDPR Art. 7(3).
    Withdrawn,
    /// SENSE bit cleared in-band.
    SenseRevoked,
    /// REMEMBER cleared: baseline wiped, live session ring kept.
    RememberRevoked,
    /// Gate re-check no longer returns `Allow` (purpose change, prohibited context...).
    GateDenied(ConsentDecision),
    /// Store dropped / call ended without REMEMBER.
    SessionEnd,
}

/// Host hook run synchronously on every wipe. Must delete any host-side copy
/// (on-disk baseline, Hermes sideband cache, Galion One mirror...).
pub trait WipeHook {
    fn on_wipe(&mut self, reason: WipeReason);
}

impl<F: FnMut(WipeReason)> WipeHook for F {
    fn on_wipe(&mut self, reason: WipeReason) {
        self(reason)
    }
}

/// Proof of a wipe, for the Art. 30-style processing record. Counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WipeReceipt {
    pub reason: WipeReason,
    pub at_ms: u64,
    pub frames_wiped: usize,
    pub baseline_wiped: bool,
    pub hooks_run: usize,
}

/// Running VAD baseline ("calibration"). Only exists inside `Consented`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AffectBaseline {
    pub n: u32,
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
}

impl AffectBaseline {
    fn update(&mut self, f: &EmotionFrame) {
        self.n = self.n.saturating_add(1);
        let k = 1.0 / self.n as f32;
        self.valence += k * (f.valence - self.valence);
        self.arousal += k * (f.arousal - self.arousal);
        self.dominance += k * (f.dominance - self.dominance);
    }
}

struct Inner {
    ring: Vec<EmotionFrame>,
    head: usize,
    len: usize,
    baseline: AffectBaseline,
    bits: u8,
    purpose: Purpose,
    share_confirmed: bool,
    hooks: Vec<Box<dyn WipeHook>>,
}

impl Inner {
    fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            ring: vec![EmotionFrame::neutral(0, 0, 0); cap],
            head: 0,
            len: 0,
            baseline: AffectBaseline::default(),
            bits: 0,
            purpose: Purpose::PersonalAssistant,
            share_confirmed: false,
            hooks: Vec::new(),
        }
    }

    fn zero_ring(&mut self) -> usize {
        let n = self.len;
        for s in self.ring.iter_mut() {
            *s = EmotionFrame::neutral(0, 0, 0);
        }
        self.head = 0;
        self.len = 0;
        n
    }

    fn wipe_all(&mut self, reason: WipeReason, at_ms: u64) -> WipeReceipt {
        let frames_wiped = self.zero_ring();
        let baseline_wiped = self.baseline.n > 0;
        self.baseline = AffectBaseline::default();
        self.bits = 0;
        self.share_confirmed = false;
        let hooks_run = self.run_hooks(reason);
        WipeReceipt { reason, at_ms, frames_wiped, baseline_wiped, hooks_run }
    }

    fn run_hooks(&mut self, reason: WipeReason) -> usize {
        for h in self.hooks.iter_mut() {
            h.on_wipe(reason);
        }
        self.hooks.len()
    }
}

/// Typestate affect store. See module docs.
pub struct AffectStore<S: ConsentState> {
    inner: Inner,
    _s: PhantomData<S>,
}

impl<S: ConsentState> core::fmt::Debug for AffectStore<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print frames.
        f.debug_struct("AffectStore")
            .field("state", &core::any::type_name::<S>())
            .field("len", &self.inner.len)
            .field("bits", &self.inner.bits)
            .field("hooks", &self.inner.hooks.len())
            .finish()
    }
}

/// Result of re-validating a consented store against the gate.
#[derive(Debug)]
pub enum Revalidated {
    StillConsented(AffectStore<Consented>),
    Wiped(AffectStore<Unconsented>, WipeReceipt),
}

impl<S: ConsentState> AffectStore<S> {
    fn retag<T: ConsentState>(self) -> AffectStore<T> {
        // `AffectStore` has no Drop of its own (the wipe-on-drop lives on
        // `Inner`), so moving `inner` across states is free and safe.
        AffectStore { inner: self.inner, _s: PhantomData }
    }

    /// Register a host wipe hook. Allowed in any state (hooks carry no affect).
    /// Allocates; call at setup, not in the frame loop.
    pub fn on_wipe(&mut self, hook: Box<dyn WipeHook>) {
        self.inner.hooks.push(hook);
    }

    /// Frames currently held. Always 0 while `Unconsented`.
    pub fn len(&self) -> usize {
        self.inner.len
    }
    pub fn is_empty(&self) -> bool {
        self.inner.len == 0
    }
    /// Current in-band consent bits (0 while `Unconsented`).
    pub fn bits(&self) -> u8 {
        self.inner.bits
    }
    pub fn capacity(&self) -> usize {
        self.inner.ring.len()
    }
}

impl AffectStore<Unconsented> {
    /// Default-deny constructor. `capacity` frames are pre-allocated.
    pub fn new(capacity: usize) -> Self {
        AffectStore { inner: Inner::new(capacity), _s: PhantomData }
    }

    /// Try to enter `Consented`. Needs gate `Allow` for `purpose` **and** SENSE.
    /// On refusal the unconsented store is handed back with the reason.
    pub fn grant(
        self,
        gate: &dyn ConsentGate,
        purpose: Purpose,
        bits: u8,
    ) -> Result<AffectStore<Consented>, (AffectStore<Unconsented>, ConsentDecision)> {
        let d = gate.check(purpose);
        gate.audit(purpose, d);
        if !d.allowed() {
            return Err((self, d));
        }
        if bits & SENSE == 0 {
            return Err((self, ConsentDecision::DenyNoConsent));
        }
        let mut s: AffectStore<Consented> = self.retag();
        s.inner.bits = bits & 0x0f;
        s.inner.purpose = purpose;
        // SHARE bit alone is not enough: needs a spoken confirmation.
        s.inner.share_confirmed = false;
        Ok(s)
    }
}

impl Default for AffectStore<Unconsented> {
    fn default() -> Self {
        Self::new(64)
    }
}

impl AffectStore<Consented> {
    /// Record one consented inbound frame. Zero allocation.
    pub fn record(&mut self, f: &EmotionFrame) {
        let f = f.clamped();
        let cap = self.inner.ring.len();
        self.inner.ring[self.inner.head] = f;
        self.inner.head = (self.inner.head + 1) % cap;
        self.inner.len = (self.inner.len + 1).min(cap);
        if f.confidence >= 0.2 {
            self.inner.baseline.update(&f);
        }
    }

    /// Most recent frame, if any.
    pub fn latest(&self) -> Option<EmotionFrame> {
        if self.inner.len == 0 {
            return None;
        }
        let cap = self.inner.ring.len();
        Some(self.inner.ring[(self.inner.head + cap - 1) % cap])
    }

    /// Frame to steer Galion's prosody — `None` unless ADAPT (-> neutral-warm).
    pub fn adapt_target(&self) -> Option<EmotionFrame> {
        if self.inner.bits & ADAPT == 0 {
            return None;
        }
        self.latest()
    }

    /// Calibration baseline (session-local). Always available while consented.
    pub fn baseline(&self) -> AffectBaseline {
        self.inner.baseline
    }

    /// Baseline for persistence past the session — only with REMEMBER.
    pub fn baseline_for_persist(&self) -> Option<AffectBaseline> {
        if self.inner.bits & REMEMBER == 0 || self.inner.baseline.n == 0 {
            return None;
        }
        Some(self.inner.baseline)
    }

    /// Record that the caller *spoke* the SHARE confirmation (see [`spoken`]).
    /// Has effect only if the SHARE bit is also set.
    pub fn confirm_share_spoken(&mut self, proof: spoken::ShareConfirmed) {
        let _ = proof;
        self.inner.share_confirmed = self.inner.bits & SHARE != 0;
    }

    /// Frame that may leave the voice runtime: SHARE bit **and** spoken confirmation.
    pub fn share_frame(&self) -> Option<EmotionFrame> {
        if self.inner.bits & SHARE == 0 || !self.inner.share_confirmed {
            return None;
        }
        self.latest()
    }

    /// Withdraw consent: synchronous wipe + hooks, back to `Unconsented`.
    pub fn withdraw(mut self, at_ms: u64) -> (AffectStore<Unconsented>, WipeReceipt) {
        let r = self.inner.wipe_all(WipeReason::Withdrawn, at_ms);
        (self.retag(), r)
    }

    /// Update in-band bits. Clearing SENSE wipes everything and leaves
    /// `Consented`; clearing REMEMBER wipes the baseline only; clearing SHARE
    /// drops the spoken confirmation. Bits can only be *added* via `grant`
    /// on a fresh store (escalation needs a fresh consent ceremony).
    pub fn set_bits(mut self, bits: u8, at_ms: u64) -> Revalidated {
        let bits = bits & 0x0f;
        if bits & SENSE == 0 {
            let r = self.inner.wipe_all(WipeReason::SenseRevoked, at_ms);
            return Revalidated::Wiped(self.retag(), r);
        }
        let old = self.inner.bits;
        let new = old & bits; // never escalate here
        if old & REMEMBER != 0 && new & REMEMBER == 0 {
            self.inner.baseline = AffectBaseline::default();
            self.inner.run_hooks(WipeReason::RememberRevoked);
        }
        if new & SHARE == 0 {
            self.inner.share_confirmed = false;
        }
        self.inner.bits = new;
        Revalidated::StillConsented(self)
    }

    /// Re-check the gate (e.g. once per heartbeat or on any consent event).
    /// Anything but `Allow` wipes.
    pub fn revalidate(mut self, gate: &dyn ConsentGate, at_ms: u64) -> Revalidated {
        let p = self.inner.purpose;
        let d = gate.check(p);
        gate.audit(p, d);
        if d.allowed() {
            Revalidated::StillConsented(self)
        } else {
            let r = self.inner.wipe_all(WipeReason::GateDenied(d), at_ms);
            Revalidated::Wiped(self.retag(), r)
        }
    }
}

impl Drop for Inner {
    /// Hang-up / drop: wipe the live ring and the baseline. Persisting the
    /// baseline is the host's job via `baseline_for_persist()` *before* drop,
    /// and only with REMEMBER.
    fn drop(&mut self) {
        if self.len > 0 || self.baseline.n > 0 {
            let _ = self.wipe_all(WipeReason::SessionEnd, 0);
        }
    }
}

/// Spoken consent ceremony (AI Act Art. 50(3) notice + GDPR Art. 7).
///
/// Stub: keyword matching on an already-transcribed utterance. The real
/// version consumes an intent from the cascade/ASR lane; the state machine
/// and its outputs stay the same.
pub mod spoken {
    use crate::band::consent_bits::{ADAPT, SENSE};
    use crate::consent::notice_text;

    /// Unforgeable outside this module: proof the caller *said* yes to SHARE.
    #[derive(Debug)]
    pub struct ShareConfirmed(());

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Heard {
        Yes,
        No,
        /// "stop", "prestani", "povuci pristanak"... -> withdraw immediately.
        Withdraw,
        Unclear,
    }

    pub fn classify(utterance: &str) -> Heard {
        let u = utterance.to_lowercase();
        let has = |ws: &[&str]| ws.iter().any(|w| u.split(|c: char| !c.is_alphanumeric()).any(|t| t == *w));
        if has(&["stop", "withdraw", "prestani", "povuci", "povlačim", "zaustavi"]) {
            Heard::Withdraw
        } else if has(&["no", "ne", "nemoj", "nope"]) {
            Heard::No
        } else if has(&["yes", "da", "okay", "ok", "može", "slažem"]) {
            Heard::Yes
        } else {
            Heard::Unclear
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Step {
        /// Notice must be spoken first; nothing granted.
        NoticePending,
        AwaitingAnswer,
        /// Caller said yes: SENSE|ADAPT may be requested via `AffectStore::grant`.
        Granted { bits: u8 },
        Declined,
    }

    /// Consent ceremony. Unclear answers never count as yes.
    #[derive(Debug, Clone)]
    pub struct SpokenConsent {
        pub lang: &'static str,
        pub step: Step,
    }

    impl SpokenConsent {
        pub fn new(lang: &'static str) -> Self {
            Self { lang, step: Step::NoticePending }
        }
        /// Text to speak (Art. 50(3) notice). Advances to `AwaitingAnswer`.
        pub fn speak_notice(&mut self) -> &'static str {
            self.step = Step::AwaitingAnswer;
            notice_text(self.lang)
        }
        pub fn hear(&mut self, utterance: &str) -> Step {
            if self.step == Step::AwaitingAnswer {
                self.step = match classify(utterance) {
                    Heard::Yes => Step::Granted { bits: SENSE | ADAPT },
                    Heard::No | Heard::Withdraw => Step::Declined,
                    Heard::Unclear => Step::AwaitingAnswer,
                };
            }
            self.step
        }
    }

    /// Separate spoken question for SHARE (A9/F: spoken SHARE confirmation).
    pub fn share_question(lang: &str) -> &'static str {
        match lang {
            "hr" => "Smijem li podijeliti procjenu Vašeg tona s Galion OS-om ili Hermesom? Recite 'da' ili 'ne'.",
            _ => "May I share the estimate of your tone with Galion OS or Hermes? Say 'yes' or 'no'.",
        }
    }

    /// Only an explicit "yes" to the share question yields the proof.
    pub fn confirm_share(answer: &str) -> Option<ShareConfirmed> {
        (classify(answer) == Heard::Yes).then_some(ShareConfirmed(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::DefaultConsentGate;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn fr(v: f32, a: f32) -> EmotionFrame {
        EmotionFrame { seq: 1, t_ms: 0, dur_ms: 80, valence: v, arousal: a, dominance: 0.5, confidence: 0.9 }
    }

    fn granted_gate() -> DefaultConsentGate {
        let mut g = DefaultConsentGate::default();
        g.grant(1, "hr", Purpose::PersonalAssistant);
        g
    }

    fn counter_hook(log: &Rc<RefCell<Vec<WipeReason>>>) -> Box<dyn WipeHook> {
        let l = Rc::clone(log);
        Box::new(move |r: WipeReason| l.borrow_mut().push(r))
    }

    #[test]
    fn default_is_unconsented_and_empty() {
        let s = AffectStore::default();
        assert_eq!(s.len(), 0);
        assert_eq!(s.bits(), 0);
        // `s.record(..)` does not compile here: that is the point.
    }

    #[test]
    fn grant_requires_gate_allow() {
        let g = DefaultConsentGate::default();
        let (s, d) = AffectStore::new(4).grant(&g, Purpose::PersonalAssistant, SENSE | ADAPT).unwrap_err();
        assert_eq!(d, ConsentDecision::DenyNoConsent);
        assert!(s.is_empty());
    }

    #[test]
    fn grant_requires_sense_bit() {
        let g = granted_gate();
        let r = AffectStore::new(4).grant(&g, Purpose::PersonalAssistant, ADAPT | REMEMBER);
        assert_eq!(r.unwrap_err().1, ConsentDecision::DenyNoConsent);
    }

    #[test]
    fn prohibited_purpose_never_consented() {
        let mut g = DefaultConsentGate::default();
        g.grant(1, "en", Purpose::Workplace);
        let r = AffectStore::new(4).grant(&g, Purpose::Workplace, 0x0f);
        assert_eq!(r.unwrap_err().1, ConsentDecision::DenyProhibitedPurpose);
    }

    #[test]
    fn record_ring_is_bounded_and_adapt_gated() {
        let g = granted_gate();
        let mut s = AffectStore::new(3).grant(&g, Purpose::PersonalAssistant, SENSE).unwrap();
        for i in 0..5 {
            s.record(&fr(0.1 * i as f32, 0.5));
        }
        assert_eq!(s.len(), 3);
        assert!((s.latest().unwrap().valence - 0.4).abs() < 1e-6);
        assert!(s.adapt_target().is_none(), "no ADAPT -> neutral-warm");
        let s2 = AffectStore::new(3).grant(&g, Purpose::PersonalAssistant, SENSE | ADAPT).unwrap();
        let mut s2 = s2;
        s2.record(&fr(-0.5, 0.9));
        assert!(s2.adapt_target().is_some());
    }

    #[test]
    fn withdraw_wipes_and_runs_hooks() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let g = granted_gate();
        let mut s0 = AffectStore::new(8);
        s0.on_wipe(counter_hook(&log));
        let mut s = s0.grant(&g, Purpose::PersonalAssistant, SENSE | ADAPT | REMEMBER).unwrap();
        s.record(&fr(0.5, 0.5));
        s.record(&fr(0.6, 0.5));
        assert!(s.baseline_for_persist().is_some());
        let (u, r) = s.withdraw(42);
        assert_eq!(r.frames_wiped, 2);
        assert!(r.baseline_wiped);
        assert_eq!(r.hooks_run, 1);
        assert_eq!(r.at_ms, 42);
        assert_eq!(u.len(), 0);
        assert_eq!(u.bits(), 0);
        assert_eq!(*log.borrow(), vec![WipeReason::Withdrawn]);
        drop(u); // empty store: drop must not fire hooks again
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn wiped_memory_is_zeroed() {
        let g = granted_gate();
        let mut s = AffectStore::new(2).grant(&g, Purpose::PersonalAssistant, SENSE).unwrap();
        s.record(&fr(0.9, 0.9));
        let (u, _) = s.withdraw(1);
        assert!(u.inner.ring.iter().all(|f| f.confidence == 0.0 && f.valence == 0.0));
        assert_eq!(u.inner.baseline, AffectBaseline::default());
    }

    #[test]
    fn revoking_sense_wipes_revoking_remember_wipes_baseline_only() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let g = granted_gate();
        let mut s0 = AffectStore::new(8);
        s0.on_wipe(counter_hook(&log));
        let mut s = s0.grant(&g, Purpose::PersonalAssistant, SENSE | ADAPT | REMEMBER).unwrap();
        s.record(&fr(0.5, 0.5));
        let mut s = match s.set_bits(SENSE | ADAPT, 5) {
            Revalidated::StillConsented(s) => s,
            _ => panic!("SENSE still set"),
        };
        assert_eq!(s.len(), 1, "live ring kept");
        assert!(s.baseline_for_persist().is_none());
        assert_eq!(s.baseline().n, 0);
        // no escalation via set_bits
        s = match s.set_bits(0x0f, 6) {
            Revalidated::StillConsented(s) => s,
            _ => panic!(),
        };
        assert_eq!(s.bits() & REMEMBER, 0);
        match s.set_bits(ADAPT, 7) {
            Revalidated::Wiped(u, r) => {
                assert_eq!(r.reason, WipeReason::SenseRevoked);
                assert!(u.is_empty());
            }
            _ => panic!("SENSE cleared must wipe"),
        }
        assert_eq!(*log.borrow(), vec![WipeReason::RememberRevoked, WipeReason::SenseRevoked]);
    }

    #[test]
    fn revalidate_wipes_when_gate_withdrawn() {
        let mut g = granted_gate();
        let mut s = AffectStore::new(4).grant(&g, Purpose::PersonalAssistant, SENSE).unwrap();
        s.record(&fr(0.1, 0.1));
        let s = match s.revalidate(&g, 1) {
            Revalidated::StillConsented(s) => s,
            _ => panic!(),
        };
        g.withdraw(2);
        match s.revalidate(&g, 2) {
            Revalidated::Wiped(u, r) => {
                assert_eq!(r.reason, WipeReason::GateDenied(ConsentDecision::DenyWithdrawn));
                assert!(u.is_empty());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn drop_at_hangup_runs_hooks() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let g = granted_gate();
        let mut s0 = AffectStore::new(4);
        s0.on_wipe(counter_hook(&log));
        let mut s = s0.grant(&g, Purpose::PersonalAssistant, SENSE).unwrap();
        s.record(&fr(0.0, 0.5));
        drop(s);
        assert_eq!(*log.borrow(), vec![WipeReason::SessionEnd]);
    }

    #[test]
    fn share_needs_bit_and_spoken_confirmation() {
        let g = granted_gate();
        let mut s = AffectStore::new(4).grant(&g, Purpose::PersonalAssistant, SENSE | SHARE).unwrap();
        s.record(&fr(0.2, 0.2));
        assert!(s.share_frame().is_none(), "bit alone is not enough");
        assert!(spoken::confirm_share("hmm maybe").is_none());
        s.confirm_share_spoken(spoken::confirm_share("da").unwrap());
        assert!(s.share_frame().is_some());
        let mut s2 = AffectStore::new(4).grant(&g, Purpose::PersonalAssistant, SENSE).unwrap();
        s2.record(&fr(0.2, 0.2));
        s2.confirm_share_spoken(spoken::confirm_share("yes").unwrap());
        assert!(s2.share_frame().is_none(), "no SHARE bit -> spoken yes ignored");
    }

    #[test]
    fn spoken_ceremony() {
        use spoken::*;
        let mut c = SpokenConsent::new("hr");
        assert_eq!(c.hear("da"), Step::NoticePending, "no yes before the notice is spoken");
        assert!(c.speak_notice().contains("Pristanak"));
        assert_eq!(c.hear("ummm"), Step::AwaitingAnswer, "unclear is never yes");
        assert_eq!(c.hear("Da, može."), Step::Granted { bits: SENSE | ADAPT });
        let mut c2 = SpokenConsent::new("en");
        c2.speak_notice();
        assert_eq!(c2.hear("no thanks"), Step::Declined);
        assert_eq!(classify("Galion, prestani!"), Heard::Withdraw);
        assert_eq!(classify("stop"), Heard::Withdraw);
    }
}
