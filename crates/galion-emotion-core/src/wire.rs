// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Consent and the 8-byte feeling band.
//!
//! Byte 7's low four bits are `galion-emotion` `consent_bits`: sense, adapt,
//! remember, share. This module calls `encode_with_consent` and then clears
//! the three-bit affect label. The open state does not fit in those three
//! bits. Universal scores live in [`crate::project`], not in the label.

use galion_emotion::band::consent_bits;
use galion_emotion::{encode_with_consent, Direction, EmotionFrame, EmotionWire};

use crate::axes::{AROUSAL, DOMINANCE, VALENCE};
use crate::clock::FRAME_MS;
use crate::intent::{IntentMsg, IntentSource};
use crate::moment::MomentState;

/// Inbound pull enters the moment only when the caller granted both bits.
/// Her own intent does not need the caller's consent.
pub fn admit_pull(msg: &IntentMsg) -> bool {
    match msg.source {
        IntentSource::Own => true,
        IntentSource::Inbound => {
            let bits = msg.consent & 0x0f;
            bits & consent_bits::SENSE != 0 && bits & consent_bits::ADAPT != 0
        }
    }
}

pub fn may_remember(consent: u8) -> bool {
    consent & consent_bits::REMEMBER != 0
}

pub fn may_share(consent: u8) -> bool {
    consent & consent_bits::SHARE != 0
}

/// Outbound sample of the moment. Affect-label bits stay 0.
pub fn local_outbound(moment: &MomentState, consent: u8, seq: u16) -> EmotionWire {
    let frame = EmotionFrame {
        seq: moment.tick,
        t_ms: moment.t_ms,
        dur_ms: FRAME_MS as u16,
        valence: moment.pos[VALENCE],
        arousal: moment.pos[AROUSAL],
        dominance: moment.pos[DOMINANCE],
        confidence: 1.0,
    };
    let mut wire = encode_with_consent(Direction::Outbound, seq, Some(&frame), consent & 0x0f);
    wire[2] &= !(0b111 << 2);
    wire
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::VALENCE;
    use crate::intent::IntentMsg;
    use galion_emotion::band::decode;
    use galion_emotion::{consent_bits_of, encode_with_consent};

    #[test]
    fn sense_and_adapt_gate_inbound_pull_only() {
        let mut inbound = IntentMsg::hold(0);
        inbound.source = IntentSource::Inbound;
        inbound.consent = 0;
        assert!(!admit_pull(&inbound));
        inbound.consent = consent_bits::SENSE;
        assert!(!admit_pull(&inbound));
        inbound.consent = consent_bits::ADAPT;
        assert!(!admit_pull(&inbound));
        inbound.consent = consent_bits::SENSE | consent_bits::ADAPT;
        assert!(admit_pull(&inbound));
        assert!(admit_pull(&IntentMsg::hold(0)));
    }

    #[test]
    fn outbound_wire_keeps_scalars_and_drops_the_affect_label() {
        let moment = MomentState {
            t_ms: 80,
            tick: 2,
            pos: [-0.4, 0.7, 0.3, 0.0],
            vel: [0.0; 4],
        };
        let bits = consent_bits::SHARE;
        let wire = local_outbound(&moment, bits | 0xF0, 9);
        let frame = EmotionFrame {
            seq: 2,
            t_ms: 80,
            dur_ms: 80,
            valence: moment.pos[VALENCE],
            arousal: moment.pos[AROUSAL],
            dominance: moment.pos[DOMINANCE],
            confidence: 1.0,
        };
        let raw = encode_with_consent(Direction::Outbound, 9, Some(&frame), bits);
        assert_eq!(&wire[3..7], &raw[3..7]);
        assert_eq!(consent_bits_of(&wire), bits);
        assert_eq!(wire[2] & 1, 1);
        assert_eq!(wire[2] & 0b10, 0b10);
        assert_eq!(wire[2] & (0b111 << 2), 0);
        let (_dir, seq, decoded) = decode(&wire, 80, 80);
        assert_eq!(seq, 9);
        let decoded = decoded.expect("present");
        assert!((decoded.valence - frame.valence).abs() < 1.0 / 127.0);
        assert!((decoded.arousal - frame.arousal).abs() < 1.0 / 255.0);
    }
}
