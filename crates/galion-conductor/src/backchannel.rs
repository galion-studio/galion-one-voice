// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Listening backchannel while the user still holds the floor.
//!
//! Samantha only. One short nod at least [`BACKCHANNEL_GAP_MS`] after the
//! last one, on the quiet frame that follows a clause-final fall. A yield,
//! a question rise, a filler, a denied ear, and any persona other than
//! Samantha stay silent. The nod is quieter than ordinary speech.
//!
//! `horbm-voice` keeps the same numbers. The crates are separate workspaces,
//! so this file is the conductor's copy. The voice loop is the mouth.

use crate::clock::FRAME_MS;

/// Process environment that closes listening backchannels.
/// Unset means on. `0` and `off` close them. Matching is trimmed and
/// case-insensitive. `1`, `on`, and `allow` stay on.
pub const BACKCHANNEL_ENV: &str = "GALION_BACKCHANNEL";

/// Minimum gap between nods. The middle of the 4–6 s band, on the 80 ms grid.
pub const BACKCHANNEL_GAP_MS: u32 = 4_800;

/// Ordinary fake-mouth speech is at least this peak. A nod stays under it.
pub const SPEECH_PEAK_MIN: i16 = 200;

/// Highest sample a generated nod may use.
pub const BACKCHANNEL_PEAK_MAX: i16 = 110;

const _: () = assert!(BACKCHANNEL_GAP_MS == 4_800);
const _: () = assert!(BACKCHANNEL_GAP_MS % FRAME_MS == 0);
const _: () = assert!(BACKCHANNEL_GAP_MS / FRAME_MS == 60);
const _: () = assert!(BACKCHANNEL_PEAK_MAX < SPEECH_PEAK_MIN);

/// What the ear's listening cue said. Same four variants as `horbm_ear::ListenCue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenCue {
    None,
    ClauseFall,
    QuestionRise,
    Filler,
}

/// One of Samantha's four listening sounds. Not a reply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackchannelKind {
    MmHm,
    Yeah,
    Right,
    /// A soft breath-laugh. The text is a hum so a looped transcript still
    /// matches the listed-backchannel rule and does not cancel the user.
    BreathLaugh,
}

impl BackchannelKind {
    pub const fn text(self) -> &'static str {
        match self {
            Self::MmHm => "mm-hm",
            Self::Yeah => "yeah",
            Self::Right => "right",
            Self::BreathLaugh => "mm",
        }
    }
}

/// Valence −1..=1, arousal 0..=1. Dominance is not used for the choice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affect {
    pub valence: f32,
    pub arousal: f32,
}

impl Affect {
    fn sanitized(self) -> Self {
        Self {
            valence: if self.valence.is_finite() {
                self.valence.clamp(-1.0, 1.0)
            } else {
                0.25
            },
            arousal: if self.arousal.is_finite() {
                self.arousal.clamp(0.0, 1.0)
            } else {
                0.3
            },
        }
    }
}

/// Samantha's moment, plus the listener's feeling when the bus carried one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListenMood {
    pub moment: Affect,
    /// Inbound feeling. `None` when the frame had no listener feeling.
    pub listener: Option<Affect>,
}

impl ListenMood {
    /// Resting Samantha. Valence 0.25, arousal 0.3, no listener on the bus.
    pub const fn resting() -> Self {
        Self {
            moment: Affect {
                valence: 0.25,
                arousal: 0.3,
            },
            listener: None,
        }
    }
}

/// The nod this frame may play. `energy` is a fraction of ordinary speech.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backchannel {
    pub kind: BackchannelKind,
    pub text: &'static str,
    pub energy: f32,
    /// Quiet frames since the clause-final voiced frame. The short pause is 1.
    pub frames_after_fall: u32,
}

/// Config flag. Default is on. The environment is a second switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackchannelConfig {
    pub enabled: bool,
}

impl Default for BackchannelConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl BackchannelConfig {
    pub fn from_env(value: Option<&str>) -> Self {
        Self {
            enabled: !env_disables(value),
        }
    }

    /// [`BACKCHANNEL_ENV`]. A missing or non-Unicode value stays on.
    pub fn from_process_env() -> Self {
        match std::env::var(BACKCHANNEL_ENV) {
            Ok(value) => Self::from_env(Some(&value)),
            Err(_) => Self::default(),
        }
    }
}

/// `0` and `off` close the layer. Anything else, including unset, leaves it on.
pub fn env_disables(value: Option<&str>) -> bool {
    matches!(
        value.map(|raw| raw.trim().to_ascii_lowercase()).as_deref(),
        Some("0") | Some("off")
    )
}

/// True only for Samantha. Aria stays parked. A clone pack id is refused.
pub fn persona_allows(id: &str) -> bool {
    id.trim().eq_ignore_ascii_case("samantha")
}

/// Inbound feeling band, EmotionWire layout. Outbound and absent frames are
/// not the listener. This does not link `galion-emotion`.
pub fn listener_on_bus(wire: &[u8; 8]) -> Option<Affect> {
    let flags = wire[2];
    if flags & 0b1 == 0 || flags & 0b10 != 0 {
        return None;
    }
    let valence = f32::from(wire[3] as i8) / 127.0;
    let arousal = f32::from(wire[4]) / 255.0;
    if !valence.is_finite() || !arousal.is_finite() {
        return None;
    }
    Some(Affect {
        valence: valence.clamp(-1.0, 1.0),
        arousal: arousal.clamp(0.0, 1.0),
    })
}

/// Which sound, and how quiet. The listener, when present, tugs the moment.
pub fn choose(mood: ListenMood) -> (BackchannelKind, f32) {
    let blended = blend(mood);
    let kind = choose_kind(blended);
    (kind, energy_of(kind, blended))
}

fn blend(mood: ListenMood) -> Affect {
    let moment = mood.moment.sanitized();
    match mood.listener.map(Affect::sanitized) {
        None => moment,
        Some(heard) => Affect {
            valence: (0.65 * moment.valence + 0.35 * heard.valence).clamp(-1.0, 1.0),
            arousal: (0.65 * moment.arousal + 0.35 * heard.arousal).clamp(0.0, 1.0),
        },
    }
}

fn choose_kind(affect: Affect) -> BackchannelKind {
    // Soft laugh only when she is warm and the moment is not flat or hot.
    if affect.valence >= 0.55 && (0.40..0.72).contains(&affect.arousal) {
        return BackchannelKind::BreathLaugh;
    }
    if affect.arousal >= 0.60 && affect.valence >= 0.10 {
        return BackchannelKind::Yeah;
    }
    if affect.arousal >= 0.45 && affect.valence >= 0.05 {
        return BackchannelKind::Right;
    }
    BackchannelKind::MmHm
}

fn energy_of(kind: BackchannelKind, affect: Affect) -> f32 {
    let mut energy = 0.30 + 0.22 * affect.arousal;
    if affect.valence < 0.0 {
        energy *= 0.85;
    }
    if kind == BackchannelKind::BreathLaugh {
        energy *= 0.80;
    }
    energy.clamp(0.22, 0.55)
}

/// Floor policy for one conductor. The turn machine still owns the floor.
#[derive(Clone, Debug, PartialEq)]
pub struct ListenPolicy {
    pub enabled: bool,
    persona: String,
    last_voiced_ms: Option<u64>,
    last_played_ms: Option<u64>,
}

impl Default for ListenPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl ListenPolicy {
    pub fn new() -> Self {
        Self {
            enabled: true,
            persona: "samantha".to_string(),
            last_voiced_ms: None,
            last_played_ms: None,
        }
    }

    pub fn set_persona(&mut self, persona: impl Into<String>) {
        self.persona = persona.into();
    }

    pub fn persona(&self) -> &str {
        &self.persona
    }

    /// The user is still in a word. A later quiet frame measures from here.
    pub fn note_voiced(&mut self, t_ms: u64) {
        self.last_voiced_ms = Some(t_ms);
    }

    /// Maybe nod. `holding` is false on Yield, on PauseOnly, while Samantha
    /// is speaking, and on any frame that already took or cancelled the turn.
    ///
    /// PauseOnly is the denied-ear path. The cue needs a pitch fall and must
    /// refuse a question rise. A pause timer cannot see either, so the nod
    /// stays off instead of guessing. End-of-turn pause timing is untouched.
    pub fn consider(
        &mut self,
        t_ms: u64,
        quiet: bool,
        holding: bool,
        cue: ListenCue,
        mood: ListenMood,
    ) -> Option<Backchannel> {
        if !self.enabled || !persona_allows(&self.persona) {
            return None;
        }
        if !quiet || !holding || !matches!(cue, ListenCue::ClauseFall) {
            return None;
        }
        let voiced_at = self.last_voiced_ms?;
        let delta = t_ms.saturating_sub(voiced_at);
        if delta != u64::from(FRAME_MS) {
            return None;
        }
        if let Some(prev) = self.last_played_ms {
            if t_ms.saturating_sub(prev) < u64::from(BACKCHANNEL_GAP_MS) {
                return None;
            }
        }
        let (kind, energy) = choose(mood);
        debug_assert!(energy < 1.0);
        self.last_played_ms = Some(t_ms);
        Some(Backchannel {
            kind,
            text: kind.text(),
            energy,
            frames_after_fall: 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_and_the_persona_gate() {
        assert!(BackchannelConfig::default().enabled);
        assert!(BackchannelConfig::from_env(None).enabled);
        assert!(BackchannelConfig::from_env(Some("")).enabled);
        assert!(BackchannelConfig::from_env(Some("1")).enabled);
        assert!(BackchannelConfig::from_env(Some("on")).enabled);
        assert!(!BackchannelConfig::from_env(Some("0")).enabled);
        assert!(!BackchannelConfig::from_env(Some(" off ")).enabled);
        assert!(!BackchannelConfig::from_env(Some("OFF")).enabled);
        assert!(persona_allows("Samantha"));
        assert!(persona_allows(" samantha "));
        assert!(!persona_allows("aria"));
        assert!(!persona_allows("Aria"));
        assert!(!persona_allows("samantha-clone"));
        assert!(!persona_allows("amy-koenig"));
        assert!(!persona_allows(""));
    }

    #[test]
    fn the_choice_follows_the_moment_and_a_listener_on_the_bus() {
        let (kind, energy) = choose(ListenMood::resting());
        assert_eq!(kind, BackchannelKind::MmHm);
        assert_eq!(kind.text(), "mm-hm");
        assert!(energy < 0.55);
        let (sad_kind, sad_energy) = choose(ListenMood {
            moment: Affect {
                valence: -0.4,
                arousal: 0.3,
            },
            listener: None,
        });
        assert_eq!(sad_kind, BackchannelKind::MmHm);
        assert!(sad_energy < energy);

        let warm = ListenMood {
            moment: Affect {
                valence: 0.70,
                arousal: 0.55,
            },
            listener: None,
        };
        assert_eq!(choose(warm).0, BackchannelKind::BreathLaugh);

        let awake = ListenMood {
            moment: Affect {
                valence: 0.40,
                arousal: 0.75,
            },
            listener: None,
        };
        assert_eq!(choose(awake).0, BackchannelKind::Yeah);

        let steady = ListenMood {
            moment: Affect {
                valence: 0.30,
                arousal: 0.50,
            },
            listener: None,
        };
        assert_eq!(choose(steady).0, BackchannelKind::Right);

        let mut wire = [0u8; 8];
        wire[2] = 0b1;
        wire[3] = (-0.8_f32 * 127.0).round() as i8 as u8;
        wire[4] = (0.2_f32 * 255.0).round() as u8;
        let sad = listener_on_bus(&wire).expect("inbound feeling");
        assert!(sad.valence < 0.0);
        let tugged = ListenMood {
            moment: warm.moment,
            listener: Some(sad),
        };
        assert_eq!(choose(tugged).0, BackchannelKind::MmHm);
        assert!(choose(tugged).1 <= 0.55);
        assert!(choose(warm).1 <= 0.55);

        wire[2] = 0b11;
        assert!(
            listener_on_bus(&wire).is_none(),
            "outbound is not the listener"
        );
        wire[2] = 0;
        assert!(listener_on_bus(&wire).is_none(), "absent feeling");
    }

    #[test]
    fn the_policy_nods_once_per_gap_and_refuses_the_rest() {
        let mut policy = ListenPolicy::new();
        let mood = ListenMood::resting();
        assert!(policy
            .consider(0, true, true, ListenCue::ClauseFall, mood)
            .is_none());
        policy.note_voiced(0);
        assert!(policy
            .consider(FRAME_MS as u64, false, true, ListenCue::ClauseFall, mood)
            .is_none());
        assert!(policy
            .consider(FRAME_MS as u64, true, false, ListenCue::ClauseFall, mood)
            .is_none());
        for cue in [ListenCue::None, ListenCue::QuestionRise, ListenCue::Filler] {
            assert!(policy
                .consider(FRAME_MS as u64, true, true, cue, mood)
                .is_none());
        }
        let nod = policy
            .consider(FRAME_MS as u64, true, true, ListenCue::ClauseFall, mood)
            .expect("first nod");
        assert_eq!(nod.frames_after_fall, 1);
        assert_eq!(nod.kind, BackchannelKind::MmHm);
        assert!(nod.energy <= 0.55);
        policy.note_voiced(2_000);
        assert!(
            policy
                .consider(2_080, true, true, ListenCue::ClauseFall, mood)
                .is_none(),
            "inside the 4.8 s gap"
        );
        policy.note_voiced(FRAME_MS as u64 + u64::from(BACKCHANNEL_GAP_MS));
        let again = policy
            .consider(
                policy.last_voiced_ms.unwrap() + u64::from(FRAME_MS),
                true,
                true,
                ListenCue::ClauseFall,
                mood,
            )
            .expect("after the gap");
        assert_eq!(again.frames_after_fall, 1);

        policy.enabled = false;
        policy.note_voiced(20_000);
        assert!(policy
            .consider(20_080, true, true, ListenCue::ClauseFall, mood)
            .is_none());
        policy.enabled = true;
        policy.set_persona("aria");
        assert!(policy
            .consider(20_080, true, true, ListenCue::ClauseFall, mood)
            .is_none());
    }
}
