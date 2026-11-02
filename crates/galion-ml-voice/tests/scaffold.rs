// Galion Voice — Perpetual Open MIT License
use galion_ml_voice::stub::*;
use galion_ml_voice::*;

#[test]
fn only_english_enabled() {
    assert_eq!(LanguageId::ALL, &[LanguageId::En]);
    assert_eq!(LanguageId::En.code(), "en");
    assert_eq!(LanguageId::En.to_string(), "en");
}

#[test]
fn en_token_roundtrip() {
    assert_eq!(LanguageId::En.token(), "<|en|>");
    assert_eq!(LanguageId::from_token("<|en|>"), Some(LanguageId::En));
    assert_eq!(LanguageId::from_token(" <|EN|> "), Some(LanguageId::En));
    assert_eq!(LanguageId::from_token("<|pl|>"), None);
    assert_eq!(LanguageId::from_token("en"), None);
    assert_eq!(LanguageId::from_code("pl"), None);
}

#[test]
fn token_prefix_helpers() {
    let s = with_lang_token(LanguageId::En, FIXTURE_EN_TEXT);
    assert_eq!(s, "<|en|>hello galion");
    assert_eq!(split_lang_token(&s), (Some(LanguageId::En), FIXTURE_EN_TEXT));
    assert_eq!(split_lang_token("<|xx|>hi"), (None, "<|xx|>hi"));
    assert_eq!(split_lang_token("plain"), (None, "plain"));
}

#[test]
fn synthetic_fixture_shape() {
    let a = fixture_audio(FIXTURE_EN_TEXT);
    assert_eq!(a.sample_rate, SAMPLE_RATE);
    assert_eq!(a.samples.len(), FIXTURE_EN_TEXT.len() * SAMPLES_PER_CHAR);
    assert!(a.samples.iter().all(|s| (0.0..=1.0).contains(s)));
}

#[test]
fn tts_then_asr_roundtrip() {
    let mut tts = StubTts;
    let mut asr = StubAsr;
    let router = EnglishOnlyRouter;
    let tagged = with_lang_token(LanguageId::En, FIXTURE_EN_TEXT);
    let lang = router.route_text(&tagged);
    assert_eq!(lang, LanguageId::En);
    let audio = tts.synthesize(&tagged, lang).unwrap();
    assert_eq!(router.route_audio(&audio), LanguageId::En);
    let t = asr.transcribe(&audio, None).unwrap();
    assert_eq!(t, Transcript { lang: LanguageId::En, text: FIXTURE_EN_TEXT.into() });
}

#[test]
fn empty_inputs_error() {
    let empty = Audio { sample_rate: SAMPLE_RATE, samples: vec![] };
    assert_eq!(StubAsr.transcribe(&empty, None), Err(VoiceError::EmptyInput));
    assert_eq!(StubTts.synthesize("<|en|>", LanguageId::En), Err(VoiceError::EmptyInput));
}

#[test]
fn traits_are_object_safe() {
    let _a: Box<dyn AsrEngine> = Box::new(StubAsr);
    let _t: Box<dyn TtsEngine> = Box::new(StubTts);
    let _r: Box<dyn LangRouter> = Box::new(EnglishOnlyRouter);
}
