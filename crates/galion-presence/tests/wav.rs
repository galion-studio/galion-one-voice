// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! WAV/PCM loader: committed fixtures, spec integration, format coverage.
use galion_presence::backend::{MockBackend, MockConfig};
use galion_presence::spec::{parse_presence_at, YouState};
use galion_presence::wav::{encode_pcm16, parse_wav, read_wav, you_states, Gate};
use galion_presence::{run, Verdict};
use std::path::{Path, PathBuf};

const RATE: u32 = 8_000;
const FL: usize = 640; // 80 ms at 8 kHz

fn specs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs")
}

/// Seeded LCG noise in [-1, 1] (deterministic, no RNG crate).
struct Lcg(u32);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// state per frame -> mono f32 signal (speech = 0.3 sine 220 Hz, breath = 0.01 noise, silence = 0).
fn synth(states: &[(u32, u32, YouState)], frames: u32, seed: u32) -> Vec<f32> {
    let mut rng = Lcg(seed);
    let mut v = vec![0.0f32; frames as usize * FL];
    for (i, s) in v.iter_mut().enumerate() {
        let f = (i / FL) as u32;
        let st = states.iter().find(|(a, b, _)| *a <= f && f < *b).map(|x| x.2).unwrap_or(YouState::Silence);
        let n = rng.next();
        *s = match st {
            YouState::Speech => 0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / RATE as f32).sin(),
            YouState::Breath => 0.01 * n,
            YouState::Silence => 0.0,
        };
    }
    v
}

fn q(x: f32) -> i16 {
    (x * 32_767.0).round().clamp(-32_768.0, 32_767.0) as i16
}

fn p7_bytes() -> Vec<u8> {
    let s = synth(&[(5, 20, YouState::Speech), (20, 23, YouState::Breath)], 40, 7);
    encode_pcm16(RATE, 1, &s.iter().map(|x| q(*x)).collect::<Vec<_>>())
}

fn p8_bytes() -> Vec<u8> {
    let l = synth(&[(12, 20, YouState::Speech)], 40, 8);
    let mut inter = Vec::with_capacity(l.len() * 2);
    for x in l {
        inter.push(q(x));
        inter.push(0);
    }
    encode_pcm16(RATE, 2, &inter)
}

#[test]
#[ignore = "writes specs/fixtures/*.wav; run with --ignored to regenerate"]
fn regenerate_fixtures() {
    let d = specs_dir().join("fixtures");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("p7-turn-mono-8k16.wav"), p7_bytes()).unwrap();
    std::fs::write(d.join("p8-barge-stereo-8k16.wav"), p8_bytes()).unwrap();
}

#[test]
fn committed_fixtures_decode_to_expected_states() {
    let d = specs_dir().join("fixtures");
    let p7 = read_wav(&d.join("p7-turn-mono-8k16.wav")).expect("p7 fixture");
    assert_eq!((p7.sample_rate, p7.channels, p7.bits_per_sample, p7.frames(80).unwrap()), (8_000, 1, 16, 40));
    let st = you_states(&p7, 80, Gate::default()).unwrap();
    for (t, s) in st.iter().enumerate() {
        let want = match t {
            5..=19 => YouState::Speech,
            20..=22 => YouState::Breath,
            _ => YouState::Silence,
        };
        assert_eq!(*s, want, "p7 frame {t}");
    }
    let p8 = read_wav(&d.join("p8-barge-stereo-8k16.wav")).expect("p8 fixture");
    assert_eq!((p8.channels, p8.samples.len()), (2, 40 * FL));
    let st = you_states(&p8, 80, Gate::default()).unwrap();
    assert!(st.iter().enumerate().all(|(t, s)| (*s == YouState::Speech) == (12..20).contains(&t)));
}

#[test]
fn wav_spec_is_green_on_mock_and_matches_frame_spec() {
    let src = std::fs::read_to_string(specs_dir().join("presence-wav.json")).unwrap();
    let f = parse_presence_at(&src, &specs_dir()).expect("presence-wav.json must validate");
    assert_eq!(f.specs.len(), 2);
    assert_eq!(f.specs[0].frames, 40, "frames derived from audio length");
    let r = run(&f, &mut MockBackend::new(MockConfig::default()));
    assert_eq!(r.verdict, Verdict::Green, "{}", r.summary());
    assert_eq!(r.specs[0].metrics.onset_ms, Some(240.0)); // same as frame-scripted P2
    assert_eq!(r.specs[1].metrics.barge_in_ms, Some(80.0)); // same as frame-scripted P1
    let r2 = run(&f, &mut MockBackend::new(MockConfig::default()));
    assert_eq!(r.digest(), r2.digest());
    assert_eq!(r.digest(), "db7541ec9f6f42c7", "wav golden changed (fixtures or loader):\n{}", r.summary());
    // slow yield must still fail barge-in when driven from audio
    let slow = run(&f, &mut MockBackend::new(MockConfig { yield_frames: 4, ..Default::default() }));
    assert_eq!(slow.verdict, Verdict::Red);
}

#[test]
fn wav_spec_validation_errors() {
    let src = std::fs::read_to_string(specs_dir().join("presence-wav.json")).unwrap();
    let both = src.replacen("\"you_wav\": \"fixtures/p7", "\"you\": [{\"from\": 1, \"to\": 2, \"state\": \"speech\"}], \"you_wav\": \"fixtures/p7", 1);
    assert!(parse_presence_at(&both, &specs_dir()).unwrap_err().contains("mutually exclusive"));
    let missing = src.replacen("fixtures/p7-turn-mono-8k16.wav", "fixtures/nope.wav", 1);
    assert!(parse_presence_at(&missing, &specs_dir()).unwrap_err().contains("cannot read"));
    let bad_gate = src.replacen("\"speech_dbfs\": -35", "\"speech_dbfs\": -60", 1);
    assert!(parse_presence_at(&bad_gate, &specs_dir()).unwrap_err().contains("breath_dbfs < speech_dbfs"));
}

#[test]
fn decodes_8_24_32_bit_and_extensible() {
    fn wav(tag: u16, ch: u16, bits: u16, data: &[u8], ext: bool) -> Vec<u8> {
        let fmt_len: u32 = if ext { 40 } else { 16 };
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&(4 + 8 + fmt_len + 8 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&fmt_len.to_le_bytes());
        v.extend_from_slice(&tag.to_le_bytes());
        v.extend_from_slice(&ch.to_le_bytes());
        v.extend_from_slice(&16_000u32.to_le_bytes());
        v.extend_from_slice(&(16_000 * ch as u32 * bits as u32 / 8).to_le_bytes());
        v.extend_from_slice(&(ch * bits / 8).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        if ext {
            v.extend_from_slice(&22u16.to_le_bytes());
            v.extend_from_slice(&bits.to_le_bytes());
            v.extend_from_slice(&0u32.to_le_bytes());
            v.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xAA, 0, 0x38, 0x9B, 0x71]);
        }
        v.extend_from_slice(b"LIST\x02\x00\x00\x00ab"); // unknown chunk skipped
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }
    assert_eq!(parse_wav(&wav(1, 1, 8, &[128, 192, 0], false)).unwrap().samples, vec![0.0, 0.5, -1.0]);
    // 24-bit: 0x400000 = +0.5, 0xC00000 = -0.5
    assert_eq!(parse_wav(&wav(1, 1, 24, &[0, 0, 0x40, 0, 0, 0xC0], false)).unwrap().samples, vec![0.5, -0.5]);
    assert_eq!(parse_wav(&wav(1, 1, 32, &[0, 0, 0, 0x40], false)).unwrap().samples, vec![0.5]);
    let e = parse_wav(&wav(0xFFFE, 2, 16, &[0, 0x40, 0, 0x40], true)).unwrap();
    assert_eq!((e.channels, e.samples.clone()), (2, vec![0.5]));
    assert!(parse_wav(&wav(3, 1, 32, &[0; 4], false)).is_err(), "float rejected");
}
