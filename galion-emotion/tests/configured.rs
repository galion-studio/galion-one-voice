// Galion Voice — Perpetual Open MIT License
//! Built-in profiles are selected by id. Outputs stay the bits captured on
//! main before profile fields replaced persona-name branches.
use galion_emotion::persona_mood::{
    configured, configured_from_env, Persona, PersonaMood, CONDITIONING_DIM, CONDITIONING_VERSION,
    DEV_PERSONA_ENV, UnknownPersona,
};
use galion_emotion::{EmotionFrame, Feeling, Manner, ObsSource, UserObservation};

const OBS: &[(f32, f32, f32, f32)] = &[
    (0.0, 0.3, 0.5, 0.0),
    (0.9, 0.85, 0.5, 0.9),
    (-0.7, 0.3, 0.4, 0.9),
    (-0.9, 0.95, 0.9, 0.9),
    (0.8, 0.4, 0.5, 0.9),
    (-0.6, 0.2, 0.4, 1.0),
    (0.2, 0.55, 0.5, 0.4),
    (f32::NAN, f32::INFINITY, -2.0, 2.0),
];

const AT_MS: &[u64] = &[1_000, 1_500, 2_000, 5_000, 12_000, 20_000, 40_000, 80_000];

type CallerVad = Option<(f32, f32, f32, f32)>;

const FRAMES: &[(&str, CallerVad)] = &[
    ("none", None),
    ("joy", Some((0.8, 0.4, 0.5, 0.9))),
    ("excited", Some((0.8, 0.85, 0.5, 0.9))),
    ("tired", Some((-0.6, 0.2, 0.4, 1.0))),
    ("angry", Some((-0.8, 0.9, 0.8, 0.9))),
    ("calm", Some((0.1, 0.1, 0.5, 0.9))),
    ("low", Some((0.9, 0.9, 0.5, 0.1))),
    ("nan", Some((f32::NAN, f32::NAN, f32::NAN, f32::NAN))),
];

#[test]
fn configured_resolves_every_built_in_id_case_insensitively() {
    for persona in Persona::ALL {
        let id = persona.as_str();
        let upper = id.to_ascii_uppercase();
        let mixed: String = id
            .chars()
            .enumerate()
            .map(|(i, c)| if i % 2 == 0 { c.to_ascii_uppercase() } else { c })
            .collect();
        let padded = format!("  {mixed}  ");
        for candidate in [id.to_string(), upper, mixed, padded] {
            let got = configured(Some(&candidate)).unwrap_or_else(|_| panic!("resolve {candidate}"));
            assert_eq!(got.persona, persona, "{candidate}");
            assert!(std::ptr::eq(got, persona.profile()), "{candidate}");
            assert_eq!(Persona::parse(&candidate), Some(persona), "{candidate}");
        }
    }
}

#[test]
fn unknown_or_blank_id_is_an_error_not_a_default() {
    for id in [Some("northwind"), Some(""), Some("   "), None] {
        assert_eq!(configured(id), Err(UnknownPersona), "{id:?}");
    }
}

#[test]
fn parse_has_one_name_source() {
    for persona in Persona::ALL {
        assert_eq!(Persona::parse(persona.as_str()), Some(persona));
    }
}

#[test]
fn configured_from_env_reads_galion_horbm_dev_persona() {
    assert_eq!(DEV_PERSONA_ENV, "GALION_HORBM_DEV_PERSONA");
    let raw = std::env::var(DEV_PERSONA_ENV).ok();
    assert_eq!(configured_from_env(), configured(raw.as_deref()));
}

#[test]
fn outputs_are_bit_identical_after_the_data_move() {
    for (slot, persona) in Persona::ALL.iter().copied().enumerate() {
        let profile = persona.profile();
        assert_eq!(profile.conditioning_slot, u8::try_from(slot).unwrap());
        assert_eq!(profile.voice_persona.as_str(), persona.as_str());
    }
    let got = capture();
    let expected = SNAPSHOT;
    assert_eq!(got.lines().count(), expected.lines().count());
    for (i, (left, right)) in got.lines().zip(expected.lines()).enumerate() {
        assert_eq!(left, right, "snapshot line {i}");
    }
}

fn capture() -> String {
    let mut out = String::new();
    out.push_str(&format!("v{CONDITIONING_VERSION} d{CONDITIONING_DIM}\n"));
    for persona in Persona::ALL {
        let mut mood = PersonaMood::new(persona);
        out.push_str(&sample_cond(persona, 0, &mood.conditioning(0)));
        for (i, &(v, a, d, c)) in OBS.iter().enumerate() {
            let t = AT_MS[i];
            mood.observe(&observation(v, a, d, c), t);
            out.push_str(&sample_cond(persona, t, &mood.conditioning(t)));
        }
        let last = *AT_MS.last().unwrap();
        out.push_str(&sample_cond(persona, last + 20_000, &mood.conditioning(last + 20_000)));
        out.push_str(&sample_cond(
            persona,
            last + 400_000,
            &mood.conditioning(last + 400_000),
        ));
        let voice = persona.profile().voice_persona;
        out.push_str(&sample_feel(persona, "rest", "none", 0, Feeling::rest(voice)));
        for (name, values) in FRAMES {
            let caller = values.map(|(v, a, d, c)| frame(v, a, d, c));
            out.push_str(&sample_feel(
                persona,
                "live",
                name,
                1,
                Feeling::live(voice, caller.as_ref()),
            ));
            out.push_str(&sample_feel(
                persona,
                "conduct",
                name,
                1,
                Feeling::conduct(voice, caller.as_ref(), true),
            ));
            out.push_str(&sample_feel(
                persona,
                "conduct",
                name,
                0,
                Feeling::conduct(voice, caller.as_ref(), false),
            ));
        }
    }
    out
}

fn observation(v: f32, a: f32, d: f32, c: f32) -> UserObservation {
    UserObservation {
        valence: v,
        arousal: a,
        dominance: d,
        confidence: c,
        source: ObsSource::Fused,
    }
}

fn frame(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
    EmotionFrame {
        seq: 1,
        t_ms: 80,
        dur_ms: 80,
        valence: v,
        arousal: a,
        dominance: d,
        confidence: c,
    }
}

fn hex(x: f32) -> String {
    format!("{:08x}", x.to_bits())
}

fn sample_cond(persona: Persona, t: u64, c: &galion_emotion::PersonaConditioning) -> String {
    let prosody = c.piper();
    let mut line = format!("cond {} {t}", persona.as_str());
    for x in c.to_array() {
        line.push(' ');
        line.push_str(&hex(x));
    }
    for x in [
        prosody.length_scale,
        prosody.noise_scale,
        prosody.noise_w,
        prosody.sentence_silence_s,
        prosody.pitch_shift_st,
        prosody.energy,
    ] {
        line.push(' ');
        line.push_str(&hex(x));
    }
    line.push(' ');
    line.push_str(&format!("{}", prosody.pre_pause_ms));
    line.push('\n');
    line
}

fn sample_feel(persona: Persona, op: &str, frame_name: &str, hub: u8, feeling: Feeling) -> String {
    let manner = match feeling.manner {
        Manner::Plain => "plain",
        Manner::Answered => "answered",
        Manner::Shared => "shared",
    };
    format!(
        "feel {} {op} {frame_name} hub{hub} persona={} e={} l={} c={} h={} pitch={} range={} rate={} energy={} warmth={} pause={} wash={} manner={} enabled={} gain={} tags={}\n",
        persona.as_str(),
        feeling.persona.as_str(),
        hex(feeling.e),
        hex(feeling.oklch.l),
        hex(feeling.oklch.c),
        hex(feeling.oklch.h),
        hex(feeling.prosody.pitch_shift_st),
        hex(feeling.prosody.pitch_range),
        hex(feeling.prosody.rate),
        hex(feeling.prosody.energy),
        hex(feeling.prosody.warmth),
        feeling.prosody.pre_pause_ms,
        hex(feeling.wash_amber),
        manner,
        u8::from(feeling.hub.enabled),
        hex(feeling.hub.gain),
        u8::from(feeling.tags),
    )
}

/// `f32::to_bits` (hex) and integer fields, captured from main before this change.
const SNAPSHOT: &str = r#"v1 d10
cond samantha 0 3f800000 00000000 3e800000 3eb33333 3ee66666 00000000 3e99999a 3f000000 00000000 00000000 3f800000 3f2ac083 3f4ccccd 3e4ccccd 3ecccccd 3f800000 0
cond samantha 1000 3f800000 00000000 3e800000 3eb33333 3ee66666 00000000 3e99999a 3f000000 00000000 00000000 3f800000 3f2ac083 3f4ccccd 3e4ccccd 3ecccccd 3f800000 0
cond samantha 1500 3f800000 00000000 3effde08 3effb3aa 3ee66666 3f666666 3f59999a 3f000000 3f666666 00000000 3f70b31b 3f2ac083 3f4ccccd 3e268c92 3f906c45 3f889b41 0
cond samantha 2000 3f800000 00000000 3e832b55 3ec5bbd5 3edf91c9 bf333333 3e99999a 3ecccccd 3f666666 3f666666 3f89aaa8 3f2ac083 3f4ccccd 3e43887c 3d21f5d0 3f5556e8 225
cond samantha 5000 3f800000 00000000 3da47420 3ebfa438 3ed94f2f bf666666 3f733333 3f666666 3f666666 3f666666 3f8a469e 3f2ac083 3f4ccccd 3e46944a be893f4e 3f54372a 225
cond samantha 12000 3f800000 00000000 3ec520b8 3ec071c1 3ee08390 3f4ccccd 3ecccccd 3f000000 3f666666 00000000 3f7d59e4 3f2ac083 3f4ccccd 3e462d86 3f2c4563 3f817d70 0
cond samantha 20000 3f800000 00000000 3e3d3645 3e98520e 3ede2fc1 bf19999a 3e4ccccd 3ecccccd 3f800000 3f800000 3f8f7cea 3f2ac083 3f4ccccd 3e5a3d60 bea3c3e7 3f47f632 250
cond samantha 40000 3f800000 00000000 3e681c2f 3ebdfc5b 3ee3764a 3e4ccccd 3f0ccccd 3f000000 3ecccccd 00000000 3f7dd7c5 3f2ac083 3f4ccccd 3e476839 3ed16ab1 3f8136a1 0
cond samantha 80000 3f800000 00000000 3e7cb713 3eac9e79 3ee5fefc 00000000 3e99999a 00000000 3f800000 00000000 3f80a879 3f2ac083 3f4ccccd 3e50172a 3ebbb1a9 3f7e84f0 0
cond samantha 100000 3f800000 00000000 3e7e5b8a 3eafe8d6 3ee632b1 00000000 3e99999a 00000000 3e3504f3 00000000 3f80543c 3f2ac083 3f4ccccd 3e4e71fc 3ec43f3c 3f7f4278 0
cond samantha 480000 3f800000 00000000 3e800000 3eb33333 3ee66666 00000000 3e99999a 00000000 267fffff 00000000 3f800000 3f2ac083 3f4ccccd 3e4ccccd 3ecccccd 3f800000 0
feel samantha rest none hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha live none hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct none hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct none hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha live joy hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha conduct joy hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha conduct joy hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha live excited hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha conduct excited hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha conduct excited hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=3df5c28f range=3f800000 rate=3f83d70a energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=shared enabled=0 gain=00000000 tags=1
feel samantha live tired hub1 persona=samantha e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=bf000000 range=3f6147ae rate=3f6b851f energy=3f4ccccd warmth=3f400000 pause=250 wash=3f000000 manner=answered enabled=0 gain=00000000 tags=1
feel samantha conduct tired hub1 persona=samantha e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=bf000000 range=3f6147ae rate=3f6b851f energy=3f4ccccd warmth=3f400000 pause=250 wash=3f000000 manner=answered enabled=0 gain=00000000 tags=1
feel samantha conduct tired hub0 persona=samantha e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=bf000000 range=3f6147ae rate=3f6b851f energy=3f4ccccd warmth=3f400000 pause=250 wash=3f000000 manner=answered enabled=0 gain=00000000 tags=1
feel samantha live angry hub1 persona=samantha e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bee66666 range=3f645a1d rate=3f6d9169 energy=3f51eb86 warmth=3f3cac08 pause=225 wash=3f03126f manner=answered enabled=0 gain=00000000 tags=1
feel samantha conduct angry hub1 persona=samantha e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bee66666 range=3f645a1d rate=3f6d9169 energy=3f51eb86 warmth=3f3cac08 pause=225 wash=3f03126f manner=answered enabled=0 gain=00000000 tags=1
feel samantha conduct angry hub0 persona=samantha e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bee66666 range=3f645a1d rate=3f6d9169 energy=3f51eb86 warmth=3f3cac08 pause=225 wash=3f03126f manner=answered enabled=0 gain=00000000 tags=1
feel samantha live calm hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct calm hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct calm hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha live low hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct low hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct low hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha live nan hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct nan hub1 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
feel samantha conduct nan hub0 persona=samantha e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=00000000 range=3f800000 rate=3f800000 energy=3f800000 warmth=3f1eb852 pause=0 wash=3f1eb852 manner=plain enabled=0 gain=00000000 tags=1
cond jarvis 0 00000000 3f800000 3dcccccd 3e8f5c29 3f1eb852 00000000 3e99999a 3f000000 00000000 00000000 3f86bbbc 3f000000 3f19999a 3e88f5c2 be978d50 3f683904 80
cond jarvis 1000 00000000 3f800000 3dcccccd 3e8f5c29 3f1eb852 00000000 3e99999a 3f000000 00000000 00000000 3f86bbbc 3f000000 3f19999a 3e88f5c2 be978d50 3f683904 80
cond jarvis 1500 00000000 3f800000 3e3dd4a5 3e9d3404 3f1eb852 3f666666 3f59999a 3f000000 3f666666 00000000 3f86314d 3f000000 3f19999a 3e857fcc be647f1c 3f697f0e 80
cond jarvis 2000 00000000 3f800000 3e0531f6 3e92fc76 3f20ff31 bf333333 3e99999a 3ecccccd 3f666666 3f666666 3f8b3320 3f000000 3f19999a 3e880daf beb97e15 3f5f74d0 116
cond jarvis 5000 00000000 3f800000 3d9ce089 3e8e5289 3f235e3a bf666666 3f733333 3f666666 3f666666 3f666666 3f8b61c2 3f000000 3f19999a 3e89382a becda688 3f5f0b45 116
cond jarvis 12000 00000000 3f800000 3e26b3cf 3e91beff 3f211623 3f4ccccd 3ecccccd 3f000000 3f666666 00000000 3f86a3e0 3f000000 3f19999a 3e885d0d be825a39 3f687137 80
cond jarvis 20000 00000000 3f800000 3dcc1f37 3e895508 3f21c212 bf19999a 3e4ccccd 3ecccccd 3f800000 3f800000 3f8c16bb 3f000000 3f19999a 3e8a778b becfad0d 3f5d987b 120
cond jarvis 40000 00000000 3f800000 3ddb3125 3e922212 3f1f8690 3e4ccccd 3f0ccccd 3f000000 3ecccccd 00000000 3f86a001 3f000000 3f19999a 3e884448 be932cbb 3f687a55 80
cond jarvis 80000 00000000 3f800000 3dcddef4 3e90140f 3f1ec7aa 00000000 3e99999a 00000000 3f800000 00000000 3f86b48d 3f000000 3f19999a 3e88c7c9 be96d112 3f6849f0 80
cond jarvis 100000 00000000 3f800000 3dcd2327 3e8f9616 3f1ebd27 00000000 3e99999a 00000000 3e3504f3 00000000 3f86b979 3f000000 3f19999a 3e88e747 be975205 3f683e59 80
cond jarvis 480000 00000000 3f800000 3dcccccd 3e8f5c29 3f1eb852 00000000 3e99999a 00000000 267fffff 00000000 3f86bbbc 3f000000 3f19999a 3e88f5c2 be978d50 3f683904 80
feel jarvis rest none hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis live none hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct none hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct none hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=0 gain=00000000 tags=0
feel jarvis live joy hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=1 gain=3d23d70a tags=0
feel jarvis conduct joy hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=1 gain=3d23d70a tags=0
feel jarvis conduct joy hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=0 gain=00000000 tags=0
feel jarvis live excited hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=1 gain=3d23d70a tags=0
feel jarvis conduct excited hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=1 gain=3d23d70a tags=0
feel jarvis conduct excited hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=shared enabled=0 gain=00000000 tags=0
feel jarvis live tired hub1 persona=jarvis e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=becccccd range=3f666666 rate=3f70a3d7 energy=3f6147ae warmth=3f0ccccd pause=120 wash=3f000000 manner=answered enabled=1 gain=3d23d70a tags=0
feel jarvis conduct tired hub1 persona=jarvis e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=becccccd range=3f666666 rate=3f70a3d7 energy=3f6147ae warmth=3f0ccccd pause=120 wash=3f000000 manner=answered enabled=1 gain=3d23d70a tags=0
feel jarvis conduct tired hub0 persona=jarvis e=3f800000 l=3f3851ec c=3db851ec h=439b0000 pitch=becccccd range=3f666666 rate=3f70a3d7 energy=3f6147ae warmth=3f0ccccd pause=120 wash=3f000000 manner=answered enabled=0 gain=00000000 tags=0
feel jarvis live angry hub1 persona=jarvis e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bec7ae14 range=3f66e978 rate=3f7126e9 energy=3f624dd3 warmth=3f0978d5 pause=116 wash=3ef4bc6a manner=answered enabled=1 gain=3d23d70a tags=0
feel jarvis conduct angry hub1 persona=jarvis e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bec7ae14 range=3f66e978 rate=3f7126e9 energy=3f624dd3 warmth=3f0978d5 pause=116 wash=3ef4bc6a manner=answered enabled=1 gain=3d23d70a tags=0
feel jarvis conduct angry hub0 persona=jarvis e=3f666666 l=3f3ebee0 c=3da67310 h=439af4fd pitch=bec7ae14 range=3f66e978 rate=3f7126e9 energy=3f624dd3 warmth=3f0978d5 pause=116 wash=3ef4bc6a manner=answered enabled=0 gain=00000000 tags=0
feel jarvis live calm hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct calm hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct calm hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=0 gain=00000000 tags=0
feel jarvis live low hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct low hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct low hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=0 gain=00000000 tags=0
feel jarvis live nan hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct nan hub1 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=1 gain=3d23d70a tags=0
feel jarvis conduct nan hub0 persona=jarvis e=00000000 l=3f789375 c=3b449ba6 h=438f0000 pitch=be99999a range=3f6b851f rate=3f75c28f energy=3f6b851f warmth=3ed70a3d pause=80 wash=3e8f5c29 manner=plain enabled=0 gain=00000000 tags=0
"#;
