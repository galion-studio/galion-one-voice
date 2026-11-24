// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Canonical little-endian bytes. No timestamps from the wall clock.

use crate::{Error, Frame, Phase, Track, FRAME_MS, MAX_FRAMES};

const MAGIC: &[u8; 4] = b"HB01";
const VERSION: u16 = 1;
const HEADER: usize = 12;
const FRAME_BYTES: usize = 32;
const FLAG_SPEAKING: u8 = 0b0000_0001;
const FLAG_PAUSE: u8 = 0b0000_0010;

impl Track {
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER + self.frames.len() * FRAME_BYTES);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(self.frames.len() as u32).to_le_bytes());
        for frame in &self.frames {
            push_frame(&mut out, frame);
        }
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < HEADER || &bytes[0..4] != MAGIC {
            return Err(Error::BadBytes("magic"));
        }
        let version = u16::from_le_bytes(arr2(bytes, 4)?);
        if version != VERSION {
            return Err(Error::BadBytes("version"));
        }
        if u16::from_le_bytes(arr2(bytes, 6)?) != 0 {
            return Err(Error::BadBytes("reserved"));
        }
        let count = u32::from_le_bytes(arr4(bytes, 8)?);
        if count > MAX_FRAMES {
            return Err(Error::BadBytes("count"));
        }
        let need = HEADER + count as usize * FRAME_BYTES;
        if bytes.len() != need {
            return Err(Error::BadBytes("length"));
        }
        let mut frames = Vec::with_capacity(count as usize);
        for i in 0..count {
            frames.push(read_frame(bytes, HEADER + i as usize * FRAME_BYTES, i)?);
        }
        Ok(Self { frames })
    }
}

fn push_frame(out: &mut Vec<u8>, frame: &Frame) {
    out.extend_from_slice(&frame.t_ms.to_le_bytes());
    out.push(frame.phase.wire());
    let mut flags = 0u8;
    if frame.speaking {
        flags |= FLAG_SPEAKING;
    }
    if frame.pause {
        flags |= FLAG_PAUSE;
    }
    out.push(flags);
    out.extend_from_slice(&0u16.to_le_bytes());
    for value in [
        frame.subglottal_cm_h2o,
        frame.airflow_ml_s,
        frame.open_quotient,
        frame.breathiness,
        frame.nasal_coupling,
        frame.mouth_openness,
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

fn read_frame(bytes: &[u8], at: usize, index: u32) -> Result<Frame, Error> {
    let t_ms = u32::from_le_bytes(arr4(bytes, at)?);
    if t_ms != index.saturating_mul(FRAME_MS) {
        return Err(Error::BadBytes("t_ms"));
    }
    let phase = Phase::from_wire(bytes[at + 4])?;
    let flags = bytes[at + 5];
    if flags & !(FLAG_SPEAKING | FLAG_PAUSE) != 0 {
        return Err(Error::BadBytes("flags"));
    }
    if u16::from_le_bytes(arr2(bytes, at + 6)?) != 0 {
        return Err(Error::BadBytes("pad"));
    }
    let nums = [
        f32::from_le_bytes(arr4(bytes, at + 8)?),
        f32::from_le_bytes(arr4(bytes, at + 12)?),
        f32::from_le_bytes(arr4(bytes, at + 16)?),
        f32::from_le_bytes(arr4(bytes, at + 20)?),
        f32::from_le_bytes(arr4(bytes, at + 24)?),
        f32::from_le_bytes(arr4(bytes, at + 28)?),
    ];
    if nums.iter().any(|n| !n.is_finite()) {
        return Err(Error::BadBytes("number"));
    }
    Ok(Frame {
        t_ms,
        phase,
        speaking: flags & FLAG_SPEAKING != 0,
        pause: flags & FLAG_PAUSE != 0,
        subglottal_cm_h2o: nums[0],
        airflow_ml_s: nums[1],
        open_quotient: nums[2],
        breathiness: nums[3],
        nasal_coupling: nums[4],
        mouth_openness: nums[5],
    })
}

fn arr2(bytes: &[u8], at: usize) -> Result<[u8; 2], Error> {
    let s = bytes.get(at..at + 2).ok_or(Error::BadBytes("short"))?;
    s.try_into().map_err(|_| Error::BadBytes("short"))
}

fn arr4(bytes: &[u8], at: usize) -> Result<[u8; 4], Error> {
    let s = bytes.get(at..at + 4).ok_or(Error::BadBytes("short"))?;
    s.try_into().map_err(|_| Error::BadBytes("short"))
}
