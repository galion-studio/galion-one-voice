//! Consent gate for inbound affect inference.
//!
//! Legal context (engineering notes, NOT legal advice — review with counsel / DPO):
//!
//! * **GDPR (EU 2016/679)** — a voice is personal data; inferring a person's
//!   emotional state is profiling-adjacent processing of personal data.
//!   Art. 6 lawful basis needed -> we use explicit, specific, freely-given
//!   consent (Art. 7: withdrawable at any time, as easy as giving it).
//!   Emotion inferences can reveal health data (Art. 9 special category, e.g.
//!   depression/anxiety signals) -> treat as *explicit* consent territory.
//!   Art. 5(1)(c) data minimisation + Art. 25 privacy by design: frames are
//!   computed on-device, never persisted by default, never used to identify.
//! * **EU AI Act (EU 2024/1689)** — Art. 5(1)(f): emotion recognition in the
//!   **workplace and education** is *prohibited* (except medical/safety
//!   reasons). Art. 50(3): deployers of emotion recognition must **inform**
//!   natural persons exposed to it. Emotion recognition systems are also
//!   listed in Annex III (high-risk) where not prohibited.
//! * **Croatia** — GDPR applies directly; national implementation via
//!   *Zakon o provedbi Opće uredbe o zaštiti podataka* (NN 42/2018);
//!   supervisory authority is **AZOP** (Agencija za zaštitu osobnih podataka).
//!   Croatian-language consent text should be offered to Croatian callers.
//!
//! Engineering consequence: **default deny**. No consent -> no inbound
//! affect inference at all (the band carries `None`). Outbound *expression*
//! (Galion's own voice) needs no caller consent, because it processes no
//! caller data — except when it is *driven by* caller affect, which is
//! itself gated here.

/// Why emotion inference would run. Used to enforce AI Act Art. 5(1)(f).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// Personal / consumer conversation with Galion (allowed with consent).
    PersonalAssistant,
    /// Accessibility or wellbeing feature requested by the user themself.
    Accessibility,
    /// Medical or safety use — AI Act exception; still needs consent + review.
    MedicalOrSafety,
    /// Workplace context (employer/employee) — PROHIBITED by AI Act Art. 5(1)(f).
    Workplace,
    /// Education context (school/university) — PROHIBITED by AI Act Art. 5(1)(f).
    Education,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsentState {
    /// Never asked. Treated as deny.
    Unknown,
    /// Explicit opt-in. `at_ms` = unix ms of consent, `notice_lang` = language
    /// of the Art. 50(3)/GDPR notice shown (e.g. "hr", "en").
    Granted { at_ms: u64, notice_lang: String, purpose: Purpose },
    Denied,
    /// Withdrawn after being granted (GDPR Art. 7(3)). Must stop immediately.
    Withdrawn { at_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentDecision {
    Allow,
    DenyNoConsent,
    DenyWithdrawn,
    /// Blocked by law regardless of consent (AI Act Art. 5(1)(f)).
    DenyProhibitedPurpose,
    /// Consent given for a different purpose (GDPR purpose limitation, Art. 5(1)(b)).
    DenyPurposeMismatch,
}

impl ConsentDecision {
    pub fn allowed(self) -> bool {
        matches!(self, ConsentDecision::Allow)
    }
}

/// Hook point: hosts (Galion OS, Hermes, Galion One) plug their own consent
/// store / UI here.
pub trait ConsentGate {
    fn check(&self, purpose: Purpose) -> ConsentDecision;
    /// Called on every decision so hosts can keep an Art. 30-style record of
    /// processing (decision only — never audio or frames).
    fn audit(&self, _purpose: Purpose, _decision: ConsentDecision) {}
}

/// Reference gate: in-memory state, default deny.
#[derive(Debug, Clone)]
pub struct DefaultConsentGate {
    pub state: ConsentState,
}

impl Default for DefaultConsentGate {
    fn default() -> Self {
        Self { state: ConsentState::Unknown }
    }
}

impl DefaultConsentGate {
    pub fn grant(&mut self, at_ms: u64, notice_lang: &str, purpose: Purpose) {
        self.state = ConsentState::Granted { at_ms, notice_lang: notice_lang.to_string(), purpose };
    }
    pub fn withdraw(&mut self, at_ms: u64) {
        self.state = ConsentState::Withdrawn { at_ms };
    }
}

impl ConsentGate for DefaultConsentGate {
    fn check(&self, purpose: Purpose) -> ConsentDecision {
        if matches!(purpose, Purpose::Workplace | Purpose::Education) {
            return ConsentDecision::DenyProhibitedPurpose;
        }
        match &self.state {
            ConsentState::Unknown | ConsentState::Denied => ConsentDecision::DenyNoConsent,
            ConsentState::Withdrawn { .. } => ConsentDecision::DenyWithdrawn,
            ConsentState::Granted { purpose: p, .. } if *p == purpose => ConsentDecision::Allow,
            ConsentState::Granted { .. } => ConsentDecision::DenyPurposeMismatch,
        }
    }
}

/// Short bilingual notice text (AI Act Art. 50(3) transparency). Placeholder
/// wording — to be reviewed.
pub fn notice_text(lang: &str) -> &'static str {
    match lang {
        "hr" => "Galion može procjenjivati ton Vašeg glasa (emocije) isključivo na Vašem uređaju kako bi odgovarao toplije. Ništa se ne pohranjuje. Pristanak možete povući u bilo kojem trenutku.",
        _ => "Galion can estimate the tone of your voice (emotion) on your device only, to respond more warmly. Nothing is stored. You can withdraw consent at any time.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_deny() {
        let g = DefaultConsentGate::default();
        assert_eq!(g.check(Purpose::PersonalAssistant), ConsentDecision::DenyNoConsent);
    }

    #[test]
    fn grant_then_withdraw() {
        let mut g = DefaultConsentGate::default();
        g.grant(1, "hr", Purpose::PersonalAssistant);
        assert!(g.check(Purpose::PersonalAssistant).allowed());
        g.withdraw(2);
        assert_eq!(g.check(Purpose::PersonalAssistant), ConsentDecision::DenyWithdrawn);
    }

    #[test]
    fn ai_act_prohibited_purposes_blocked_even_with_consent() {
        let mut g = DefaultConsentGate::default();
        g.grant(1, "en", Purpose::Workplace);
        assert_eq!(g.check(Purpose::Workplace), ConsentDecision::DenyProhibitedPurpose);
        assert_eq!(g.check(Purpose::Education), ConsentDecision::DenyProhibitedPurpose);
    }

    #[test]
    fn purpose_limitation() {
        let mut g = DefaultConsentGate::default();
        g.grant(1, "en", Purpose::Accessibility);
        assert_eq!(g.check(Purpose::PersonalAssistant), ConsentDecision::DenyPurposeMismatch);
    }

    #[test]
    fn notices_exist() {
        assert!(notice_text("hr").contains("pristanak") || notice_text("hr").contains("Pristanak"));
        assert!(notice_text("en").contains("consent"));
    }
}
