// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Intent message between the inner life and the MnE stack.
//!
//! Fixed 72 bytes, little-endian, magic `GEMI`, version 1. The verb is an
//! open token of at most 16 ASCII bytes. It is not an emotion category.
//! A pull is an offset from the speaker baseline, not a Cowen pin.

use crate::axes::{finite_in, AXES, PULL_LIMIT};
use crate::factors::{BreathPhase, ENERGY_REST, FATIGUE_REST, HEALTH_REST, VOLUME_REST};
use crate::CoreError;

pub const INTENT_BYTES: usize = 72;
pub const INTENT_MAGIC: [u8; 4] = *b"GEMI";
pub const INTENT_VERSION: u16 = 1;
pub const VERB_BYTES: usize = 16;

pub const MASK_ENERGY: u8 = 1 << 0;
pub const MASK_FATIGUE: u8 = 1 << 1;
pub const MASK_HEALTH: u8 = 1 << 2;
pub const MASK_BREATH: u8 = 1 << 3;
pub const FACTOR_MASK_ALL: u8 = MASK_ENERGY | MASK_FATIGUE | MASK_HEALTH | MASK_BREATH;
pub const PULL_MASK_ALL: u8 = (1 << AXES) - 1;

/// Who sent the message. Own is her inner life. Inbound is the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IntentSource {
    Own = 0,
    Inbound = 1,
}

impl IntentSource {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Own),
            1 => Some(Self::Inbound),
            _ => None,
        }
    }
}

/// One frame of intent. Copy, no heap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntentMsg {
    pub t_ms: u32,
    pub source: IntentSource,
    pub consent: u8,
    pub verb: [u8; VERB_BYTES],
    pub strength: f32,
    pub pull: [f32; AXES],
    pub pull_mask: u8,
    pub factor_mask: u8,
    pub breath_phase: BreathPhase,
    pub energy: f32,
    pub fatigue: f32,
    pub health: f32,
    pub lung_volume: f32,
    pub air_flow: f32,
}

impl IntentMsg {
    /// No pulls and no factor writes. Breath, if the core is stepping,
    /// follows the stand-in cycle.
    pub const fn hold(t_ms: u32) -> Self {
        Self {
            t_ms,
            source: IntentSource::Own,
            consent: 0,
            verb: verb_bytes(b"hold"),
            strength: 0.0,
            pull: [0.0; AXES],
            pull_mask: 0,
            factor_mask: 0,
            breath_phase: BreathPhase::Rest,
            energy: ENERGY_REST,
            fatigue: FATIGUE_REST,
            health: HEALTH_REST,
            lung_volume: VOLUME_REST,
            air_flow: 0.0,
        }
    }

    /// Rest factors, written every tick, so the breath cycle stays off.
    pub const fn steady(t_ms: u32) -> Self {
        let mut msg = Self::hold(t_ms);
        msg.factor_mask = FACTOR_MASK_ALL;
        msg
    }

    pub fn set_verb(&mut self, verb: &str) -> Result<(), CoreError> {
        self.verb = pack_verb(verb)?;
        Ok(())
    }

    pub fn verb_str(&self) -> &str {
        let end = self.verb.iter().position(|b| *b == 0).unwrap_or(VERB_BYTES);
        core::str::from_utf8(&self.verb[..end]).unwrap_or("")
    }

    pub fn set_pull(&mut self, axis: usize, value: f32) -> Result<(), CoreError> {
        if axis >= AXES {
            return Err(CoreError::BadIntent);
        }
        self.pull[axis] = value;
        self.pull_mask |= 1 << axis;
        Ok(())
    }

    pub fn encode(&self) -> Result<[u8; INTENT_BYTES], CoreError> {
        validate(self)?;
        let mut out = [0u8; INTENT_BYTES];
        out[0..4].copy_from_slice(&INTENT_MAGIC);
        out[4..6].copy_from_slice(&INTENT_VERSION.to_le_bytes());
        out[6] = self.source as u8;
        out[7] = self.consent;
        out[8..12].copy_from_slice(&self.t_ms.to_le_bytes());
        out[12..28].copy_from_slice(&self.verb);
        write_f32(&mut out, 28, self.strength);
        for (i, pull) in self.pull.iter().enumerate() {
            write_f32(&mut out, 32 + i * 4, *pull);
        }
        out[48] = self.pull_mask;
        out[49] = self.factor_mask;
        out[50] = self.breath_phase as u8;
        out[51] = 0;
        write_f32(&mut out, 52, self.energy);
        write_f32(&mut out, 56, self.fatigue);
        write_f32(&mut out, 60, self.health);
        write_f32(&mut out, 64, self.lung_volume);
        write_f32(&mut out, 68, self.air_flow);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CoreError> {
        if bytes.len() != INTENT_BYTES {
            return Err(CoreError::BadIntent);
        }
        if bytes[0..4] != INTENT_MAGIC {
            return Err(CoreError::BadIntent);
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != INTENT_VERSION {
            return Err(CoreError::BadIntent);
        }
        let source = IntentSource::from_u8(bytes[6]).ok_or(CoreError::BadIntent)?;
        let mut verb = [0u8; VERB_BYTES];
        verb.copy_from_slice(&bytes[12..28]);
        let mut pull = [0.0; AXES];
        for (i, slot) in pull.iter_mut().enumerate() {
            *slot = read_f32(bytes, 32 + i * 4)?;
        }
        let breath_phase = BreathPhase::from_u8(bytes[50]).ok_or(CoreError::BadIntent)?;
        if bytes[51] != 0 {
            return Err(CoreError::BadIntent);
        }
        let msg = Self {
            t_ms: u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
            source,
            consent: bytes[7],
            verb,
            strength: read_f32(bytes, 28)?,
            pull,
            pull_mask: bytes[48],
            factor_mask: bytes[49],
            breath_phase,
            energy: read_f32(bytes, 52)?,
            fatigue: read_f32(bytes, 56)?,
            health: read_f32(bytes, 60)?,
            lung_volume: read_f32(bytes, 64)?,
            air_flow: read_f32(bytes, 68)?,
        };
        validate(&msg)?;
        Ok(msg)
    }
}

pub fn pack_verb(verb: &str) -> Result<[u8; VERB_BYTES], CoreError> {
    if verb.len() > VERB_BYTES || !verb_ok(verb.as_bytes()) {
        return Err(CoreError::BadIntent);
    }
    let mut out = [0u8; VERB_BYTES];
    out[..verb.len()].copy_from_slice(verb.as_bytes());
    Ok(out)
}

pub fn validate(msg: &IntentMsg) -> Result<(), CoreError> {
    if msg.consent & 0xF0 != 0 {
        return Err(CoreError::BadIntent);
    }
    if !verb_shape(&msg.verb) {
        return Err(CoreError::BadIntent);
    }
    if !finite_in(msg.strength, 0.0, 1.0) {
        return Err(CoreError::BadIntent);
    }
    if msg.pull_mask & !PULL_MASK_ALL != 0 || msg.factor_mask & !FACTOR_MASK_ALL != 0 {
        return Err(CoreError::BadIntent);
    }
    for v in msg.pull {
        if !finite_in(v, -PULL_LIMIT, PULL_LIMIT) {
            return Err(CoreError::BadIntent);
        }
    }
    if !finite_in(msg.energy, 0.0, 1.0)
        || !finite_in(msg.fatigue, 0.0, 1.0)
        || !finite_in(msg.health, 0.0, 1.0)
        || !finite_in(msg.lung_volume, 0.0, 1.0)
        || !finite_in(msg.air_flow, -1.0, 1.0)
    {
        return Err(CoreError::BadIntent);
    }
    Ok(())
}

const fn verb_bytes(bytes: &[u8]) -> [u8; VERB_BYTES] {
    let mut out = [0u8; VERB_BYTES];
    let mut i = 0;
    while i < bytes.len() && i < VERB_BYTES {
        out[i] = bytes[i];
        i += 1;
    }
    out
}

fn verb_ok(bytes: &[u8]) -> bool {
    bytes.iter().all(|b| (0x20..=0x7e).contains(b))
}

fn verb_shape(bytes: &[u8; VERB_BYTES]) -> bool {
    let mut seen_zero = false;
    for b in bytes {
        if seen_zero {
            if *b != 0 {
                return false;
            }
        } else if *b == 0 {
            seen_zero = true;
        } else if !(0x20..=0x7e).contains(b) {
            return false;
        }
    }
    true
}

fn write_f32(out: &mut [u8; INTENT_BYTES], at: usize, v: f32) {
    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn read_f32(bytes: &[u8], at: usize) -> Result<f32, CoreError> {
    let v = f32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    if v.is_finite() {
        Ok(v)
    } else {
        Err(CoreError::BadIntent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::VALENCE;

    #[test]
    fn intent_roundtrip_keeps_every_byte() {
        let mut msg = IntentMsg::hold(80);
        msg.source = IntentSource::Inbound;
        msg.consent = 0b0110;
        msg.set_verb("settle").unwrap();
        msg.strength = 0.4;
        msg.set_pull(VALENCE, -0.25).unwrap();
        msg.factor_mask = MASK_FATIGUE | MASK_BREATH;
        msg.breath_phase = BreathPhase::Exhale;
        msg.fatigue = 0.84;
        msg.lung_volume = 0.2;
        msg.air_flow = -0.5;
        let bytes = msg.encode().unwrap();
        assert_eq!(bytes.len(), INTENT_BYTES);
        assert_eq!(&bytes[0..4], b"GEMI");
        let back = IntentMsg::decode(&bytes).unwrap();
        assert_eq!(back, msg);
        assert_eq!(back.encode().unwrap(), bytes);
        assert_eq!(back.verb_str(), "settle");
    }

    #[test]
    fn a_short_buffer_and_a_bad_magic_are_refused() {
        let msg = IntentMsg::steady(0);
        let bytes = msg.encode().unwrap();
        assert!(IntentMsg::decode(&bytes[..71]).is_err());
        let mut bad = bytes;
        bad[0] = b'X';
        assert!(IntentMsg::decode(&bad).is_err());
    }
}
