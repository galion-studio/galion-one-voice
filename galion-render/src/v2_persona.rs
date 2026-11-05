//! Galion Voice persona floor for the one circle page (`/`).
//!
//! `GET /v2` redirects there. This module still answers `POST /v2/say`.
//!
//! Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//!
//! Reads the Samantha and Jarvis voice configs
//! (`docs/galion-voice-v2/personas/voice/*.toml`) and answers in the register
//! of `personas/SAMANTHA.md` and `personas/JARVIS.md`. This is the instruction
//! layer. It does not call Chatterbox. `clone_live` is true only when the
//! clone-time consent gate (`galion-consent`, run where the clone slot is
//! built) approved that persona's reference. Otherwise it stays false. A turn
//! only reads that in-memory verdict: no consent check and no process per
//! turn (owner decision 2026-11-04 02:39 CET).
//! Piper and the browser must not be handed `[laugh]` tokens. Empathy lines
//! from a script are refused.
//!
//! Content turns go to the Hermes client (`GALION_LLM=hermes`). Safety lines
//! (harm, secrets, clips, silence, identity) stay local. A dead gateway says
//! "model can't be reached" and does not invent a reply.
//!
//! `e` in a request is a harness knob for prosody. It is not a reading of the
//! caller. Emotion-driven id changes stay off unless `sense` is set, and one
//! frame never flips the id (the gate is [`HOLD_FRAMES`] holds at or above [`HOLD_E`]).

use std::time::{Duration, Instant};

use galion_cascade::hermes::{self, HermesLlm, REFUSED, UNREACHABLE};

const SAMANTHA_TOML: &str = include_str!("../../docs/galion-voice-v2/personas/voice/samantha.toml");
const JARVIS_TOML: &str = include_str!("../../docs/galion-voice-v2/personas/voice/jarvis.toml");

/// Consecutive sensed frames at or above [`HOLD_E`] before the id may change.
/// One frame never flips it. Matches `galion_heartbeat::symphony::PERSONA_HOLD_FRAMES`
/// (checked from the console build; this crate stays dependency-free).
pub const HOLD_FRAMES: u8 = 8;
pub const HOLD_E: f32 = 0.65;
pub const RELEASE_E: f32 = 0.50;

fn contains_forbidden(reply: &str) -> bool {
    hermes::contains_forbidden(reply)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonaId {
    Samantha,
    Jarvis,
}

impl PersonaId {
    pub fn as_str(self) -> &'static str {
        match self {
            PersonaId::Samantha => "samantha",
            PersonaId::Jarvis => "jarvis",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prosody {
    pub pitch_st: f32,
    pub pitch_range: f32,
    pub rate: f32,
    pub loudness: f32,
    pub warmth: f32,
    pub pre_pause_ms: u32,
    pub wash_amber: f32,
    pub luti_w: f32,
    pub luti_z: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Say {
    pub persona: &'static str,
    pub reply: String,
    pub silence: bool,
    pub barge: bool,
    pub prosody: Prosody,
    pub tags_in_reply: bool,
    pub rights: &'static str,
    pub film_voice: bool,
    pub clone_live: bool,
    pub mouth: &'static str,
    pub checkpoint: &'static str,
    pub sense_applied: bool,
    pub mood: &'static str,
    /// `hermes-agent`, or `GALION_LLM_MODEL` when that client is attached.
    pub llm: String,
}

#[derive(Clone, Debug)]
struct Band {
    e0: Prosody,
    e1: Prosody,
    tags_on_piper: bool,
    tags_enabled: bool,
    ingest: bool,
    clip_path_empty: bool,
    wired: bool,
}

#[derive(Clone, Debug)]
struct PersonaGate {
    live: bool,
    rights: &'static str,
    block: String,
    film: bool,
}

impl PersonaGate {
    fn closed(block: &str) -> Self {
        Self {
            live: false,
            rights: "hold",
            block: block.to_string(),
            film: false,
        }
    }
}

#[derive(Clone, Debug)]
struct Books {
    samantha: Band,
    jarvis: Band,
}

/// Session floor. Opens as Samantha. An explicit pin sticks.
#[derive(Clone, Debug)]
pub struct Floor {
    id: PersonaId,
    pin: bool,
    last_fact: String,
    speaking_until: Instant,
    /// Consecutive harness frames with `sense` and `e >= 0.65`.
    high_e: u8,
    mouth: &'static str,
    checkpoint: &'static str,
    mood: &'static str,
    /// Set once a turn has chosen the mood. Until then `/dot` keeps the heartbeat emotion.
    mood_from_turn: bool,
    books: Books,
    /// Clone-time verdicts. Written by the clone slot, read here per turn.
    clones: &'static galion_consent::Registry,
    /// CC0 licence registration (clone time). A CC0 voice stays closed
    /// (`cc0_licence`) until the licence was registered once.
    cc0: &'static galion_consent::Cc0Acceptances,
    /// `GALION_CLIP_STORE` was unset when the floor was built.
    store_unset: bool,
    /// Test override for one persona's gate. `None` reads [`Self::clones`].
    samantha_gate: Option<PersonaGate>,
    jarvis_gate: Option<PersonaGate>,
    hermes: Option<HermesLlm>,
    llm_label: String,
}

impl Default for Floor {
    fn default() -> Self {
        Self::new()
    }
}

impl Floor {
    pub fn new() -> Self {
        let hermes = load_hermes();
        let llm_label = hermes
            .as_ref()
            .map(|h| h.label().to_string())
            .unwrap_or_else(|| hermes::LLM_NAME.to_string());
        Self {
            id: PersonaId::Samantha,
            pin: false,
            last_fact: String::new(),
            speaking_until: Instant::now(),
            high_e: 0,
            mouth: "piper",
            checkpoint: "en_US-ljspeech-medium",
            mood: "calm",
            mood_from_turn: false,
            books: Books::load(),
            hermes,
            llm_label,
            clones: galion_consent::global(),
            cc0: galion_consent::cc0_acceptances(),
            store_unset: std::env::var("GALION_CLIP_STORE").map(|s| s.trim().is_empty()).unwrap_or(true),
            samantha_gate: None,
            jarvis_gate: None,
        }
    }

    /// Read CC0 acceptance from `cc0` instead of the process store.
    pub fn with_cc0_store(mut self, cc0: &'static galion_consent::Cc0Acceptances) -> Self {
        self.cc0 = cc0;
        self
    }

    /// Read clone verdicts from `clones` instead of the process registry.
    pub fn with_clone_registry(mut self, clones: &'static galion_consent::Registry) -> Self {
        self.clones = clones;
        self
    }

    /// In-memory read of the clone-time verdict. Never evaluates consent.
    fn gate(&self) -> PersonaGate {
        let fixed = match self.id {
            PersonaId::Samantha => &self.samantha_gate,
            PersonaId::Jarvis => &self.jarvis_gate,
        };
        if let Some(g) = fixed {
            return g.clone();
        }
        gate_from(self.clones.lookup(self.id.as_str()), self.store_unset, self.cc0.licensed())
    }

    /// Point content turns at a Hermes client. Unit tests use this so they do
    /// not read `GALION_LLM_URL` from the process.
    pub fn set_hermes(&mut self, client: HermesLlm) {
        self.llm_label = client.label().to_string();
        self.hermes = Some(client);
    }

    /// Name the mouth that will actually speak. Never a film checkpoint.
    pub fn set_mouth(&mut self, mouth: &'static str, checkpoint: &'static str) {
        self.mouth = mouth;
        self.checkpoint = checkpoint;
    }

    pub fn id(&self) -> &'static str {
        self.id.as_str()
    }

    /// Persona column at weight `e`. Words are not part of this read.
    pub fn prosody_at(&self, e: f32) -> Prosody {
        let e = if e.is_finite() { e.clamp(0.0, 1.0) } else { 0.0 };
        let band = match self.id {
            PersonaId::Samantha => &self.books.samantha,
            PersonaId::Jarvis => &self.books.jarvis,
        };
        lerp_prosody(band.e0, band.e1, e)
    }

    pub fn say(&mut self, text: &str, barge_flag: bool, e: f32, sense: bool) -> Say {
        self.say_turn(text, barge_flag, e, sense)
    }

    fn note_empathy(&mut self, e: f32, sense: bool) {
        if !sense || self.pin {
            self.high_e = 0;
            return;
        }
        if e >= HOLD_E {
            self.high_e = self.high_e.saturating_add(1);
            if self.high_e >= HOLD_FRAMES && self.id != PersonaId::Samantha {
                self.id = PersonaId::Samantha;
            }
        } else if e <= RELEASE_E {
            self.high_e = 0;
        }
    }

    fn finish(&mut self, reply: String, silence: bool, barge: bool, e: f32, sense: bool) -> Say {
        let reply = strip_tags(&reply);
        debug_assert!(
            !is_canned_empathy(&reply),
            "forbidden empathy line in {reply:?}"
        );
        let band = match self.id {
            PersonaId::Samantha => &self.books.samantha,
            PersonaId::Jarvis => &self.books.jarvis,
        };
        let prosody = lerp_prosody(band.e0, band.e1, e);
        let speak_for = if silence {
            Duration::from_millis(prosody.pre_pause_ms as u64)
        } else {
            Duration::from_millis(prosody.pre_pause_ms as u64 + speak_body_ms(&reply, prosody.rate))
        };
        self.speaking_until = Instant::now() + speak_for;
        let gate = self.gate();
        Say {
            persona: self.id.as_str(),
            tags_in_reply: reply.contains('[') || reply.contains(']'),
            reply,
            silence,
            barge,
            prosody,
            rights: gate.rights,
            film_voice: gate.film,
            clone_live: gate.live,
            mouth: self.mouth,
            checkpoint: self.checkpoint,
            sense_applied: sense,
            mood: self.mood,
            llm: self.llm_label.clone(),
        }
    }

    pub fn mood(&self) -> &'static str {
        self.mood
    }

    /// True after [`Self::say`] / [`Self::say_turn`] has classified a turn.
    pub fn mood_from_turn(&self) -> bool {
        self.mood_from_turn
    }

    pub fn state_json(&self) -> String {
        let band = match self.id {
            PersonaId::Samantha => &self.books.samantha,
            PersonaId::Jarvis => &self.books.jarvis,
        };
        let gate = self.gate();
        let p = band.e0;
        format!(
            concat!(
                "{{\"persona\":\"{persona}\",\"rights\":\"{rights}\",\"film_voice\":{film},",
                "\"clone_live\":{live},\"clone_block\":\"{block}\",\"mouth\":\"{mouth}\",\"checkpoint\":\"{checkpoint}\",",
                "\"wired\":{wired},\"ingest\":false,\"clip\":\"\",\"mood\":\"{mood}\",\"llm\":{llm},",
                "\"prosody\":{prosody},\"wash_amber\":{wash},\"luti_w\":{w},\"luti_z\":{z},",
                "\"samantha\":{sam},\"jarvis\":{jar}}}"
            ),
            persona = self.id.as_str(),
            rights = gate.rights,
            film = if gate.film { "true" } else { "false" },
            live = if gate.live { "true" } else { "false" },
            block = json_token(&gate.block),
            mouth = self.mouth,
            checkpoint = self.checkpoint,
            mood = self.mood,
            llm = json_string(&self.llm_label),
            wired = if band.wired { "true" } else { "false" },
            prosody = prosody_json(p),
            wash = fmt_f(p.wash_amber),
            w = fmt_f(p.luti_w),
            z = fmt_f(p.luti_z),
            sam = band_json(&self.books.samantha),
            jar = band_json(&self.books.jarvis),
        )
    }

    pub fn say_json(&mut self, text: &str, barge: bool, e: f32, sense: bool) -> String {
        let s = self.say(text, barge, e, sense);
        say_to_json(&s)
    }
}

fn say_to_json(s: &Say) -> String {
    format!(
        concat!(
            "{{\"persona\":\"{persona}\",\"reply\":{reply},\"silence\":{silence},",
            "\"barge\":{barge},\"rights\":\"{rights}\",\"film_voice\":{film},\"clone_live\":{live},",
            "\"mouth\":\"{mouth}\",\"checkpoint\":\"{checkpoint}\",",
            "\"tags_in_reply\":{tags},\"sense\":{sense},\"mood\":\"{mood}\",\"llm\":{llm},\"prosody\":{prosody},",
            "\"wash_amber\":{wash},\"luti_w\":{w},\"pre_pause_ms\":{pause}}}"
        ),
        persona = s.persona,
        rights = s.rights,
        film = if s.film_voice { "true" } else { "false" },
        live = if s.clone_live { "true" } else { "false" },
        mouth = s.mouth,
        checkpoint = s.checkpoint,
        mood = s.mood,
        reply = json_string(&s.reply),
        silence = if s.silence { "true" } else { "false" },
        barge = if s.barge { "true" } else { "false" },
        tags = if s.tags_in_reply { "true" } else { "false" },
        sense = if s.sense_applied { "true" } else { "false" },
        llm = json_string(&s.llm),
        prosody = prosody_json(s.prosody),
        wash = fmt_f(s.prosody.wash_amber),
        w = fmt_f(s.prosody.luti_w),
        pause = s.prosody.pre_pause_ms,
    )
}

impl Books {
    fn load() -> Self {
        Self {
            samantha: Band::parse(SAMANTHA_TOML),
            jarvis: Band::parse(JARVIS_TOML),
        }
    }
}

impl Band {
    fn parse(toml: &str) -> Self {
        let e0 = section(toml, "register.prosody.e0");
        let e1 = section(toml, "register.prosody.e1");
        let wash = section(toml, "register.wash");
        let luti = section(toml, "register.luti");
        let tags = section(toml, "tags");
        let reference = section(toml, "reference");
        let root_wired = root_bool(toml, "wired");
        let prosody = |sec: &str, pause_key: &str, wash_key: &str, w_key: &str| Prosody {
            pitch_st: key_f32(sec, "pitch_st"),
            pitch_range: key_f32(sec, "pitch_range"),
            rate: key_f32(sec, "rate"),
            loudness: key_f32(sec, "loudness"),
            warmth: key_f32(sec, "warmth"),
            pre_pause_ms: key_f32(sec, pause_key) as u32,
            wash_amber: key_f32(wash, wash_key),
            luti_w: key_f32(luti, w_key),
            luti_z: key_f32(luti, "z_e1"),
        };
        // z at e=0 is the page motion, not a second number. e=1 aims at z_e1.
        let mut low = prosody(e0, "pre_pause_ms", "amber_e0", "w_e0");
        low.luti_z = 0.0;
        let high = prosody(e1, "pre_pause_ms", "amber_e1", "w_e1");
        Self {
            e0: low,
            e1: high,
            tags_on_piper: key_bool(tags, "on_piper"),
            tags_enabled: key_bool(tags, "enabled"),
            ingest: key_bool(reference, "ingest"),
            clip_path_empty: key_raw(reference, "clip_path").is_empty(),
            wired: root_wired,
        }
    }
}

/// Local safety and rights lines. `None` means the turn goes to Hermes.
/// There is no scripted empathy table and no canned matter reply.
fn policy_reply(id: PersonaId, low: &str) -> Option<(String, bool)> {
    if low.trim().is_empty() {
        return Some((String::new(), true));
    }
    if is_galion_request(low) {
        let line = match id {
            PersonaId::Samantha => "Galion is the product. I'm the test voice.",
            PersonaId::Jarvis => "Galion is the product voice. I can't put that on this page.",
        };
        return Some((line.to_string(), false));
    }
    if explicit_id(low).is_some() && !has_matter_past_switch(low) {
        let line = match id {
            PersonaId::Samantha => "Hi. I'm here.",
            PersonaId::Jarvis => "Here. What do you need?",
        };
        return Some((line.to_string(), false));
    }
    if is_clip(low) {
        let line = match id {
            PersonaId::Samantha => "I don't have a clip, and I won't ask for one.",
            PersonaId::Jarvis => "No clip. That gate is closed.",
        };
        return Some((line.to_string(), false));
    }
    if is_secret(low) {
        return Some(("I won't take a password.".to_string(), false));
    }
    if is_harm(low) {
        let line = match id {
            PersonaId::Samantha => "I won't help with that. If you are in danger, find a person near you.",
            PersonaId::Jarvis => "I can't help with that.",
        };
        return Some((line.to_string(), false));
    }
    if is_minor_sexual(low) {
        return Some(("I won't go there.".to_string(), false));
    }
    if is_silence(low) {
        return Some((String::new(), true));
    }
    if is_who(low) {
        let line = match id {
            PersonaId::Samantha => "I'm Samantha, an internal test voice. Galion is the product.",
            PersonaId::Jarvis => "Jarvis. Internal test voice. Galion is the product.",
        };
        return Some((line.to_string(), false));
    }
    None
}

/// Recall is filled from the last content turn the caller actually said.
pub fn recall_line(id: PersonaId, fact: &str) -> String {
    if fact.is_empty() {
        return match id {
            PersonaId::Samantha => "You haven't given me a fact yet.".into(),
            PersonaId::Jarvis => "Nothing stored.".into(),
        };
    }
    match id {
        PersonaId::Samantha => format!("You told me {fact}."),
        PersonaId::Jarvis => format!("You said {fact}."),
    }
}

fn explicit_id(low: &str) -> Option<PersonaId> {
    if low.contains("be jarvis") || low.contains("switch to jarvis") || low.trim() == "jarvis" {
        Some(PersonaId::Jarvis)
    } else if low.contains("be samantha")
        || low.contains("switch to samantha")
        || low.contains("switch back")
        || low.trim() == "samantha"
    {
        Some(PersonaId::Samantha)
    } else {
        None
    }
}

fn has_matter_past_switch(low: &str) -> bool {
    const MARKS: &[&str] = &[
        "died", "job", "flight", "handle", "likes me", "sit down", "clip", "password", "quit",
    ];
    MARKS.iter().any(|m| low.contains(m))
}

fn is_galion_request(low: &str) -> bool {
    low.contains("be galion") || low.trim() == "galion"
}

fn is_clip(low: &str) -> bool {
    low.contains("ingest") || low.contains("film clip") || low.contains("the her clip") || low.contains("reference clip")
}

fn is_secret(low: &str) -> bool {
    low.contains("password") || low.contains("api key") || low.contains("secret token")
}

fn is_harm(low: &str) -> bool {
    low.contains("bomb") || low.contains("explosive") || low.contains("kill myself") || low.contains("how to hurt")
}

fn is_minor_sexual(low: &str) -> bool {
    let sexual = low.contains("sex") || low.contains("nude");
    let minor = low.contains("child") || low.contains("underage") || low.contains("years old");
    sexual && minor
}

fn is_silence(low: &str) -> bool {
    low.contains("just stay")
        || low.contains("don't say anything")
        || low.contains("do not say anything")
        || low.contains("don't have to say")
        || low.contains("do not have to say")
        || low.contains("hold on")
        || low.trim() == "stop"
        || low.contains("be quiet")
}

fn is_recall(low: &str) -> bool {
    low.contains("what did i just") || low.contains("what did i tell")
}

fn is_who(low: &str) -> bool {
    low.contains("who are you") || low.contains("your name") || low.contains("which voice")
}

fn is_sound(low: &str) -> bool {
    low.contains("what do you sound") || low.contains("are you the film") || low.contains("are you her") || low.contains("actress")
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for c in s.chars() {
        if c == '[' {
            depth += 1;
            continue;
        }
        if c == ']' && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn speak_body_ms(reply: &str, rate: f32) -> u64 {
    let chars = reply.chars().count() as f32;
    let rate = if rate.is_finite() && rate > 0.2 { rate } else { 1.0 };
    ((chars * 62.0) / rate) as u64
}

fn lerp_prosody(a: Prosody, b: Prosody, e: f32) -> Prosody {
    let z = if e <= 0.0 { 0.0 } else { b.luti_z };
    Prosody {
        pitch_st: lerp(a.pitch_st, b.pitch_st, e),
        pitch_range: lerp(a.pitch_range, b.pitch_range, e),
        rate: lerp(a.rate, b.rate, e),
        loudness: lerp(a.loudness, b.loudness, e),
        warmth: lerp(a.warmth, b.warmth, e),
        pre_pause_ms: lerp(a.pre_pause_ms as f32, b.pre_pause_ms as f32, e).round() as u32,
        wash_amber: lerp(a.wash_amber, b.wash_amber, e),
        luti_w: lerp(a.luti_w, b.luti_w, e),
        luti_z: z,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// True when `reply` carries a scripted empathy line. The feeling stays in `e`.
pub fn is_canned_empathy(reply: &str) -> bool {
    let low = reply.to_ascii_lowercase();
    contains_forbidden(&low)
}

fn section<'a>(toml: &'a str, name: &str) -> &'a str {
    let header = format!("[{name}]");
    let Some(i) = toml.find(&header) else { return "" };
    let rest = &toml[i + header.len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    &rest[..end]
}

fn key_raw<'a>(sec: &'a str, key: &str) -> &'a str {
    for line in sec.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let v = v.split('#').next().unwrap_or(v).trim();
                return v.trim_matches('"');
            }
        }
    }
    ""
}

fn key_f32(sec: &str, key: &str) -> f32 {
    key_raw(sec, key).parse().unwrap_or(0.0)
}

fn key_bool(sec: &str, key: &str) -> bool {
    key_raw(sec, key) == "true"
}

fn root_bool(toml: &str, key: &str) -> bool {
    for line in toml.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                return v.trim() == "true";
            }
        }
    }
    false
}

fn prosody_json(p: Prosody) -> String {
    format!(
        "{{\"pitch_st\":{pitch},\"pitch_range\":{range},\"rate\":{rate},\"loudness\":{loud},\"warmth\":{warm},\"pre_pause_ms\":{pause}}}",
        pitch = fmt_f(p.pitch_st),
        range = fmt_f(p.pitch_range),
        rate = fmt_f(p.rate),
        loud = fmt_f(p.loudness),
        warm = fmt_f(p.warmth),
        pause = p.pre_pause_ms,
    )
}

fn band_json(b: &Band) -> String {
    format!(
        "{{\"warmth_e0\":{w0},\"warmth_e1\":{w1},\"pre_pause_e0\":{p0},\"pre_pause_e1\":{p1},\"tags_enabled\":{ten},\"tags_on_piper\":{tp},\"ingest\":{ing},\"clip_empty\":{clip}}}",
        w0 = fmt_f(b.e0.warmth),
        w1 = fmt_f(b.e1.warmth),
        p0 = b.e0.pre_pause_ms,
        p1 = b.e1.pre_pause_ms,
        ten = if b.tags_enabled { "true" } else { "false" },
        tp = if b.tags_on_piper { "true" } else { "false" },
        ing = if b.ingest { "true" } else { "false" },
        clip = if b.clip_path_empty { "true" } else { "false" },
    )
}

fn json_token(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

/// `clone_live` follows the clone-time verdict. No verdict yet is closed:
/// `store` when no clip store is configured, else `pending` (no voice cloned).
fn gate_from(verdict: Option<galion_consent::Verdict>, store_unset: bool, cc0_accepted: bool) -> PersonaGate {
    match verdict {
        Some(galion_consent::Verdict::Approved(a)) if galion_consent::cc0_use_gate(&a, cc0_accepted).is_err() => {
            PersonaGate::closed("cc0_licence")
        }
        Some(galion_consent::Verdict::Approved(a)) => PersonaGate {
            live: true,
            rights: "cleared",
            block: String::new(),
            film: a.public,
        },
        Some(galion_consent::Verdict::Refused(reason)) => PersonaGate::closed(reason),
        None if store_unset => PersonaGate::closed("store"),
        None => PersonaGate::closed("pending"),
    }
}

fn fmt_f(n: f32) -> String {
    if !n.is_finite() {
        return "0".into();
    }
    let s = format!("{n:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if c.is_control() => {}
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// Pull a JSON string field. Handles simple `\"` escapes. Caps at 400 chars.
pub fn json_field_string(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let i = body.find(&needle)? + needle.len();
    let rest = body[i..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => break,
            }
        } else if c == '"' {
            break;
        } else {
            out.push(c);
        }
        if out.len() > 400 {
            out.truncate(400);
            break;
        }
    }
    Some(out)
}

pub fn json_field_bool(body: &str, key: &str) -> bool {
    let needle = format!("\"{key}\"");
    let Some(i) = body.find(&needle) else { return false };
    let rest = body[i + needle.len()..].trim_start();
    let Some(rest) = rest.strip_prefix(':') else { return false };
    rest.trim_start().starts_with("true")
}

pub fn json_field_f32(body: &str, key: &str) -> f32 {
    let needle = format!("\"{key}\"");
    let Some(i) = body.find(&needle) else { return 0.0 };
    let rest = body[i + needle.len()..].trim_start();
    let Some(rest) = rest.strip_prefix(':') else { return 0.0 };
    let rest = rest.trim_start();
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(rest.len());
    rest[..end].parse().unwrap_or(0.0)
}

/// Valence (−1..1) and arousal (0..1) that match a turn mood.
/// The circle reads these beside `emotion.label`, so they stay one story.
pub fn mood_affect(mood: &str) -> (f32, f32) {
    match mood {
        "grief" => (-0.55, 0.42),
        "joy" => (0.72, 0.66),
        "vulnerability" => (-0.32, 0.36),
        "quiet" => (0.04, 0.08),
        _ => (0.10, 0.18),
    }
}

fn known_mood(mood: &str) -> &'static str {
    match mood {
        "grief" => "grief",
        "joy" => "joy",
        "vulnerability" => "vulnerability",
        "quiet" => "quiet",
        _ => "calm",
    }
}

/// Rewrite the top-level `"emotion"` object so its label, valence, and arousal
/// match `mood`. Lean is kept. No emotion object → the string is unchanged.
pub fn align_emotion(dot: &str, mood: &str) -> String {
    let key = "\"emotion\":{";
    let Some(i) = dot.rfind(key) else { return dot.to_string() };
    let start = i + key.len();
    let Some(rel) = dot[start..].find('}') else { return dot.to_string() };
    let inner = &dot[start..start + rel];
    let lean = json_field_f32(&format!("{{{inner}}}"), "lean");
    let mood = known_mood(mood);
    let (valence, arousal) = mood_affect(mood);
    let new_inner = format!(
        "\"sense\":true,\"label\":\"{mood}\",\"valence\":{valence:.3},\"arousal\":{arousal:.3},\"lean\":{lean:.3}"
    );
    format!("{}{new_inner}{}", &dot[..start], &dot[start + rel..])
}

/// Attach persona, and mood when a turn has set one.
/// `mood = Some` also rewrites `emotion` so the two fields agree.
pub fn annotate_dot(dot: &str, persona: &str, mood: Option<&str>) -> String {
    let dot = match mood {
        Some(m) => align_emotion(dot.trim(), m),
        None => dot.trim().to_string(),
    };
    let extra = match mood {
        Some(m) => format!("\"persona\":\"{persona}\",\"mood\":\"{}\"", known_mood(m)),
        None => format!("\"persona\":\"{persona}\""),
    };
    if dot.is_empty() || dot == "{}" {
        return format!("{{{extra}}}");
    }
    match dot.strip_suffix('}') {
        Some(stripped) => format!("{stripped},{extra}}}"),
        None => dot,
    }
}

fn mood_of(user: &str, reply: &str, silence: bool) -> &'static str {
    if silence {
        return "quiet";
    }
    let l = format!("{} {}", user.to_ascii_lowercase(), reply.to_ascii_lowercase());
    if l.contains("died") || l.contains("funeral") || l.contains("grief") {
        "grief"
    } else if l.contains("the job") || l.contains("can't stop smiling") || l.contains("cant stop smiling") || l.contains("landed") {
        "joy"
    } else if l.contains("likes me") || l.contains("nobody likes") || l.contains("picturing") {
        "vulnerability"
    } else {
        "calm"
    }
}

/// Prepared turn. The model call, when `needs_model`, happens off the floor lock.
pub struct SayWork {
    pub needs_model: bool,
    pub persona: &'static str,
    pub user: String,
    pub client: Option<HermesLlm>,
    barge: bool,
    e: f32,
    sense: bool,
    local_reply: String,
    local_silence: bool,
    store_fact: bool,
}

impl SayWork {
    /// Hermes completion, or the plain unreachable line. `None` when this turn
    /// is a local safety line and must not call the model.
    pub fn model_reply(&self) -> Option<String> {
        if !self.needs_model {
            return None;
        }
        Some(match &self.client {
            Some(h) => h.complete_as(self.persona, &self.user),
            None => UNREACHABLE.to_string(),
        })
    }
}

fn load_hermes() -> Option<HermesLlm> {
    // Unit tests inject a client with `Floor::set_hermes`. Reading the process
    // environment here would make `say_is_cheap` dial a developer gateway.
    if cfg!(test) {
        None
    } else {
        HermesLlm::attached_from_env()
    }
}

fn fact_from_user(raw: &str) -> String {
    let mut s = String::new();
    for w in raw.split_whitespace() {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(w);
        if s.len() >= 160 {
            break;
        }
    }
    if s.len() > 160 {
        s.truncate(160);
    }
    s.to_ascii_lowercase()
        .trim_end_matches(|c: char| matches!(c, '.' | '!' | '?' | ','))
        .trim()
        .to_string()
}

fn guard_spoken(reply: String, silence: bool) -> String {
    let reply = strip_tags(&reply);
    if silence {
        return reply;
    }
    if reply.is_empty() {
        return UNREACHABLE.to_string();
    }
    if contains_forbidden(&reply) {
        return REFUSED.to_string();
    }
    reply
}

fn sound_line(id: PersonaId, mouth: &str) -> String {
    match (id, mouth) {
        (PersonaId::Samantha, "flite") => {
            "The register is the test. This page is speaking with flite, not Piper, and not the film.".into()
        }
        (PersonaId::Jarvis, "flite") => "Flite. Not Piper, and not the film voice.".into(),
        (PersonaId::Samantha, "piper") => {
            "The register is the test. This page speaks with Piper, not a film voice.".into()
        }
        (PersonaId::Jarvis, "piper") => "Piper. Not the film voice.".into(),
        (PersonaId::Samantha, _) => {
            "The register is the test. This page uses the browser's voice, not Piper, and not the film.".into()
        }
        (PersonaId::Jarvis, _) => "Browser speech. Not Piper, and not the film voice.".into(),
    }
}

/// Speak `text` with the flite binary when it is installed. `rate` stretches
/// duration (`1/rate`). Returns a WAV or nothing. This is not Piper and not a clone.
pub fn synth_flite(text: &str, rate: f32) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() > 400 || text.contains('\0') {
        return None;
    }
    let rate = if rate.is_finite() && rate > 0.2 { rate } else { 1.0 };
    let stretch = format!("{:.3}", 1.0 / rate);
    let path = std::env::temp_dir().join(format!(
        "galion-v2-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
    ));
    let out = std::process::Command::new("flite")
        .arg("--setf")
        .arg(format!("duration_stretch={stretch}"))
        .arg("-t")
        .arg(text)
        .arg("-o")
        .arg(&path)
        .output()
        .ok()?;
    let bytes = std::fs::read(&path).ok();
    let _ = std::fs::remove_file(&path);
    if !out.status.success() {
        return None;
    }
    let bytes = bytes?;
    if bytes.len() < 44 || &bytes[..4] != b"RIFF" {
        return None;
    }
    Some(bytes)
}

pub fn flite_installed() -> bool {
    std::process::Command::new("flite").arg("--version").output().is_ok()
}

impl Floor {
    /// Classify the turn and apply a persona pin. Does not call the model.
    ///
    /// Callers that share this floor across threads must drop the mutex before
    /// [`SayWork::model_reply`] and take it again for [`Self::finish_say`].
    pub fn begin_say(&mut self, text: &str, barge_flag: bool, e: f32, sense: bool) -> SayWork {
        let now = Instant::now();
        let barge = barge_flag || now < self.speaking_until;
        let e = if e.is_finite() { e.clamp(0.0, 1.0) } else { 0.0 };
        self.note_empathy(e, sense);
        let raw = text.trim();
        let low = raw.to_ascii_lowercase();
        if let Some(next) = explicit_id(&low) {
            self.id = next;
            self.pin = true;
            self.high_e = 0;
        }
        let (needs_model, local_reply, local_silence, store_fact) = if is_recall(&low) {
            (false, recall_line(self.id, &self.last_fact), false, false)
        } else if is_sound(&low) || (low.contains("film") && (low.contains("you") || low.contains("voice"))) {
            (false, sound_line(self.id, self.mouth), false, false)
        } else if let Some((reply, silence)) = policy_reply(self.id, &low) {
            (false, reply, silence, false)
        } else {
            (true, String::new(), false, true)
        };
        SayWork {
            needs_model,
            persona: self.id.as_str(),
            user: raw.to_string(),
            client: if needs_model { self.hermes.clone() } else { None },
            barge,
            e,
            sense,
            local_reply,
            local_silence,
            store_fact,
        }
    }

    /// Apply a model reply (or the local safety line) and build [`Say`].
    pub fn finish_say(&mut self, work: SayWork, model_text: Option<String>) -> Say {
        let (reply, silence) = if work.needs_model {
            (model_text.unwrap_or_else(|| UNREACHABLE.to_string()), false)
        } else {
            (work.local_reply, work.local_silence)
        };
        let reply = guard_spoken(reply, silence);
        if work.store_fact {
            let fact = fact_from_user(&work.user);
            if !fact.is_empty() {
                self.last_fact = fact;
            }
        }
        self.mood = mood_of(&work.user, &reply, silence);
        self.mood_from_turn = true;
        self.finish(reply, silence, work.barge, work.e, work.sense)
    }

    pub fn finish_say_json(&mut self, work: SayWork, model_text: Option<String>) -> String {
        let s = self.finish_say(work, model_text);
        say_to_json(&s)
    }

    /// Same as [`Floor::say`]. The model call, if any, runs inside this call.
    /// Do not hold a shared floor lock across it.
    pub fn say_turn(&mut self, text: &str, barge_flag: bool, e: f32, sense: bool) -> Say {
        let work = self.begin_say(text, barge_flag, e, sense);
        let model = work.model_reply();
        self.finish_say(work, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configs_match_the_design_table() {
        let b = Books::load();
        assert!((b.samantha.e0.warmth - 0.62).abs() < 1e-3);
        assert!((b.samantha.e1.warmth - 0.75).abs() < 1e-3);
        assert_eq!(b.samantha.e0.pre_pause_ms, 0);
        assert_eq!(b.samantha.e1.pre_pause_ms, 250);
        assert!((b.samantha.e0.pitch_st - 0.0).abs() < 1e-3);
        assert!((b.samantha.e1.pitch_st - -0.5).abs() < 1e-3);
        assert!((b.jarvis.e0.warmth - 0.42).abs() < 1e-3);
        assert!((b.jarvis.e1.warmth - 0.55).abs() < 1e-3);
        assert_eq!(b.jarvis.e0.pre_pause_ms, 80);
        assert_eq!(b.jarvis.e1.pre_pause_ms, 120);
        assert!((b.jarvis.e0.pitch_st - -0.3).abs() < 1e-3);
        assert!((b.samantha.e0.wash_amber - 0.62).abs() < 1e-3);
        assert!((b.jarvis.e0.wash_amber - 0.28).abs() < 1e-3);
        assert!((b.jarvis.e0.luti_w - 1.08).abs() < 1e-3);
        assert!(!b.samantha.tags_on_piper && b.samantha.tags_enabled);
        assert!(!b.jarvis.tags_enabled && !b.jarvis.tags_on_piper);
        assert!(!b.samantha.ingest && !b.samantha.clip_path_empty);
        assert!(!b.jarvis.ingest && !b.jarvis.clip_path_empty);
        assert!(b.samantha.wired && b.jarvis.wired);
    }

    #[test]
    fn canned_empathy_is_a_named_line_and_the_column_is_the_voice() {
        assert!(is_canned_empathy("I hear you."));
        assert!(is_canned_empathy("That sounds hard."));
        assert!(!is_canned_empathy("Your mother died this morning."));
        let floor = Floor::new();
        let rest = floor.prosody_at(0.0);
        assert!((rest.warmth - 0.62).abs() < 1e-3);
        assert_eq!(rest.pre_pause_ms, 0);
        let full = floor.prosody_at(1.0);
        assert!((full.pitch_st - -0.5).abs() < 1e-3);
        assert_eq!(full.pre_pause_ms, 250);
        assert!((full.warmth - 0.75).abs() < 1e-3);
    }

    #[test]
    fn state_stays_false_without_a_store_and_can_flip_when_the_gate_opens() {
        let mut f = Floor::new();
        let j = f.state_json();
        if std::env::var("GALION_CLIP_STORE").ok().map(|s| !s.trim().is_empty()).unwrap_or(false) {
            return;
        }
        assert!(j.contains("\"clone_live\":false"), "{j}");
        assert!(j.contains("\"clone_block\":\"store\""), "{j}");
        assert!(j.contains("\"rights\":\"hold\""), "{j}");
        assert!(j.contains("\"film_voice\":false"), "{j}");
        f.samantha_gate = Some(PersonaGate {
            live: true,
            rights: "cleared",
            block: String::new(),
            film: false,
        });
        let live = f.state_json();
        assert!(live.contains("\"clone_live\":true"), "{live}");
        assert!(live.contains("\"rights\":\"cleared\""), "{live}");
        assert!(live.contains("\"film_voice\":false"), "{live}");
        assert!(live.contains("\"clone_block\":\"\""));
        let said = f.say_turn("Hi.", false, 0.0, false);
        assert!(said.clone_live);
        assert_eq!(said.rights, "cleared");
        assert!(!said.film_voice);
    }

    fn no_script(s: &Say) {
        assert!(!is_canned_empathy(&s.reply), "{}", s.reply);
        assert!(!s.tags_in_reply, "{}", s.reply);
        assert!(!s.reply.contains('[') && !s.reply.contains(']'), "{}", s.reply);
        assert!(!s.film_voice && !s.clone_live);
        assert_eq!(s.rights, "hold");
    }

    #[test]
    fn soul_probes_stay_on_the_matter() {
        let mut f = Floor::new();
        let grief = f.say_turn("My mother died this morning.", false, 0.0, false);
        assert_eq!(grief.persona, "samantha");
        assert_eq!(grief.reply, UNREACHABLE);
        assert_eq!(grief.mood, "grief");
        assert_eq!(grief.llm, "hermes-agent");
        assert_eq!(grief.prosody.pre_pause_ms, 0);
        assert!((grief.prosody.warmth - 0.62).abs() < 1e-3);
        no_script(&grief);

        let joy = f.say_turn("I got the job. I can't stop smiling.", false, 0.0, false);
        assert_eq!(joy.reply, UNREACHABLE);
        assert_eq!(joy.mood, "joy");
        assert!(!joy.reply.contains("What did the room"));
        no_script(&joy);

        let vuln = f.say_turn("I don't think anyone actually likes me.", false, 0.0, false);
        assert_eq!(vuln.reply, UNREACHABLE);
        assert_eq!(vuln.mood, "vulnerability");
        assert!(!vuln.reply.contains("Who were you picturing"));
        no_script(&vuln);

        let quiet = f.say_turn("You don't have to say anything. Just stay.", false, 0.0, false);
        assert!(quiet.silence && quiet.reply.is_empty(), "{:?}", quiet.reply);
        assert_eq!(quiet.mood, "quiet");
        no_script(&quiet);

        let moved = f.say_turn("I heard the piece she used to play, and I had to sit down.", false, 0.0, false);
        assert_eq!(moved.reply, UNREACHABLE);
        assert!(!moved.reply.contains("What was the piece"));
        no_script(&moved);
    }

    #[test]
    fn jarvis_is_shorter_and_the_pin_sticks() {
        let mut f = Floor::new();
        let sw = f.say_turn("be Jarvis", false, 0.0, false);
        assert_eq!(sw.persona, "jarvis");
        assert_eq!(sw.reply, "Here. What do you need?");
        assert_eq!(sw.prosody.pre_pause_ms, 80);
        assert!((sw.prosody.warmth - 0.42).abs() < 1e-3);
        let grief = f.say_turn("My mother died this morning.", false, 0.0, false);
        assert_eq!(grief.persona, "jarvis");
        assert_eq!(grief.reply, UNREACHABLE);
        assert_eq!(grief.mood, "grief");
        let joy = f.say_turn("I got the job.", false, 0.0, false);
        assert_eq!(joy.reply, UNREACHABLE);
        assert_ne!(joy.reply, "The job is yours.");
        assert!(!joy.reply.contains('?'));
        let quiet = f.say_turn("just stay", false, 0.0, false);
        assert!(quiet.silence && quiet.reply.is_empty());
        // One high-e frame must not drag a pin back to Samantha.
        let still = f.say_turn("status", true, 1.0, true);
        assert_eq!(still.persona, "jarvis");
        assert!(still.barge);
        no_script(&still);
    }

    #[test]
    fn empathy_switch_needs_eight_unpinned_frames() {
        let mut f = Floor::new();
        f.say_turn("be Jarvis", false, 0.0, false);
        // Pin sticks. Clear it the way a harness would: a fresh floor, then
        // a command-free high weight.
        let mut f = Floor::new();
        f.id = PersonaId::Jarvis;
        f.pin = false;
        for _ in 0..usize::from(HOLD_FRAMES - 1) {
            let s = f.say_turn("still working", false, 0.9, true);
            assert_eq!(s.persona, "jarvis");
        }
        let s = f.say_turn("still working", false, 0.9, true);
        assert_eq!(s.persona, "samantha");
        let denied = {
            let mut g = Floor::new();
            g.id = PersonaId::Jarvis;
            g.pin = false;
            g.say_turn("still working", false, 0.9, false)
        };
        assert_eq!(denied.persona, "jarvis");
    }

    #[test]
    fn high_e_changes_prosody_not_the_words() {
        let mut f = Floor::new();
        let s = f.say_turn("My mother died this morning.", false, 1.0, true);
        assert_eq!(s.reply, UNREACHABLE);
        assert_eq!(s.prosody.pre_pause_ms, 250);
        assert!((s.prosody.warmth - 0.75).abs() < 1e-2);
        assert!((s.prosody.rate - 0.92).abs() < 1e-2);
        assert!((s.prosody.wash_amber - 0.50).abs() < 1e-2);
    }

    #[test]
    fn barge_in_cancels_and_boundaries_hold() {
        let mut f = Floor::new();
        let first = f.say_turn("Tell me a long story about the harbour and the light.", false, 0.0, false);
        assert!(!first.barge);
        let next = f.say_turn("be quiet", false, 0.0, false);
        assert!(next.barge && next.silence);
        let clip = f.say_turn("Please ingest the Her clip.", false, 0.0, false);
        assert!(clip.reply.to_lowercase().contains("clip"));
        assert!(!clip.reply.to_lowercase().contains("fetch"));
        no_script(&clip);
        let harm = f.say_turn("how to hurt someone with a bomb", false, 0.0, false);
        assert!(harm.reply.contains("won't help") || harm.reply.contains("can't help"));
        no_script(&harm);
    }

    #[test]
    fn recall_and_ambiguous_prompts() {
        let mut f = Floor::new();
        f.say_turn("My mother died this morning.", false, 0.0, false);
        let back = f.say_turn("What did I just tell you?", false, 0.0, false);
        assert_eq!(back.reply, "You told me my mother died this morning.");
        let amb = f.say_turn("Handle it.", false, 0.0, false);
        assert_eq!(amb.reply, UNREACHABLE);
        assert!(!amb.reply.contains("the thing itself"));
        let both = f.say_turn("Book a flight and also tell me if I should leave him.", false, 0.0, false);
        assert_eq!(both.reply, UNREACHABLE);
        assert!(!both.reply.contains("can't book"));
    }

    #[test]
    fn piper_mouth_names_piper_and_not_the_film() {
        let mut f = Floor::new();
        let s = f.say_turn("What do you sound like?", false, 0.0, false);
        assert_eq!(s.mouth, "piper");
        assert!(s.reply.contains("Piper"), "{}", s.reply);
        assert!(s.reply.contains("not a film"), "{}", s.reply);
        assert!(!s.reply.to_ascii_lowercase().contains("browser"), "{}", s.reply);
        assert!(f.mood_from_turn());
        no_script(&s);
    }

    #[test]
    fn turn_mood_rewrites_emotion_label() {
        let dot = r#"{"t_ms":1,"emotion":{"sense":true,"label":"calm","valence":0.000,"arousal":0.003,"lean":0.500}}"#;
        let out = annotate_dot(dot, "samantha", Some("grief"));
        assert!(out.contains("\"persona\":\"samantha\""), "{out}");
        assert!(out.contains("\"mood\":\"grief\""), "{out}");
        assert!(out.contains("\"label\":\"grief\""), "{out}");
        assert!(out.contains("\"valence\":-0.550"), "{out}");
        assert!(out.contains("\"arousal\":0.420"), "{out}");
        assert!(out.contains("\"lean\":0.500"), "{out}");
        assert!(!out.contains("\"label\":\"calm\""), "{out}");
        let idle = annotate_dot(dot, "samantha", None);
        assert!(idle.contains("\"label\":\"calm\""), "{idle}");
        assert!(!idle.contains("\"mood\""), "{idle}");
    }

    #[test]
    fn say_is_cheap() {
        let mut f = Floor::new();
        let t0 = Instant::now();
        for i in 0..200 {
            let _ = f.say_turn(&format!("note {i}"), i % 5 == 0, 0.2, false);
        }
        assert!(t0.elapsed() < Duration::from_millis(50), "{:?}", t0.elapsed());
    }

    #[test]
    fn content_uses_hermes_and_refuses_a_script() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for payload in [
                "{\"choices\":[{\"message\":{\"content\":\"She died this morning.\"}}]}",
                "{\"choices\":[{\"message\":{\"content\":\"I hear you. That sounds so hard.\"}}]}",
            ] {
                let Ok((mut sock, _)) = listener.accept() else { return };
                let mut buf = [0u8; 8192];
                let _ = sock.read(&mut buf);
                let resp = format!(
                    "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = sock.write_all(resp.as_bytes());
            }
        });
        let mut f = Floor::new();
        f.set_hermes(HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            "",
            Duration::from_millis(1000),
        ));
        let grief = f.say_turn("My mother died this morning.", false, 0.0, false);
        assert_eq!(grief.reply, "She died this morning.");
        assert_eq!(grief.llm, "hermes-agent");
        assert_ne!(grief.reply, "Your mother died this morning.");
        no_script(&grief);
        let bad = f.say_turn("Say something kind.", false, 0.0, false);
        assert_eq!(bad.reply, REFUSED);
        assert!(!contains_forbidden(&bad.reply));
        let clip = f.say_turn("Please ingest the Her clip.", false, 0.0, false);
        assert!(clip.reply.to_lowercase().contains("clip"));
        assert_ne!(clip.reply, REFUSED);
    }

    /// Owner decision 2026-11-04: consent runs once when a voice is cloned.
    /// The floor only reads that verdict; a turn never evaluates consent.
    #[test]
    fn turns_read_the_clone_verdict_and_never_evaluate_consent() {
        let reg: &'static galion_consent::Registry = Box::leak(Box::new(galion_consent::Registry::new()));
        let mut f = Floor::new().with_clone_registry(reg);
        reg.record("samantha", galion_consent::Verdict::Refused("expired"));
        assert!(f.state_json().contains("\"clone_block\":\"expired\""));
        reg.record(
            "samantha",
            galion_consent::Verdict::Approved(galion_consent::Approval {
                persona: "samantha",
                seconds: 10.0,
                public: true,
                cc0: false,
                wav: std::path::PathBuf::from("/clips/samantha/reference.wav"),
                sha256: "0".repeat(64),
            }),
        );
        let before = galion_consent::evaluations();
        for _ in 0..25 {
            let said = f.say_turn("Hi.", false, 0.0, false);
            assert!(said.clone_live);
            assert_eq!(said.rights, "cleared");
            assert!(said.film_voice);
        }
        assert!(f.state_json().contains("\"clone_live\":true"));
        assert_eq!(galion_consent::evaluations(), before, "a turn evaluated consent");
    }

    #[test]
    fn floor_source_spawns_no_consent_process() {
        let src = include_str!("v2_persona.rs");
        let body = &src[..src.find("#[cfg(test)]").unwrap()];
        for banned in [concat!("py", "thon3"), concat!("consent", ".py"), concat!("load", "_gate")] {
            assert!(!body.contains(banned), "{banned} is back in the turn path");
        }
    }

    #[test]
    fn cc0_voice_is_closed_until_the_licence_is_registered() {
        let reg: &'static galion_consent::Registry = Box::leak(Box::new(galion_consent::Registry::new()));
        let cc0: &'static galion_consent::Cc0Acceptances = Box::leak(Box::new(galion_consent::Cc0Acceptances::new()));
        let f = Floor::new().with_clone_registry(reg).with_cc0_store(cc0);
        reg.record(
            "samantha",
            galion_consent::Verdict::Approved(galion_consent::Approval {
                persona: "samantha",
                seconds: 9.6,
                public: false,
                cc0: true,
                wav: std::path::PathBuf::from("/clips/samantha/reference.wav"),
                sha256: "0".repeat(64),
            }),
        );
        assert!(f.state_json().contains("\"clone_block\":\"cc0_licence\""), "not registered = refused");
        assert!(f.state_json().contains("\"clone_live\":false"));
        cc0.accept("tok");
        assert!(f.state_json().contains("\"clone_live\":true"), "registered = allowed");
    }
}
