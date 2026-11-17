// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Compact little-endian encoding of a [`crate::Document`].
//!
//! ```text
//! magic            4 bytes  "GMN1"
//! kind             u8       1 intent, 2 read-back
//! value_origin     u8       0 stand_in, 1 config, 2 measured
//! frame_ms         u16
//! schema_version   u16
//! n_frames         u16
//! n_strings        u16
//! strings          repeated: u16 byte_len, utf-8 bytes
//! session          11 × u16 string index:
//!                  note, persona_id, voice_id, moment_id, memory_id,
//!                  memory_snippet, place, distance, noise, listener, text
//! baseline         3 × f64  arousal, valence, dominance
//! frames           n_frames, layout below
//! ```
//!
//! Feeling inside a frame:
//! ```text
//! arousal, valence, dominance   f64
//! pin_present                   u8     0 or 1
//! track                         u8     0 none, 1 prosody, 2 burst
//! pin_index                     u8     0..23 when present, else 0
//! weight                        f64    0 when no pin
//! ```
//!
//! Intent frame after the feeling: verb u16, strength f64, energy f64,
//! fatigue f64, health f64, t_ms u32, start_byte u32, end_byte u32.
//!
//! Read-back frame after the feeling: t_ms u32, then
//! ```text
//! body_origin   u8     0 unmeasured, 1 stand_in, 2 measured
//! segment       u8     0 unknown, 1 rest, 2 inhale, 3 hold, 4 speech, 5 exhale
//! lung_flags    u8     bit0 volume, bit1 pressure, bit2 loudness_db
//! f64 × popcount(lung_flags), in that bit order
//! throat_flags  u8     bit0 f0_hz, bit1 hnr_db
//! mouth_flags   u8     bit0 f1, bit1 f2, bit2 f3
//! nose_flags    u8     bit0 nasalance
//! confidence    2 × f64 feeling, body
//! confounded    u8     0 or 1
//! ```
//!
//! Floats are IEEE-754 binary64, little-endian, and must be finite.
//! The same document encodes to the same bytes. JSON and TOML are a
//! lossless mirror of the document, not of these bytes.

use crate::model::{
    BaselinePoint, BodyOrigin, BodyState, BreathSegment, Confidence, DocKind, Document, Feeling,
    InnerFactors, Intent, IntentFrame, LungsState, Moment, MouthState, NoseState, PersonaRef, Pin,
    ReadbackFrame, TextSpan, ThroatState, ValueOrigin,
};
use crate::names::{CowenName, Track};
use crate::{validate, FRAME_MS, SCHEMA, SCHEMA_VERSION};

const MAGIC: &[u8; 4] = b"GMN1";

pub fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    validate(doc)?;
    let strings = string_table(doc);
    if strings.len() > u16::MAX as usize {
        return Err("too many strings".into());
    }
    if doc.frame_count() > u16::MAX as usize {
        return Err("too many frames".into());
    }
    let mut out = Vec::with_capacity(64 + strings.len() * 16);
    out.extend_from_slice(MAGIC);
    out.push(match doc.kind {
        DocKind::Intent => 1,
        DocKind::Readback => 2,
    });
    out.push(match doc.value_origin {
        ValueOrigin::StandIn => 0,
        ValueOrigin::Config => 1,
        ValueOrigin::Measured => 2,
    });
    push_u16(&mut out, doc.frame_ms);
    push_u16(&mut out, doc.schema_version);
    push_u16(&mut out, doc.frame_count() as u16);
    push_u16(&mut out, strings.len() as u16);
    for s in &strings {
        if s.len() > u16::MAX as usize {
            return Err("string longer than 65535 bytes".into());
        }
        push_u16(&mut out, s.len() as u16);
        out.extend_from_slice(s.as_bytes());
    }
    for s in [
        doc.note.as_str(),
        doc.persona.persona_id.as_str(),
        doc.persona.voice_id.as_str(),
        doc.moment.moment_id.as_str(),
        doc.moment.memory_id.as_str(),
        doc.moment.memory_snippet.as_str(),
        doc.moment.place.as_str(),
        doc.moment.distance.as_str(),
        doc.moment.noise.as_str(),
        doc.moment.listener.as_str(),
        doc.text.as_str(),
    ] {
        push_u16(&mut out, index_of(&strings, s)?);
    }
    push_f64(&mut out, doc.baseline.arousal);
    push_f64(&mut out, doc.baseline.valence);
    push_f64(&mut out, doc.baseline.dominance);
    match doc.kind {
        DocKind::Intent => {
            for frame in &doc.intent_frames {
                push_feeling(&mut out, &frame.feeling);
                push_u16(&mut out, index_of(&strings, &frame.intent.verb)?);
                push_f64(&mut out, frame.intent.strength);
                push_f64(&mut out, frame.inner.energy);
                push_f64(&mut out, frame.inner.fatigue);
                push_f64(&mut out, frame.inner.health);
                push_u32(&mut out, frame.t_ms);
                push_u32(&mut out, frame.text_span.start_byte);
                push_u32(&mut out, frame.text_span.end_byte);
            }
        }
        DocKind::Readback => {
            for frame in &doc.readback_frames {
                push_feeling(&mut out, &frame.feeling);
                push_u32(&mut out, frame.t_ms);
                out.push(match frame.body.origin {
                    BodyOrigin::Unmeasured => 0,
                    BodyOrigin::StandIn => 1,
                    BodyOrigin::Measured => 2,
                });
                out.push(segment_byte(frame.body.lungs.segment));
                push_opt_f64s(
                    &mut out,
                    &[
                        frame.body.lungs.lung_volume_rel,
                        frame.body.lungs.subglottal_pressure_rel,
                        frame.body.lungs.loudness_db,
                    ],
                );
                push_opt_f64s(
                    &mut out,
                    &[frame.body.throat.f0_hz, frame.body.throat.hnr_db],
                );
                push_opt_f64s(
                    &mut out,
                    &[
                        frame.body.mouth.f1_hz,
                        frame.body.mouth.f2_hz,
                        frame.body.mouth.f3_hz,
                    ],
                );
                push_opt_f64s(&mut out, &[frame.body.nose.nasalance]);
                push_f64(&mut out, frame.confidence.feeling);
                push_f64(&mut out, frame.confidence.body);
                out.push(u8::from(frame.confidence.confounded));
            }
        }
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() < 4 || &bytes[0..4] != MAGIC {
        return Err("not a galion-mne-contract binary (magic GMN1)".into());
    }
    let mut r = Reader { b: bytes, i: 4 };
    let kind = match r.u8()? {
        1 => DocKind::Intent,
        2 => DocKind::Readback,
        other => return Err(format!("unknown kind {other}")),
    };
    let value_origin = match r.u8()? {
        0 => ValueOrigin::StandIn,
        1 => ValueOrigin::Config,
        2 => ValueOrigin::Measured,
        other => return Err(format!("unknown value_origin {other}")),
    };
    let frame_ms = r.u16()?;
    let schema_version = r.u16()?;
    let n_frames = r.u16()? as usize;
    let n_strings = r.u16()? as usize;
    let mut strings = Vec::with_capacity(n_strings);
    for _ in 0..n_strings {
        let len = r.u16()? as usize;
        let raw = r.bytes(len)?;
        let s = std::str::from_utf8(raw).map_err(|_| "string table is not utf-8".to_string())?;
        strings.push(s.to_string());
    }
    let note = take(&strings, r.u16()?)?;
    let persona = PersonaRef {
        persona_id: take(&strings, r.u16()?)?,
        voice_id: take(&strings, r.u16()?)?,
    };
    let moment = Moment {
        moment_id: take(&strings, r.u16()?)?,
        memory_id: take(&strings, r.u16()?)?,
        memory_snippet: take(&strings, r.u16()?)?,
        place: take(&strings, r.u16()?)?,
        distance: take(&strings, r.u16()?)?,
        noise: take(&strings, r.u16()?)?,
        listener: take(&strings, r.u16()?)?,
    };
    let text = take(&strings, r.u16()?)?;
    let baseline = BaselinePoint {
        arousal: r.f64()?,
        valence: r.f64()?,
        dominance: r.f64()?,
    };
    let mut intent_frames = Vec::new();
    let mut readback_frames = Vec::new();
    for _ in 0..n_frames {
        let feeling = pull_feeling(&mut r)?;
        match kind {
            DocKind::Intent => {
                let verb = take(&strings, r.u16()?)?;
                let strength = r.f64()?;
                let energy = r.f64()?;
                let fatigue = r.f64()?;
                let health = r.f64()?;
                let t_ms = r.u32()?;
                let start_byte = r.u32()?;
                let end_byte = r.u32()?;
                intent_frames.push(IntentFrame {
                    t_ms,
                    intent: Intent { verb, strength },
                    feeling,
                    inner: InnerFactors {
                        energy,
                        fatigue,
                        health,
                    },
                    text_span: TextSpan {
                        start_byte,
                        end_byte,
                    },
                });
            }
            DocKind::Readback => {
                let t_ms = r.u32()?;
                let origin = match r.u8()? {
                    0 => BodyOrigin::Unmeasured,
                    1 => BodyOrigin::StandIn,
                    2 => BodyOrigin::Measured,
                    other => return Err(format!("unknown body origin {other}")),
                };
                let segment = segment_from(r.u8()?)?;
                let lung = pull_opt_f64s(&mut r, 3)?;
                let throat = pull_opt_f64s(&mut r, 2)?;
                let mouth = pull_opt_f64s(&mut r, 3)?;
                let nose = pull_opt_f64s(&mut r, 1)?;
                let confidence = Confidence {
                    feeling: r.f64()?,
                    body: r.f64()?,
                    confounded: match r.u8()? {
                        0 => false,
                        1 => true,
                        other => return Err(format!("confounded must be 0 or 1, found {other}")),
                    },
                };
                readback_frames.push(ReadbackFrame {
                    t_ms,
                    feeling,
                    body: BodyState {
                        origin,
                        lungs: LungsState {
                            segment,
                            lung_volume_rel: lung[0],
                            subglottal_pressure_rel: lung[1],
                            loudness_db: lung[2],
                        },
                        throat: ThroatState {
                            f0_hz: throat[0],
                            hnr_db: throat[1],
                        },
                        mouth: MouthState {
                            f1_hz: mouth[0],
                            f2_hz: mouth[1],
                            f3_hz: mouth[2],
                        },
                        nose: NoseState { nasalance: nose[0] },
                    },
                    confidence,
                });
            }
        }
    }
    r.finish()?;
    if frame_ms != FRAME_MS {
        return Err(format!("frame_ms {frame_ms}, want {FRAME_MS}"));
    }
    if schema_version != SCHEMA_VERSION {
        return Err(format!(
            "schema_version {schema_version}, want {SCHEMA_VERSION}"
        ));
    }
    let doc = Document {
        schema: SCHEMA.to_string(),
        schema_version,
        kind,
        frame_ms,
        value_origin,
        note,
        persona,
        moment,
        text,
        baseline,
        intent_frames,
        readback_frames,
    };
    validate(&doc)?;
    Ok(doc)
}

fn push_feeling(out: &mut Vec<u8>, feeling: &Feeling) {
    push_f64(out, feeling.arousal);
    push_f64(out, feeling.valence);
    push_f64(out, feeling.dominance);
    match &feeling.pin {
        None => {
            out.extend_from_slice(&[0, 0, 0]);
            push_f64(out, 0.0);
        }
        Some(Pin {
            name,
            track,
            weight,
        }) => {
            out.push(1);
            out.push(match track {
                Track::Prosody => 1,
                Track::Burst => 2,
            });
            out.push(name.index());
            push_f64(out, *weight);
        }
    }
}

fn pull_feeling(r: &mut Reader<'_>) -> Result<Feeling, String> {
    let arousal = r.f64()?;
    let valence = r.f64()?;
    let dominance = r.f64()?;
    let present = r.u8()?;
    let track_b = r.u8()?;
    let index = r.u8()?;
    let weight = r.f64()?;
    let pin = match present {
        0 => {
            if track_b != 0 || index != 0 || weight != 0.0 {
                return Err("absent pin must have zero track, index, and weight".into());
            }
            None
        }
        1 => {
            let track = match track_b {
                1 => Track::Prosody,
                2 => Track::Burst,
                other => return Err(format!("pin track {other}")),
            };
            let name = CowenName::from_index(index).ok_or_else(|| format!("pin index {index}"))?;
            Some(Pin {
                name,
                track,
                weight,
            })
        }
        other => return Err(format!("pin_present {other}")),
    };
    Ok(Feeling {
        arousal,
        valence,
        dominance,
        pin,
    })
}

fn push_opt_f64s(out: &mut Vec<u8>, values: &[Option<f64>]) {
    let mut flags = 0u8;
    for (i, v) in values.iter().enumerate() {
        if v.is_some() {
            flags |= 1 << i;
        }
    }
    out.push(flags);
    for v in values.iter().flatten() {
        push_f64(out, *v);
    }
}

fn pull_opt_f64s(r: &mut Reader<'_>, n: usize) -> Result<Vec<Option<f64>>, String> {
    let flags = r.u8()?;
    if flags >> n != 0 {
        return Err(format!("unused flag bits in {flags:#b}"));
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if flags & (1 << i) != 0 {
            out.push(Some(r.f64()?));
        } else {
            out.push(None);
        }
    }
    Ok(out)
}

fn segment_byte(segment: BreathSegment) -> u8 {
    match segment {
        BreathSegment::Unknown => 0,
        BreathSegment::Rest => 1,
        BreathSegment::Inhale => 2,
        BreathSegment::Hold => 3,
        BreathSegment::Speech => 4,
        BreathSegment::Exhale => 5,
    }
}

fn segment_from(byte: u8) -> Result<BreathSegment, String> {
    Ok(match byte {
        0 => BreathSegment::Unknown,
        1 => BreathSegment::Rest,
        2 => BreathSegment::Inhale,
        3 => BreathSegment::Hold,
        4 => BreathSegment::Speech,
        5 => BreathSegment::Exhale,
        other => return Err(format!("unknown breath segment {other}")),
    })
}

fn string_table(doc: &Document) -> Vec<String> {
    let mut table = Vec::new();
    let mut add = |s: &str| {
        if !table.iter().any(|e: &String| e == s) {
            table.push(s.to_string());
        }
    };
    add(&doc.note);
    add(&doc.persona.persona_id);
    add(&doc.persona.voice_id);
    add(&doc.moment.moment_id);
    add(&doc.moment.memory_id);
    add(&doc.moment.memory_snippet);
    add(&doc.moment.place);
    add(&doc.moment.distance);
    add(&doc.moment.noise);
    add(&doc.moment.listener);
    add(&doc.text);
    for frame in &doc.intent_frames {
        add(&frame.intent.verb);
    }
    table
}

fn index_of(table: &[String], s: &str) -> Result<u16, String> {
    table
        .iter()
        .position(|e| e == s)
        .map(|i| i as u16)
        .ok_or_else(|| "internal: string missing from table".into())
}

fn take(table: &[String], index: u16) -> Result<String, String> {
    table
        .get(index as usize)
        .cloned()
        .ok_or_else(|| format!("string index {index} out of range"))
}

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn push_f64(out: &mut Vec<u8>, v: f64) {
    out.extend_from_slice(&v.to_le_bytes());
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn need(&self, n: usize) -> Result<(), String> {
        if self.i + n > self.b.len() {
            Err(format!("truncated at byte {}, need {n} more", self.i))
        } else {
            Ok(())
        }
    }

    fn u8(&mut self) -> Result<u8, String> {
        self.need(1)?;
        let v = self.b[self.i];
        self.i += 1;
        Ok(v)
    }

    fn u16(&mut self) -> Result<u16, String> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.b[self.i], self.b[self.i + 1]]);
        self.i += 2;
        Ok(v)
    }

    fn u32(&mut self) -> Result<u32, String> {
        self.need(4)?;
        let mut a = [0u8; 4];
        a.copy_from_slice(&self.b[self.i..self.i + 4]);
        self.i += 4;
        Ok(u32::from_le_bytes(a))
    }

    fn f64(&mut self) -> Result<f64, String> {
        self.need(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(&self.b[self.i..self.i + 8]);
        self.i += 8;
        let v = f64::from_le_bytes(a);
        if !v.is_finite() {
            return Err(format!("non-finite float at byte {}", self.i - 8));
        }
        Ok(v)
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        self.need(n)?;
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }

    fn finish(&self) -> Result<(), String> {
        if self.i != self.b.len() {
            Err(format!("trailing {} bytes", self.b.len() - self.i))
        } else {
            Ok(())
        }
    }
}
