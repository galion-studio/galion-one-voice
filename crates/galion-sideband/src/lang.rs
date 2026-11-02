//! Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Maciej Grajczyk.
//!
//! W21-03 · multilingual plan P0.1 (types + tests only): `language_hint` and
//! `lang_pin` side-band fields. No routing logic lives here; the engine's
//! router applies precedence **pin > hint > LID** (galion-cascade `route`).
//! The types carry any BCP-47-ish tag so the contract is language-agnostic;
//! which tags are *enabled* is decided by the engine build (EN-only today).

/// A normalised language tag: trimmed, primary subtag lower-case, region
/// upper-case (`"EN-gb"` -> `"en-GB"`). Validation is syntactic only.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LangTag(String);

impl LangTag {
    /// Parse `xx` or `xx-YY` / `xxx-YY` (2–3 letter primary, optional 2-letter
    /// or 3-digit region). Returns `None` for anything else.
    pub fn parse(tag: &str) -> Option<Self> {
        let t = tag.trim().replace('_', "-");
        let mut parts = t.split('-');
        let primary = parts.next()?;
        if !(2..=3).contains(&primary.len()) || !primary.chars().all(|c| c.is_ascii_alphabetic()) { return None; }
        let mut out = primary.to_ascii_lowercase();
        if let Some(region) = parts.next() {
            let ok = (region.len() == 2 && region.chars().all(|c| c.is_ascii_alphabetic()))
                || (region.len() == 3 && region.chars().all(|c| c.is_ascii_digit()));
            if !ok { return None; }
            out.push('-');
            out.push_str(&region.to_ascii_uppercase());
        }
        if parts.next().is_some() { return None; }
        Some(LangTag(out))
    }
    pub fn as_str(&self) -> &str { &self.0 }
    /// Primary language subtag (`"en"` for `"en-GB"`).
    pub fn primary(&self) -> &str { self.0.split('-').next().unwrap_or(&self.0) }
}

/// Advisory hint from the client/Hermes: "the user probably speaks X".
/// Unknown or disabled hints are ignored by the engine (fall through to LID).
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageHint { pub tag: LangTag, pub confidence: f32 }

impl LanguageHint {
    pub fn new(tag: LangTag, confidence: f32) -> Self { Self { tag, confidence: confidence.clamp(0.0, 1.0) } }
}

/// Explicit pin: "speak and listen in X until unpinned". A pin to a disabled
/// language is an error at the engine, never a silent switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LangPin { Pinned(LangTag), Auto }

impl Default for LangPin { fn default() -> Self { LangPin::Auto } }

/// Per-session language preferences carried on the side-band.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LangPrefs { pub pin: LangPin, pub hint: Option<LanguageHint> }

/// Which field wins, before the engine checks what is enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangPrecedence { Pin, Hint, Lid }

impl LangPrefs {
    /// Precedence only (pin > hint > LID); enablement is the engine's job.
    pub fn precedence(&self) -> LangPrecedence {
        match (&self.pin, &self.hint) {
            (LangPin::Pinned(_), _) => LangPrecedence::Pin,
            (LangPin::Auto, Some(_)) => LangPrecedence::Hint,
            (LangPin::Auto, None) => LangPrecedence::Lid,
        }
    }
    /// Tag to try first, if any.
    pub fn requested(&self) -> Option<&LangTag> {
        match (&self.pin, &self.hint) {
            (LangPin::Pinned(t), _) => Some(t),
            (LangPin::Auto, Some(h)) => Some(&h.tag),
            _ => None,
        }
    }
}

/// Side-band language events (engine <-> client). Additive to `ToHermes`.
#[derive(Debug, Clone, PartialEq)]
pub enum LangEvent {
    SetHint(LanguageHint),
    ClearHint,
    Pin(LangTag),
    Unpin,
    /// Engine refused a pin because the language is not enabled in this build.
    PinRejected(LangTag),
}

impl LangPrefs {
    /// Apply one event; returns `false` for engine-originated events.
    pub fn apply(&mut self, e: &LangEvent) -> bool {
        match e {
            LangEvent::SetHint(h) => self.hint = Some(h.clone()),
            LangEvent::ClearHint => self.hint = None,
            LangEvent::Pin(t) => self.pin = LangPin::Pinned(t.clone()),
            LangEvent::Unpin => self.pin = LangPin::Auto,
            LangEvent::PinRejected(_) => { self.pin = LangPin::Auto; return false; }
        }
        true
    }
}
