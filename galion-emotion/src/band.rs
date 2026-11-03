//! The duplex emotion band: a third stream next to "caller audio" and
//! "Galion audio". One 8-byte sample per audio frame (e.g. every 80 ms at
//! 12.5 Hz, matching Mimi-style codec frame rate) = 100 B/s per direction.
//!
//! Wire layout (`EmotionWire`, little-endian, 8 bytes):
//! ```text
//! [0..2) seq   u16 (wrapping)
//! [2]    flags u8  bit0=present, bit1=direction(0=in,1=out), bits2..4=Affect label
//! [3]    valence   i8  (-127..127  <-> -1..1)
//! [4]    arousal   u8  (0..255     <->  0..1)
//! [5]    dominance u8
//! [6]    confidence u8
//! [7]    consent bits (A4 eureka): bit0=sense bit1=adapt bit2=remember bit3=share; bits4..7 reserved
//! ```
//! `present = 0` means "no consent / unknown" — explicit absence, never a guess.

use crate::frame::EmotionFrame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Caller -> Galion (detected affect).
    Inbound,
    /// Galion -> caller (expressed affect; what Galion intended to convey).
    Outbound,
}

pub type EmotionWire = [u8; 8];

/// In-band consent bits, carried in every frame (A4: `sense / adapt / remember / share`).
pub mod consent_bits {
    /// May Galion infer affect from the caller at all?
    pub const SENSE: u8 = 1 << 0;
    /// May inferred affect steer Galion's prosody?
    pub const ADAPT: u8 = 1 << 1;
    /// May an affect baseline persist past the session? Default OFF.
    pub const REMEMBER: u8 = 1 << 2;
    /// May the E stream leave the voice runtime (Galion OS / Hermes / Galion One / Open Mind)?
    pub const SHARE: u8 = 1 << 3;
}

/// Encode and stamp consent bits into byte 7.
pub fn encode_with_consent(dir: Direction, seq: u16, frame: Option<&EmotionFrame>, bits: u8) -> EmotionWire {
    // Without SENSE, an inbound frame must not be carried at all.
    let frame = if dir == Direction::Inbound && bits & consent_bits::SENSE == 0 { None } else { frame };
    let mut w = encode(dir, seq, frame);
    w[7] = bits & 0x0f;
    w
}

pub fn consent_bits_of(w: &EmotionWire) -> u8 {
    w[7] & 0x0f
}

pub fn encode(dir: Direction, seq: u16, frame: Option<&EmotionFrame>) -> EmotionWire {
    let mut w = [0u8; 8];
    w[0..2].copy_from_slice(&seq.to_le_bytes());
    let dbit = if dir == Direction::Outbound { 0b10 } else { 0 };
    match frame {
        None => {
            w[2] = dbit;
        }
        Some(f) => {
            let f = f.clamped();
            w[2] = 0b1 | dbit | ((f.label() as u8 & 0b111) << 2);
            w[3] = (f.valence * 127.0).round() as i8 as u8;
            w[4] = (f.arousal * 255.0).round() as u8;
            w[5] = (f.dominance * 255.0).round() as u8;
            w[6] = (f.confidence * 255.0).round() as u8;
        }
    }
    w
}

/// Returns (direction, seq, frame-or-absent). `t_ms`/`dur_ms` are supplied by
/// the carrying audio frame, not the wire.
pub fn decode(w: &EmotionWire, t_ms: u32, dur_ms: u16) -> (Direction, u16, Option<EmotionFrame>) {
    let seq = u16::from_le_bytes([w[0], w[1]]);
    let dir = if w[2] & 0b10 != 0 { Direction::Outbound } else { Direction::Inbound };
    if w[2] & 1 == 0 {
        return (dir, seq, None);
    }
    let f = EmotionFrame {
        seq: seq as u32,
        t_ms,
        dur_ms,
        valence: (w[3] as i8) as f32 / 127.0,
        arousal: w[4] as f32 / 255.0,
        dominance: w[5] as f32 / 255.0,
        confidence: w[6] as f32 / 255.0,
    };
    (dir, seq, Some(f.clamped()))
}

/// One frame of the duplex stream: audio + emotion, same clock.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplexPacket {
    pub t_ms: u32,
    pub dur_ms: u16,
    /// Opaque audio payload (codec tokens or PCM bytes) — not interpreted here.
    pub audio: Vec<u8>,
    pub emotion: EmotionWire,
}

/// Per-direction band state: sequencing + smoothing + bounded history
/// (in RAM only, for the live call; cleared on reset — GDPR minimisation).
#[derive(Debug)]
pub struct EmotionBand {
    pub dir: Direction,
    seq: u16,
    smoothed: Option<EmotionFrame>,
    history: Vec<EmotionWire>,
    cap: usize,
    alpha: f32,
}

impl EmotionBand {
    pub fn new(dir: Direction, history_cap: usize, alpha: f32) -> Self {
        Self { dir, seq: 0, smoothed: None, history: Vec::with_capacity(history_cap), cap: history_cap, alpha }
    }

    /// Attach the (optional) emotion frame to an audio frame, producing a packet.
    pub fn push(&mut self, t_ms: u32, dur_ms: u16, audio: Vec<u8>, frame: Option<EmotionFrame>) -> DuplexPacket {
        let out = match (frame, self.smoothed) {
            (Some(f), Some(prev)) => Some(prev.ema(&f, self.alpha)),
            (Some(f), None) => Some(f.clamped()),
            (None, _) => None,
        };
        // Absence resets smoothing: never "remember" emotion across a consent gap.
        self.smoothed = out;
        let wire = encode(self.dir, self.seq, out.as_ref());
        self.seq = self.seq.wrapping_add(1);
        if self.cap > 0 {
            if self.history.len() == self.cap {
                self.history.remove(0);
            }
            self.history.push(wire);
        }
        DuplexPacket { t_ms, dur_ms, audio, emotion: wire }
    }

    pub fn current(&self) -> Option<EmotionFrame> {
        self.smoothed
    }

    pub fn history(&self) -> &[EmotionWire] {
        &self.history
    }

    /// Wipe everything (hang-up or consent withdrawal).
    pub fn reset(&mut self) {
        self.smoothed = None;
        self.history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f32, a: f32) -> EmotionFrame {
        EmotionFrame { seq: 0, t_ms: 0, dur_ms: 80, valence: v, arousal: a, dominance: 0.5, confidence: 0.8 }
    }

    #[test]
    fn wire_roundtrip_within_quantization() {
        let x = f(-0.42, 0.77);
        let w = encode(Direction::Outbound, 513, Some(&x));
        let (d, s, y) = decode(&w, 0, 80);
        let y = y.unwrap();
        assert_eq!(d, Direction::Outbound);
        assert_eq!(s, 513);
        assert!((y.valence - x.valence).abs() < 1.0 / 127.0);
        assert!((y.arousal - x.arousal).abs() < 1.0 / 255.0);
        assert!((y.confidence - x.confidence).abs() < 1.0 / 255.0);
        assert_eq!(w.len(), 8);
    }

    #[test]
    fn absent_is_explicit() {
        let w = encode(Direction::Inbound, 7, None);
        let (d, s, y) = decode(&w, 0, 80);
        assert_eq!((d, s, y), (Direction::Inbound, 7, None));
        assert_eq!(&w[3..], &[0u8; 5]);
    }

    #[test]
    fn extremes_roundtrip() {
        for v in [-1.0f32, 1.0] {
            let w = encode(Direction::Inbound, 0, Some(&f(v, 1.0)));
            let y = decode(&w, 0, 80).2.unwrap();
            assert!((y.valence - v).abs() < 1e-6);
            assert!((y.arousal - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn band_sequences_smooths_and_bounds_history() {
        let mut b = EmotionBand::new(Direction::Inbound, 3, 0.5);
        let p0 = b.push(0, 80, vec![1, 2], Some(f(0.0, 0.0)));
        let p1 = b.push(80, 80, vec![3], Some(f(1.0, 1.0)));
        assert_eq!(decode(&p0.emotion, 0, 80).1, 0);
        assert_eq!(decode(&p1.emotion, 80, 80).1, 1);
        let cur = b.current().unwrap();
        assert!((cur.valence - 0.5).abs() < 1e-6);
        for i in 0..5 {
            b.push(160 + i * 80, 80, vec![], Some(f(0.0, 0.5)));
        }
        assert_eq!(b.history().len(), 3);
        assert_eq!(p1.audio, vec![3]);
    }

    #[test]
    fn consent_gap_resets_smoothing_and_reset_wipes() {
        let mut b = EmotionBand::new(Direction::Inbound, 8, 0.5);
        b.push(0, 80, vec![], Some(f(1.0, 1.0)));
        b.push(80, 80, vec![], None);
        assert!(b.current().is_none());
        let p = b.push(160, 80, vec![], Some(f(-1.0, 0.0)));
        let y = decode(&p.emotion, 160, 80).2.unwrap();
        assert!((y.valence + 1.0).abs() < 1e-6, "no carry-over across gap");
        b.reset();
        assert!(b.history().is_empty() && b.current().is_none());
    }

    #[test]
    fn consent_bits_in_band() {
        use super::consent_bits::*;
        let x = f(0.5, 0.5);
        let w = encode_with_consent(Direction::Inbound, 1, Some(&x), SENSE | ADAPT);
        assert_eq!(consent_bits_of(&w), SENSE | ADAPT);
        assert!(decode(&w, 0, 80).2.is_some());
        let w2 = encode_with_consent(Direction::Inbound, 2, Some(&x), ADAPT | SHARE);
        assert!(decode(&w2, 0, 80).2.is_none(), "no SENSE -> inbound affect dropped");
        let w3 = encode_with_consent(Direction::Outbound, 3, Some(&x), 0);
        assert!(decode(&w3, 0, 80).2.is_some(), "Galion's own expression needs no caller consent");
        assert_eq!(consent_bits_of(&encode_with_consent(Direction::Outbound, 0, None, 0xff)), 0x0f);
    }

    #[test]
    fn full_duplex_loop() {
        use crate::consent::{DefaultConsentGate, Purpose};
        use crate::detect::{AffectDetector, EnergyStubDetector};
        use crate::express::StubExpressor;

        let mut gate = DefaultConsentGate::default();
        gate.grant(0, "hr", Purpose::PersonalAssistant);
        let mut det = EnergyStubDetector::default();
        let exp = StubExpressor;
        let mut inb = EmotionBand::new(Direction::Inbound, 16, 0.5);
        let mut outb = EmotionBand::new(Direction::Outbound, 16, 0.5);

        let pcm: Vec<f32> = (0..1920).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect();
        let caller = det.detect(&pcm, 24_000, 0, &gate, Purpose::PersonalAssistant).ok();
        let pin = inb.push(0, 80, vec![0; 4], caller);
        let voiced = exp.voice_color(inb.current().as_ref());
        let ctrl = voiced.prosody;
        let pout = outb.push(0, 80, vec![0; 4], Some(voiced.target));
        assert_eq!(decode(&pin.emotion, 0, 80).0, Direction::Inbound);
        assert_eq!(decode(&pout.emotion, 0, 80).0, Direction::Outbound);
        assert!(ctrl.energy > 0.0);

        // Withdraw -> inbound band goes absent, Galion falls back to neutral-warm.
        gate.withdraw(1);
        let caller2 = det.detect(&pcm, 24_000, 80, &gate, Purpose::PersonalAssistant).ok();
        assert!(caller2.is_none());
        inb.reset();
        let p2 = inb.push(80, 80, vec![], caller2);
        assert!(decode(&p2.emotion, 80, 80).2.is_none());
        assert!(exp.voice_color(inb.current().as_ref()).target.valence > 0.0);
    }
}
