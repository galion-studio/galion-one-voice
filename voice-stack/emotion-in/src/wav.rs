// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! WAV and raw s16le framing. One frame is decoded into a caller-provided
//! buffer. The puller keeps at most one input frame of bytes and wipes it
//! when a frame is handed off, on finish, and on drop.
//!
//! PCM integer 16-bit and IEEE float 32-bit, mono or stereo, 1..=48000 Hz
//! when 80 ms lands on a whole number of samples. Anything else is refused.

use crate::clock::{samples_per_frame, FRAME_SAMPLES, MAX_INPUT_SAMPLES};
use crate::consent::{ConsentGate, Purpose};
use crate::error::{EmotionError, EmotionErrorKind};
use crate::report::Report;
use crate::stream::EmotionIn;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Instant;

const MAX_FRAME_BYTES: usize = MAX_INPUT_SAMPLES * 8;

pub fn resample_frame(input: &[f32], _in_rate: u32, out: &mut [f32; FRAME_SAMPLES]) {
    if input.len() == FRAME_SAMPLES {
        out.copy_from_slice(input);
        return;
    }
    if input.is_empty() {
        *out = [0.0; FRAME_SAMPLES];
        return;
    }
    if input.len() == 1 {
        *out = [input[0]; FRAME_SAMPLES];
        return;
    }
    let last = input.len() - 1;
    let span = FRAME_SAMPLES as f32 - 1.0;
    for (i, slot) in out.iter_mut().enumerate() {
        let src = (i as f32) * (last as f32) / span;
        let j = (src.floor() as usize).min(last);
        let frac = src - j as f32;
        let a = input[j];
        let b = input[(j + 1).min(last)];
        let s = a + (b - a) * frac;
        *slot = if s.is_finite() { s } else { 0.0 };
    }
}

/// Open `path` only after sense is granted. Samples are not kept in the report.
pub fn analyse_wav_path(path: &Path, gate: &dyn ConsentGate) -> Result<Report, EmotionError> {
    gate.permit(Purpose::Sense)?;
    let file = File::open(path)
        .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "could not open wav"))?;
    analyse_wav_reader(file)
}

pub fn analyse_wav_reader<R: Read>(reader: R) -> Result<Report, EmotionError> {
    let mut pull = WavPull::open(reader)?;
    let mut engine = EmotionIn::new();
    let mut times = Vec::new();
    let mut frame = [0.0f32; FRAME_SAMPLES];
    let mut frames = 0u32;
    while pull.next_frame(&mut frame)? {
        let started = Instant::now();
        engine.push_open(&frame)?;
        times.push(crate::report::elapsed_us(started));
        frames += 1;
        frame = [0.0; FRAME_SAMPLES];
    }
    let dropped = pull.dropped_tail_samples();
    pull.wipe();
    Ok(Report::from_reading(
        engine.reading(),
        frames,
        dropped,
        &times,
    ))
}

struct Fmt {
    channels: u16,
    rate: u32,
    bits: u16,
}

pub struct WavPull<R: Read> {
    reader: R,
    fmt: Fmt,
    data_left: usize,
    dropped: u32,
    raw: [u8; MAX_FRAME_BYTES],
    mono: [f32; MAX_INPUT_SAMPLES],
    done: bool,
}

impl<R: Read> Drop for WavPull<R> {
    fn drop(&mut self) {
        self.wipe();
    }
}

impl<R: Read> WavPull<R> {
    pub fn open(mut reader: R) -> Result<Self, EmotionError> {
        let (fmt, data_left) = read_header(&mut reader)?;
        Ok(Self {
            reader,
            fmt,
            data_left,
            dropped: 0,
            raw: [0; MAX_FRAME_BYTES],
            mono: [0.0; MAX_INPUT_SAMPLES],
            done: false,
        })
    }

    pub fn dropped_tail_samples(&self) -> u32 {
        self.dropped
    }

    pub fn wipe(&mut self) {
        self.raw = [0; MAX_FRAME_BYTES];
        self.mono = [0.0; MAX_INPUT_SAMPLES];
    }

    pub fn next_frame(&mut self, dst: &mut [f32; FRAME_SAMPLES]) -> Result<bool, EmotionError> {
        if self.done {
            return Ok(false);
        }
        let hop = samples_per_frame(self.fmt.rate).ok_or(EmotionError::new(
            EmotionErrorKind::BadRate,
            "wav rate must be 1..=48000 Hz and divide 80 ms into a whole number of samples",
        ))?;
        let width = (self.fmt.bits / 8) as usize;
        let channels = self.fmt.channels as usize;
        let need = hop
            .checked_mul(channels)
            .and_then(|n| n.checked_mul(width))
            .ok_or(EmotionError::new(
                EmotionErrorKind::BadAudio,
                "wav frame is too large",
            ))?;
        if need > MAX_FRAME_BYTES || hop > MAX_INPUT_SAMPLES {
            return Err(EmotionError::new(
                EmotionErrorKind::BadAudio,
                "wav frame is too large",
            ));
        }
        if self.data_left < need {
            let sample_bytes = channels * width;
            self.dropped = if sample_bytes == 0 {
                0
            } else {
                (self.data_left / sample_bytes) as u32
            };
            self.data_left = 0;
            self.done = true;
            self.wipe();
            return Ok(false);
        }
        self.reader.read_exact(&mut self.raw[..need]).map_err(|_| {
            EmotionError::new(EmotionErrorKind::BadAudio, "wav ended inside a frame")
        })?;
        self.data_left -= need;
        decode_mono(
            &self.raw[..need],
            channels,
            self.fmt.bits,
            &mut self.mono[..hop],
        )?;
        resample_frame(&self.mono[..hop], self.fmt.rate, dst);
        self.wipe();
        Ok(true)
    }
}

fn decode_mono(
    raw: &[u8],
    channels: usize,
    bits: u16,
    dst: &mut [f32],
) -> Result<(), EmotionError> {
    let width = (bits / 8) as usize;
    let frame = channels * width;
    if frame == 0 || raw.len() < dst.len() * frame {
        return Err(EmotionError::new(
            EmotionErrorKind::BadAudio,
            "short wav frame",
        ));
    }
    for (i, slot) in dst.iter_mut().enumerate() {
        let mut acc = 0.0f32;
        for c in 0..channels {
            let o = i * frame + c * width;
            acc += match bits {
                16 => i16::from_le_bytes([raw[o], raw[o + 1]]) as f32 / 32768.0,
                32 => f32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]),
                _ => {
                    return Err(EmotionError::new(
                        EmotionErrorKind::BadAudio,
                        "unsupported sample width",
                    ))
                }
            };
        }
        let s = acc / channels as f32;
        *slot = if s.is_finite() {
            s.clamp(-1.0, 1.0)
        } else {
            0.0
        };
    }
    Ok(())
}

fn read_header<R: Read>(reader: &mut R) -> Result<(Fmt, usize), EmotionError> {
    let mut riff = [0u8; 12];
    read_exact(reader, &mut riff)?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err(EmotionError::new(
            EmotionErrorKind::BadAudio,
            "not a RIFF WAVE file",
        ));
    }
    let mut fmt: Option<Fmt> = None;
    loop {
        let mut chunk = [0u8; 8];
        read_exact(reader, &mut chunk)?;
        let id = [chunk[0], chunk[1], chunk[2], chunk[3]];
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as usize;
        if &id == b"fmt " {
            if !(16..=64).contains(&size) {
                return Err(EmotionError::new(
                    EmotionErrorKind::BadAudio,
                    "wav fmt chunk is an unexpected size",
                ));
            }
            let mut body = [0u8; 64];
            read_exact(reader, &mut body[..size])?;
            if size % 2 == 1 {
                skip(reader, 1)?;
            }
            fmt = Some(parse_fmt(&body[..size])?);
        } else if &id == b"data" {
            let fmt = fmt.ok_or(EmotionError::new(
                EmotionErrorKind::BadAudio,
                "wav data chunk came before fmt",
            ))?;
            return Ok((fmt, size));
        } else {
            if size > 1_048_576 {
                return Err(EmotionError::new(
                    EmotionErrorKind::BadAudio,
                    "wav chunk is too large to skip",
                ));
            }
            skip(reader, size)?;
            if size % 2 == 1 {
                skip(reader, 1)?;
            }
        }
    }
}

fn parse_fmt(fmt: &[u8]) -> Result<Fmt, EmotionError> {
    let format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let channels = u16::from_le_bytes([fmt[2], fmt[3]]);
    let rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
    let bits = u16::from_le_bytes([fmt[14], fmt[15]]);
    if channels == 0 || channels > 2 {
        return Err(EmotionError::new(
            EmotionErrorKind::BadAudio,
            "wav must be mono or stereo",
        ));
    }
    let pcm16 = format == 1 && bits == 16;
    let f32le = format == 3 && bits == 32;
    if !pcm16 && !f32le {
        return Err(EmotionError::new(
            EmotionErrorKind::BadAudio,
            "wav must be PCM s16le or IEEE float 32",
        ));
    }
    if samples_per_frame(rate).is_none() {
        return Err(EmotionError::new(
            EmotionErrorKind::BadRate,
            "wav rate must be 1..=48000 Hz and divide 80 ms into a whole number of samples",
        ));
    }
    Ok(Fmt {
        channels,
        rate,
        bits,
    })
}

fn read_exact<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<(), EmotionError> {
    reader
        .read_exact(buf)
        .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "wav ended early"))
}

fn skip<R: Read>(reader: &mut R, mut n: usize) -> Result<(), EmotionError> {
    let mut buf = [0u8; 256];
    while n > 0 {
        let k = n.min(buf.len());
        read_exact(reader, &mut buf[..k])?;
        n -= k;
    }
    Ok(())
}

/// Raw mono s16le. One 80 ms frame at a time. The byte buffer is wiped after
/// each frame and on drop. A short tail is discarded, not kept.
pub struct PcmS16<R: Read> {
    reader: R,
    hop: usize,
    raw: [u8; MAX_INPUT_SAMPLES * 2],
    mono: [f32; MAX_INPUT_SAMPLES],
}

impl<R: Read> Drop for PcmS16<R> {
    fn drop(&mut self) {
        self.wipe();
    }
}

impl<R: Read> PcmS16<R> {
    pub fn open(reader: R, rate: u32) -> Result<Self, EmotionError> {
        let hop = samples_per_frame(rate).ok_or(EmotionError::new(
            EmotionErrorKind::BadRate,
            "pcm rate must be 1..=48000 Hz and divide 80 ms into a whole number of samples",
        ))?;
        Ok(Self {
            reader,
            hop,
            raw: [0; MAX_INPUT_SAMPLES * 2],
            mono: [0.0; MAX_INPUT_SAMPLES],
        })
    }

    pub fn wipe(&mut self) {
        self.raw = [0; MAX_INPUT_SAMPLES * 2];
        self.mono = [0.0; MAX_INPUT_SAMPLES];
    }

    pub fn next_frame(&mut self, dst: &mut [f32; FRAME_SAMPLES]) -> Result<bool, EmotionError> {
        let need = self.hop * 2;
        match read_full(&mut self.reader, &mut self.raw[..need])? {
            ReadFull::Eof | ReadFull::Short => {
                self.wipe();
                Ok(false)
            }
            ReadFull::Full => {
                decode_mono(&self.raw[..need], 1, 16, &mut self.mono[..self.hop])?;
                // `hop` samples are one 80 ms frame at the caller's rate.
                // A 1920-sample frame is copied. Any other length is mapped
                // onto the 24 kHz frame by position, which keeps the 80 ms span.
                resample_frame(&self.mono[..self.hop], 0, dst);
                self.wipe();
                Ok(true)
            }
        }
    }
}

enum ReadFull {
    Eof,
    Short,
    Full,
}

fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<ReadFull, EmotionError> {
    let mut got = 0;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) if got == 0 => return Ok(ReadFull::Eof),
            Ok(0) => return Ok(ReadFull::Short),
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err(EmotionError::new(
                    EmotionErrorKind::BadAudio,
                    "pcm read failed",
                ))
            }
        }
    }
    Ok(ReadFull::Full)
}

pub fn analyse_pcm_s16<R: Read>(reader: R, rate: u32) -> Result<Report, EmotionError> {
    let mut pull = PcmS16::open(reader, rate)?;
    let mut engine = EmotionIn::new();
    let mut times = Vec::new();
    let mut frame = [0.0f32; FRAME_SAMPLES];
    let mut frames = 0u32;
    while pull.next_frame(&mut frame)? {
        let started = Instant::now();
        engine.push_open(&frame)?;
        times.push(crate::report::elapsed_us(started));
        frames += 1;
        frame = [0.0; FRAME_SAMPLES];
    }
    pull.wipe();
    Ok(Report::from_reading(engine.reading(), frames, 0, &times))
}

/// Write mono PCM s16le. Used by tests to build a temporary file. Not a recorder.
pub fn write_wav_s16(path: &Path, samples: &[f32], rate: u32) -> Result<(), EmotionError> {
    let mut file = File::create(path)
        .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "could not create wav"))?;
    let data_len = samples.len() * 2;
    if data_len > u32::MAX as usize {
        return Err(EmotionError::new(
            EmotionErrorKind::BadAudio,
            "wav is too long",
        ));
    }
    let mut hdr = [0u8; 44];
    hdr[0..4].copy_from_slice(b"RIFF");
    hdr[4..8].copy_from_slice(&(36u32 + data_len as u32).to_le_bytes());
    hdr[8..12].copy_from_slice(b"WAVE");
    hdr[12..16].copy_from_slice(b"fmt ");
    hdr[16..20].copy_from_slice(&16u32.to_le_bytes());
    hdr[20..22].copy_from_slice(&1u16.to_le_bytes());
    hdr[22..24].copy_from_slice(&1u16.to_le_bytes());
    hdr[24..28].copy_from_slice(&rate.to_le_bytes());
    hdr[28..32].copy_from_slice(&(rate.saturating_mul(2)).to_le_bytes());
    hdr[32..34].copy_from_slice(&2u16.to_le_bytes());
    hdr[34..36].copy_from_slice(&16u16.to_le_bytes());
    hdr[36..40].copy_from_slice(b"data");
    hdr[40..44].copy_from_slice(&(data_len as u32).to_le_bytes());
    file.write_all(&hdr)
        .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "could not write wav"))?;
    let mut buf = [0u8; 2048];
    let mut i = 0;
    while i < samples.len() {
        let n = (samples.len() - i).min(1024);
        for k in 0..n {
            let x = samples[i + k];
            let c = if x.is_finite() {
                x.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let s = (c * 32767.0).round() as i16;
            buf[k * 2..k * 2 + 2].copy_from_slice(&s.to_le_bytes());
        }
        file.write_all(&buf[..n * 2])
            .map_err(|_| EmotionError::new(EmotionErrorKind::BadAudio, "could not write wav"))?;
        i += n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::Session;
    use crate::synth;

    #[test]
    fn deny_does_not_open_a_missing_wav() {
        let err =
            analyse_wav_path(Path::new("/no/such/emotion-in.wav"), &Session::DENY).unwrap_err();
        assert_eq!(err.kind, EmotionErrorKind::Denied);
    }

    #[test]
    fn roundtrip_s16_keeps_pitch() {
        let samples = synth::steady_frames(12, 180.0, 0.2);
        let path = std::env::temp_dir().join(format!(
            "galion-emotion-in-{}-{}.wav",
            std::process::id(),
            180
        ));
        write_wav_s16(&path, &samples, 24_000).unwrap();
        let report = analyse_wav_path(&path, &Session::grant_sense()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(report.audio_retained_samples, 0);
        assert!(
            (report.reading.prosody.f0_hz - 180.0).abs() < 3.6,
            "{:?}",
            report.reading.prosody
        );
        assert!(report.reading.mapped.confidence >= 0.2);
    }
}
