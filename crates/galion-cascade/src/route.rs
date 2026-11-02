// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! W21-03: per-turn language route for the cascade lane (plan P0.2 contract).
//!
//! Precedence: explicit `lang_pin` > sideband `language_hint` > LID > default.
//! - An unsupported **pin** is an error (the caller asked for something we
//!   cannot honour; never silently switch).
//! - An unsupported **hint** is advisory: it is ignored and routing falls
//!   through to LID ("unknown hint -> auto").
//! - LID is a stub that always answers EN in this build.
//!
//! ENGLISH ONLY. Every non-EN tag (he-IL, pl, ...) is rejected/ignored.

use crate::{CascadeError, Lang};

/// Where the routed language came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSource { Pin, Hint, Lid }

/// One routing decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteDecision { pub lang: Lang, pub source: RouteSource }

/// Language-ID slot (first ~1–2 frames of audio). Stub: always EN.
pub trait Lid { fn detect(&mut self, pcm: &[i16]) -> Option<Lang>; }

/// Stub LID: EN for any non-empty audio.
pub struct EnLid;
impl Lid for EnLid {
    fn detect(&mut self, pcm: &[i16]) -> Option<Lang> { if pcm.is_empty() { None } else { Some(Lang::En) } }
}

/// Cascade router: pin > hint > LID. EN-only in this build.
pub struct CascadeRoute<L: Lid = EnLid> { pub lid: L, pub last: Option<RouteDecision> }

impl Default for CascadeRoute<EnLid> { fn default() -> Self { Self { lid: EnLid, last: None } } }

impl<L: Lid> CascadeRoute<L> {
    pub fn new(lid: L) -> Self { Self { lid, last: None } }

    /// Route one turn. `pin` / `hint` are BCP-47-ish tags (e.g. "en", "en-GB").
    pub fn route(&mut self, pin: Option<&str>, hint: Option<&str>, pcm: &[i16]) -> Result<RouteDecision, CascadeError> {
        let d = if let Some(p) = pin {
            let lang = Lang::parse(p.trim()).ok_or(CascadeError::Unsupported("lang_pin not enabled (EN-only build)"))?;
            RouteDecision { lang, source: RouteSource::Pin }
        } else if let Some(lang) = hint.and_then(|h| Lang::parse(h.trim())) {
            RouteDecision { lang, source: RouteSource::Hint }
        } else {
            let lang = self.lid.detect(pcm).ok_or(CascadeError::Empty)?;
            RouteDecision { lang, source: RouteSource::Lid }
        };
        self.last = Some(d);
        Ok(d)
    }
}

/// Convenience: the EN route function used by the live-demo lane.
pub fn route_en(pin: Option<&str>, hint: Option<&str>, pcm: &[i16]) -> Result<RouteDecision, CascadeError> {
    CascadeRoute::default().route(pin, hint, pcm)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PCM: &[i16] = &[100; 160];
    #[test] fn lid_default_is_en() { assert_eq!(route_en(None, None, PCM).unwrap(), RouteDecision { lang: Lang::En, source: RouteSource::Lid }); }
    #[test] fn pin_beats_hint() { assert_eq!(route_en(Some("en-GB"), Some("en"), PCM).unwrap().source, RouteSource::Pin); }
    #[test] fn hint_beats_lid() { assert_eq!(route_en(None, Some("en-US"), PCM).unwrap().source, RouteSource::Hint); }
    #[test] fn unknown_hint_falls_to_lid() {
        for h in ["he-IL", "pl", "xx", ""] { assert_eq!(route_en(None, Some(h), PCM).unwrap().source, RouteSource::Lid, "{h}"); }
    }
    #[test] fn unsupported_pin_is_error() {
        for p in ["he-IL", "pl", "es"] { assert!(matches!(route_en(Some(p), None, PCM), Err(CascadeError::Unsupported(_))), "{p}"); }
    }
    #[test] fn empty_audio_without_pin_or_hint_is_empty() { assert_eq!(route_en(None, None, &[]), Err(CascadeError::Empty)); }
    #[test] fn pin_works_without_audio() { assert_eq!(route_en(Some("en"), None, &[]).unwrap().lang, Lang::En); }
    #[test] fn remembers_last() { let mut r = CascadeRoute::default(); r.route(None, Some("en"), PCM).unwrap(); assert_eq!(r.last.unwrap().source, RouteSource::Hint); }
}
