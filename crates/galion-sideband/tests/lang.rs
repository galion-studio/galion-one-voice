// Galion Voice — Perpetual Open MIT License. W21-03 · P0.1 language_hint / lang_pin.
use galion_sideband::lang::*;

#[test]
fn tag_normalises() {
    assert_eq!(LangTag::parse(" EN-gb ").unwrap().as_str(), "en-GB");
    assert_eq!(LangTag::parse("en_US").unwrap().as_str(), "en-US");
    assert_eq!(LangTag::parse("en").unwrap().primary(), "en");
    assert_eq!(LangTag::parse("es-419").unwrap().as_str(), "es-419");
}

#[test]
fn tag_rejects_garbage() {
    for t in ["", "e", "english", "en-", "en-G", "en-GB-x", "12", "en-1"] { assert!(LangTag::parse(t).is_none(), "{t}"); }
}

#[test]
fn hint_confidence_clamped() {
    let h = LanguageHint::new(LangTag::parse("en").unwrap(), 1.7);
    assert_eq!(h.confidence, 1.0);
    assert_eq!(LanguageHint::new(LangTag::parse("en").unwrap(), -1.0).confidence, 0.0);
}

#[test]
fn precedence_pin_hint_lid() {
    let en = LangTag::parse("en").unwrap();
    let gb = LangTag::parse("en-GB").unwrap();
    let mut p = LangPrefs::default();
    assert_eq!(p.pin, LangPin::Auto);
    assert_eq!(p.precedence(), LangPrecedence::Lid);
    assert_eq!(p.requested(), None);
    assert!(p.apply(&LangEvent::SetHint(LanguageHint::new(en.clone(), 0.8))));
    assert_eq!(p.precedence(), LangPrecedence::Hint);
    assert_eq!(p.requested(), Some(&en));
    assert!(p.apply(&LangEvent::Pin(gb.clone())));
    assert_eq!(p.precedence(), LangPrecedence::Pin);
    assert_eq!(p.requested(), Some(&gb));
    assert!(p.apply(&LangEvent::Unpin));
    assert_eq!(p.precedence(), LangPrecedence::Hint);
    assert!(p.apply(&LangEvent::ClearHint));
    assert_eq!(p.precedence(), LangPrecedence::Lid);
}

#[test]
fn rejected_pin_falls_back_to_auto() {
    let x = LangTag::parse("xx").unwrap();
    let mut p = LangPrefs::default();
    p.apply(&LangEvent::Pin(x.clone()));
    assert!(!p.apply(&LangEvent::PinRejected(x)));
    assert_eq!(p.pin, LangPin::Auto);
}
