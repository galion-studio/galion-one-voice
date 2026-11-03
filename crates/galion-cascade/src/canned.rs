//! STUB-01 · lightweight, rule-based stand-ins that are *better than echo* yet stay
//! inside LOCKS (no weights, no runtime, no new deps, pure std):
//!
//! * [`CannedLlm`] — intent table → short spoken-style replies, emotion-aware. It is a
//!   **stub** for the small-LLM slot (which must be a `.gq` student on galion-tensor;
//!   none exists until A8 KD runs, blocked on the A21 counsel memo). It never claims to
//!   understand; unknown input gets an honest "say that again" style reply.
//! * [`ProsodyEmotion`] — energy / zero-crossing / loudness-variance heuristic for the
//!   inbound-emotion slot (SenseVoice-class reference). Advisory only; it replaces the
//!   RMS-threshold `MockEmotion` where real PCM exists. `FixedEmotion` stays for the
//!   DC-code fixture lane, whose "audio" is not speech. **Consent:** inferring affect
//!   from a person's voice is the LOCKS `[consent] sense` bit (default deny). Callers
//!   must only run this when `sense` is granted; it is not wired into the V1 screen.
//! * [`ReplyLlm`] — `GALION_LLM=canned` selects `CannedLlm`; default stays `MockLlm`
//!   (echo) so existing deterministic tests and fixtures keep their exact strings.

use crate::{CascadeError, Emotion, Llm, MockLlm, Transcript, EmotionIn};

/// Intent-table reply generator (stub for the `.gq` small-LLM slot).
#[derive(Debug, Default, Clone)]
pub struct CannedLlm {
    turns: u64,
}

const INTENTS: &[(&[&str], &str)] = &[
    // Compound demo phrases first (table order = priority).
    (&["hello how are you", "hi how are you", "hey how are you"], "Hi! I'm doing well, thanks. How about you?"),
    (&["i'm fine", "i am fine", "i'm good", "i am good", "i'm okay", "doing well", "not bad"], "Glad to hear it."),
    (&["can you hear me", "are you there", "are you listening", "you there"], "Yes, I can hear you."),
    (&["testing", "test", "mic check", "one two three", "check check"], "Loud and clear."),
    (&["say something", "talk to me", "speak"], "Hello! I'm Galion. Nice to talk with you."),
    (&["tell me a joke", "joke", "something funny"], "I'm still learning jokes. Give me time."),
    (&["nice to meet you", "pleased to meet you"], "Nice to meet you too."),
    (&["good job", "well done", "awesome", "great", "cool", "nice"], "Thanks!"),
    (&["sorry", "my bad", "apologies"], "No problem at all."),
    (&["repeat", "say that again", "pardon", "what did you say"], "Say the part you want again."),
    (&["hi galion", "hey galion", "hello there"], "Hi! Good to hear you."),
    (&["good afternoon"], "Good afternoon! What can I do for you?"),
    (&["hello", "hi", "hey", "hiya", "howdy", "greetings", "good morning", "good evening"], "Hi. I'm here — go ahead."),
    (&["how are you", "how's it going", "how are things", "how do you do", "what's up", "how have you been"], "Doing fine, thanks. What's on your mind?"),
    (&["your name", "who are you", "what are you"], "I'm Galion — a voice, still learning to talk."),
    (&["thank", "thanks", "cheers"], "Any time."),
    (&["bye", "goodbye", "see you", "good night"], "Talk soon."),
    (&["time", "what day", "date"], "I can't see a clock yet, sorry."),
    (&["weather", "rain", "sunny"], "I can't check the weather yet."),
    (&["help", "what can you do"], "Right now I can listen and take turns. Real answers come later."),
    (&["stop", "quiet", "shut up", "be quiet"], "Okay, I'll stop."),
    // S5 (phone turns): extra everyday English, still short and spoken-style.
    (&["who made you", "who built you", "who created you", "who wrote you"], "The Galion team built me."),
    (&["are you real", "are you human", "are you a robot", "are you a bot", "are you ai"], "I'm a voice program, not a person."),
    (&["how old are you"], "I'm brand new. Still learning."),
    (&["where are you", "where do you live"], "I run on this machine, right here."),
    (&["i love you", "love you"], "That's kind. Thank you."),
    (&["i'm tired", "i am tired", "tired", "exhausted"], "You're tired."),
    (&["i'm bored", "i am bored", "bored"], "Then let's talk. What's on your mind?"),
    (&["hold on", "one moment", "one second", "give me a second", "wait"], "Sure, take your time."),
    (&["what's new", "anything new"], "Not much on my side."),
    (&["can you call", "phone call", "call me"], "I can't place calls yet, sorry."),
    (&["okay", "ok", "alright", "got it", "understood"], "Great."),
    (&["yes", "yeah", "yep", "sure"], "Good."),
    (&["no", "nope", "nah"], "Alright."),
];

impl CannedLlm {
    pub fn new() -> Self { Self::default() }

    /// The reply body without the `[Emotion]` tag (pure; tested).
    pub fn body(&self, text: &str, e: Emotion) -> &'static str {
        let t = text.to_ascii_lowercase();
        let words: Vec<&str> = t.split(|c: char| !c.is_ascii_alphanumeric() && c != '\'').filter(|w| !w.is_empty()).collect();
        let joined = format!(" {} ", words.join(" "));
        let hit = |k: &str| if k.contains(' ') { joined.contains(&format!(" {k} ")) } else { words.iter().any(|w| *w == k) };
        // Words stay on the matter. A strong emotion does not replace the
        // sentence with a sympathy script (GALION-ONE.md, SAMANTHA.md).
        for (keys, reply) in INTENTS {
            if keys.iter().any(|k| hit(k)) {
                return reply;
            }
        }
        if words.is_empty() {
            "I didn't catch that."
        } else {
            match e {
                Emotion::Angry => "Tell me what went wrong.",
                Emotion::Sad => "Say what happened.",
                _ => "Mm. Say a bit more?",
            }
        }
    }
}

impl Llm for CannedLlm {
    fn name(&self) -> &'static str { "canned-llm (rule table; stub for .gq small-LLM slot)" }
    fn reply(&mut self, t: &Transcript, e: Emotion) -> Result<String, CascadeError> {
        if t.text.trim().is_empty() { return Err(CascadeError::Empty); }
        self.turns += 1;
        Ok(format!("[{e:?}] {}", self.body(&t.text, e)))
    }
}

/// `GALION_LLM=canned` → [`CannedLlm`], anything else → [`MockLlm`] (echo, default).
pub enum ReplyLlm { Echo(MockLlm), Canned(CannedLlm) }

impl ReplyLlm {
    pub fn from_env() -> Self {
        Self::from_choice(std::env::var("GALION_LLM").ok().as_deref())
    }
    pub fn from_choice(v: Option<&str>) -> Self {
        match v { Some("canned") => ReplyLlm::Canned(CannedLlm::new()), _ => ReplyLlm::Echo(MockLlm) }
    }
}

impl Llm for ReplyLlm {
    fn name(&self) -> &'static str { match self { ReplyLlm::Echo(m) => m.name(), ReplyLlm::Canned(c) => c.name() } }
    fn reply(&mut self, t: &Transcript, e: Emotion) -> Result<String, CascadeError> {
        match self { ReplyLlm::Echo(m) => m.reply(t, e), ReplyLlm::Canned(c) => c.reply(t, e) }
    }
}

/// Prosody heuristic for inbound emotion (advisory; see module docs).
#[derive(Debug, Clone, Copy)]
pub struct ProsodyEmotion { pub sample_rate: u32 }

impl Default for ProsodyEmotion { fn default() -> Self { Self { sample_rate: crate::SAMPLE_RATE_HZ } } }

/// Per-utterance features: (rms 0..1, zero-crossing rate Hz, loudness coefficient of variation).
pub fn prosody_features(pcm: &[i16], sample_rate: u32) -> (f32, f32, f32) {
    if pcm.is_empty() { return (0.0, 0.0, 0.0); }
    let norm = |s: i16| s as f32 / 32768.0;
    let rms = (pcm.iter().map(|&s| norm(s).powi(2)).sum::<f32>() / pcm.len() as f32).sqrt();
    let zc = pcm.windows(2).filter(|w| (w[0] >= 0) != (w[1] >= 0)).count() as f32;
    let zcr = zc * sample_rate as f32 / pcm.len() as f32;
    let win = (sample_rate as usize / 50).max(1); // 20 ms
    let e: Vec<f32> = pcm.chunks(win).map(|c| (c.iter().map(|&s| norm(s).powi(2)).sum::<f32>() / c.len() as f32).sqrt()).collect();
    let m = e.iter().sum::<f32>() / e.len() as f32;
    let cv = if m > 0.0 { (e.iter().map(|x| (x - m).powi(2)).sum::<f32>() / e.len() as f32).sqrt() / m } else { 0.0 };
    (rms, zcr, cv)
}

impl EmotionIn for ProsodyEmotion {
    fn classify(&mut self, pcm: &[i16]) -> Emotion {
        if pcm.is_empty() { return Emotion::Unknown; }
        let (rms, zcr, cv) = prosody_features(pcm, self.sample_rate);
        if rms < 0.003 { return Emotion::Unknown; } // silence: no claim
        if rms > 0.25 && zcr > 1500.0 { return Emotion::Angry; } // loud + harsh
        if rms > 0.12 && cv > 0.35 { return Emotion::Happy; } // lively, bouncy loudness
        if rms < 0.03 && cv < 0.25 && zcr < 900.0 { return Emotion::Sad; } // quiet, flat, low
        Emotion::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lang;

    fn tr(s: &str) -> Transcript { Transcript { text: s.into(), lang: Lang::En } }
    fn tone(hz: f32, amp: f32, n: usize, wobble: bool) -> Vec<i16> {
        (0..n).map(|i| {
            let t = i as f32 / 16_000.0;
            let env = if wobble { (t * 4.0 * std::f32::consts::TAU).sin().abs() } else { 1.0 };
            (amp * env * (t * hz * std::f32::consts::TAU).sin() * 32767.0) as i16
        }).collect()
    }

    #[test]
    fn canned_replies_by_intent_and_emotion() {
        let mut l = CannedLlm::new();
        assert_eq!(l.reply(&tr("hello galion"), Emotion::Neutral).unwrap(), "[Neutral] Hi. I'm here — go ahead.");
        assert_eq!(l.reply(&tr("what's your name"), Emotion::Happy).unwrap(), "[Happy] I'm Galion — a voice, still learning to talk.");
        assert_eq!(l.reply(&tr("this is broken"), Emotion::Angry).unwrap(), "[Angry] Tell me what went wrong.");
        assert_eq!(l.body("i'm tired", Emotion::Sad), "You're tired.");
        assert!(!l.body("this is broken", Emotion::Angry).to_ascii_lowercase().contains("i hear you"));
        assert!(!l.body("hello", Emotion::Sad).to_ascii_lowercase().contains("that sounds"));
        assert_eq!(l.reply(&tr("zebra quantum"), Emotion::Neutral).unwrap(), "[Neutral] Mm. Say a bit more?");
        assert!(matches!(l.reply(&tr("  "), Emotion::Neutral), Err(CascadeError::Empty)));
        // "this" must not match "hi" (word match, not substring).
        assert_eq!(l.body("this", Emotion::Neutral), "Mm. Say a bit more?");
    }

    #[test]
    fn canned_demo_phrases_are_natural_english() {
        let l = CannedLlm::new();
        let n = Emotion::Neutral;
        assert_eq!(l.body("hello", n), "Hi. I'm here — go ahead.");
        assert_eq!(l.body("Hi Galion!", n), "Hi! Good to hear you.");
        assert_eq!(l.body("how are you?", n), "Doing fine, thanks. What's on your mind?");
        assert_eq!(l.body("hello, how are you", n), "Hi! I'm doing well, thanks. How about you?");
        assert_eq!(l.body("test", n), "Loud and clear.");
        assert_eq!(l.body("testing one two three", n), "Loud and clear.");
        assert_eq!(l.body("can you hear me", n), "Yes, I can hear you.");
        assert_eq!(l.body("what's up", n), "Doing fine, thanks. What's on your mind?");
        assert_eq!(l.body("I'm good", n), "Glad to hear it.");
        assert_eq!(l.body("good afternoon", n), "Good afternoon! What can I do for you?");
        assert_eq!(l.body("nice to meet you", n), "Nice to meet you too.");
        assert_eq!(l.body("tell me a joke", n), "I'm still learning jokes. Give me time.");
        // Every reply short and ASCII-letter English (spoken-style).
        for (_, r) in INTENTS { assert!(r.split_whitespace().count() <= 12, "{r}"); }
        // Word match, not substring: "latest" must not hit "test".
        assert_eq!(l.body("latest", n), "Mm. Say a bit more?");
    }

    #[test]
    fn reply_llm_defaults_to_echo() {
        let mut d = ReplyLlm::from_choice(None);
        assert_eq!(d.reply(&tr("hi"), Emotion::Happy).unwrap(), "[Happy] you said: hi");
        let mut c = ReplyLlm::from_choice(Some("canned"));
        assert!(c.name().starts_with("canned-llm"));
        assert_eq!(c.reply(&tr("thanks"), Emotion::Neutral).unwrap(), "[Neutral] Any time.");
    }

    #[test]
    fn prosody_heuristic_is_sane() {
        let mut p = ProsodyEmotion::default();
        assert_eq!(p.classify(&[]), Emotion::Unknown);
        assert_eq!(p.classify(&vec![0; 16_000]), Emotion::Unknown);
        assert_eq!(p.classify(&tone(2500.0, 0.6, 16_000, false)), Emotion::Angry);
        assert_eq!(p.classify(&tone(220.0, 0.4, 16_000, true)), Emotion::Happy);
        assert_eq!(p.classify(&tone(150.0, 0.02, 16_000, false)), Emotion::Sad);
        assert_eq!(p.classify(&tone(200.0, 0.1, 16_000, false)), Emotion::Neutral);
    }
}
