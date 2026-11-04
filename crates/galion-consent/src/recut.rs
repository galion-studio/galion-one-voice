// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Re-cut a reference wav to the gate's 8–12 s mono 16-bit shape, and move
//! its sha256 pin (owner item 4, 2026-11-04).
//!
//! What it does to the audio, and nothing more:
//! - reads RIFF/WAVE PCM 16-bit (1 or 2 channels, any rate);
//! - downmixes stereo to mono by averaging the two channels (no gain, no
//!   normalisation, no denoise, no resample, no high-pass);
//! - cuts one window whose length is inside [`DURATION_MIN`, `DURATION_MAX`],
//!   either the one the operator names (`start_s`/`end_s`) or the one
//!   [`choose_window`] finds at quiet 20 ms frames (word gaps), closest to
//!   [`TARGET_S`].
//!
//! It never writes a consent record. The operator copies the printed
//! `duration_s` and `sha256` into a real, signed `consent.txt`.
//! Pins: [`refs_row`] renders a `scripts/clone/REFS.sha256` row and
//! [`replace_refs_row`] swaps one persona's row in that file (the old row is
//! kept, commented, never dropped). Other pinned places (persona tomls, test
//! pins) are listed by [`files_pinning`] so the operator can move them in the
//! same commit.

use crate::{sha256, BLOCKED_VOICES, DURATION_MAX, DURATION_MIN};
use std::path::{Path, PathBuf};

/// Preferred reference length when the tool picks the window.
pub const TARGET_S: f64 = 10.0;
/// Keep this far inside the gate bounds so a rounding error never fails it.
pub const MARGIN_S: f64 = 0.05;
/// A frame is a cut candidate when it is this far below the loudest frame.
pub const GAP_DB: f64 = 35.0;
const FRAME_MS: f64 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Pcm16 {
    pub rate: u32,
    pub channels: u16,
    /// Interleaved samples.
    pub samples: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub rate: u32,
    pub start_sample: usize,
    pub end_sample: usize,
    pub mono: Vec<i16>,
}

impl Cut {
    pub fn seconds(&self) -> f64 {
        self.mono.len() as f64 / self.rate as f64
    }
    pub fn wav_bytes(&self) -> Vec<u8> {
        write_wav_mono16(self.rate, &self.mono)
    }
}

/// Parse a 16-bit PCM wav (format tag 1). Anything else is refused: this tool
/// does not convert MP3, float, 24-bit or extensible files.
pub fn read_pcm16(bytes: &[u8]) -> Result<Pcm16, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut pos = 12usize;
    let mut fmt: Option<(u16, u32, u16)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes([
            bytes[pos + 4],
            bytes[pos + 5],
            bytes[pos + 6],
            bytes[pos + 7],
        ]) as usize;
        let body = pos + 8;
        let end = body.saturating_add(size).min(bytes.len());
        if id == b"fmt " {
            let c = &bytes[body..end];
            if c.len() < 16 {
                return Err("short fmt chunk".into());
            }
            let tag = u16::from_le_bytes([c[0], c[1]]);
            let ch = u16::from_le_bytes([c[2], c[3]]);
            let rate = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
            let bits = u16::from_le_bytes([c[14], c[15]]);
            if tag != 1 || bits != 16 {
                return Err(format!("need PCM 16-bit (tag {tag}, {bits} bits)"));
            }
            if !(1..=2).contains(&ch) || rate == 0 {
                return Err(format!(
                    "need 1 or 2 channels at a real rate ({ch} ch, {rate} Hz)"
                ));
            }
            fmt = Some((ch, rate, bits));
        } else if id == b"data" {
            let (channels, rate, _) = fmt.ok_or("data before fmt")?;
            let data = &bytes[body..end];
            let n = data.len() / 2 / channels as usize * channels as usize;
            let samples = (0..n)
                .map(|i| i16::from_le_bytes([data[2 * i], data[2 * i + 1]]))
                .collect();
            return Ok(Pcm16 {
                rate,
                channels,
                samples,
            });
        }
        pos = body.saturating_add(size + (size & 1));
    }
    Err("no data chunk".into())
}

/// Average the channels. Mono is returned as is. No gain change.
pub fn downmix(p: &Pcm16) -> Vec<i16> {
    if p.channels == 1 {
        return p.samples.clone();
    }
    p.samples
        .chunks_exact(p.channels as usize)
        .map(|f| {
            let s: i32 = f.iter().map(|&v| v as i32).sum();
            // floor division keeps it deterministic for negatives too
            s.div_euclid(f.len() as i32) as i16
        })
        .collect()
}

fn frame_db(f: &[i16]) -> f64 {
    if f.is_empty() {
        return -120.0;
    }
    let e: f64 = f.iter().map(|&v| (v as f64 / 32768.0).powi(2)).sum::<f64>() / f.len() as f64;
    if e <= 0.0 {
        -120.0
    } else {
        10.0 * e.log10()
    }
}

/// Indices (in frames) of quiet 20 ms frames: cut candidates.
pub fn quiet_frames(mono: &[i16], rate: u32) -> Vec<usize> {
    let fr = ((rate as f64 * FRAME_MS / 1000.0) as usize).max(1);
    let dbs: Vec<f64> = mono.chunks(fr).map(frame_db).collect();
    let peak = dbs.iter().cloned().fold(-120.0, f64::max);
    dbs.iter()
        .enumerate()
        .filter(|(_, d)| **d <= peak - GAP_DB)
        .map(|(i, _)| i)
        .collect()
}

fn in_bounds(seconds: f64) -> bool {
    seconds >= DURATION_MIN + MARGIN_S && seconds <= DURATION_MAX - MARGIN_S
}

/// The window from one quiet frame to a later one whose length is inside the
/// gate bounds and closest to [`TARGET_S`]; ties go to the earlier start.
/// Cuts land in the middle of the quiet frame. `None` when the clip has no
/// such pair (too short, or no word gaps).
pub fn choose_window(mono: &[i16], rate: u32) -> Option<(usize, usize)> {
    let fr = ((rate as f64 * FRAME_MS / 1000.0) as usize).max(1);
    let mut q = quiet_frames(mono, rate);
    // The clip edges count as cut points too.
    q.insert(0, 0);
    let last = mono.len().div_ceil(fr);
    let mut best: Option<(f64, usize, usize)> = None;
    for (a_i, &a) in q.iter().enumerate() {
        let s = if a == 0 { 0 } else { a * fr + fr / 2 };
        for &b in q[a_i + 1..].iter().chain(std::iter::once(&last)) {
            let e = if b == last {
                mono.len()
            } else {
                (b * fr + fr / 2).min(mono.len())
            };
            if e <= s {
                continue;
            }
            let secs = (e - s) as f64 / rate as f64;
            if !in_bounds(secs) {
                continue;
            }
            let score = (secs - TARGET_S).abs();
            if best.is_none_or(|(bs, _, _)| score < bs - 1e-9) {
                best = Some((score, s, e));
            }
        }
    }
    best.map(|(_, s, e)| (s, e))
}

/// Cut `[start_s, end_s)` (operator window) or the chosen window.
pub fn recut(p: &Pcm16, window: Option<(f64, f64)>) -> Result<Cut, String> {
    let mono = downmix(p);
    let (s, e) = match window {
        Some((a, b)) => {
            if !(a >= 0.0 && b > a) {
                return Err(format!("bad window {a}..{b}"));
            }
            let s = (a * p.rate as f64).round() as usize;
            let e = ((b * p.rate as f64).round() as usize).min(mono.len());
            (s, e)
        }
        None => choose_window(&mono, p.rate).ok_or_else(|| {
            format!(
                "no {DURATION_MIN}-{DURATION_MAX} s window between word gaps in {:.2} s of audio; name one with --start/--end",
                mono.len() as f64 / p.rate as f64
            )
        })?,
    };
    if s >= e {
        return Err("window is empty".into());
    }
    let cut = Cut {
        rate: p.rate,
        start_sample: s,
        end_sample: e,
        mono: mono[s..e].to_vec(),
    };
    if !in_bounds(cut.seconds()) {
        return Err(format!(
            "cut is {:.3} s; the gate needs {:.2}-{:.2} s",
            cut.seconds(),
            DURATION_MIN + MARGIN_S,
            DURATION_MAX - MARGIN_S
        ));
    }
    Ok(cut)
}

pub fn write_wav_mono16(rate: u32, mono: &[i16]) -> Vec<u8> {
    let data_len = (mono.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for v in mono {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = sha256::Sha256::new();
    h.update(bytes);
    h.hex()
}

/// Refuse a source path that names a blocked voice (LOCKS: no celebrity clone).
pub fn refuse_blocked(path: &Path) -> Result<(), String> {
    let low = path.to_string_lossy().to_lowercase();
    match BLOCKED_VOICES.iter().find(|b| low.contains(*b)) {
        Some(b) => Err(format!(
            "{b} is a blocked voice (LOCKS: no celebrity clone)"
        )),
        None => Ok(()),
    }
}

/// One REFS.sha256 row:
/// `persona sha256 duration_s rate_hz channels bits repo_rel store_rel texture`.
pub fn refs_row(persona: &str, sha: &str, cut: &Cut, texture: &str) -> String {
    format!(
        "{persona} {sha} {:.2} {} 1 16 docs/galion-voice-v2/personas/refs/{persona}.wav {persona}/reference.wav {texture}",
        cut.seconds(),
        cut.rate
    )
}

/// Replace `persona`'s live row in REFS.sha256 text. The old row stays in the
/// file as a comment (`# superseded <note>: <old row>`), so nothing is lost.
/// Errors when the persona has no live row (this tool does not add personas).
pub fn replace_refs_row(
    text: &str,
    persona: &str,
    new_row: &str,
    note: &str,
) -> Result<(String, String), String> {
    let mut old = None;
    let mut out = String::with_capacity(text.len() + new_row.len() + 64);
    for line in text.lines() {
        let live =
            !line.trim_start().starts_with('#') && line.split_whitespace().next() == Some(persona);
        if live && old.is_none() {
            out.push_str(&format!("# superseded {note}: {line}\n{new_row}\n"));
            old = Some(line.split_whitespace().nth(1).unwrap_or("").to_string());
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    old.map(|o| (out, o))
        .ok_or_else(|| format!("no live {persona} row in REFS.sha256"))
}

/// Tracked text files under `root` (crates/, scripts/, docs/) that still
/// contain `needle`. `target/` and dot-dirs are skipped.
pub fn files_pinning(root: &Path, needle: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut stack: Vec<PathBuf> = ["crates", "scripts", "docs"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            let name = ent.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "__pycache__" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(t) = std::fs::read_to_string(&p) {
                if t.contains(needle) {
                    hits.push(p);
                }
            }
        }
    }
    hits.sort();
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn speechy(rate: u32, seconds: f64, gaps_every_s: f64) -> Vec<i16> {
        // tone bursts with 100 ms of silence every `gaps_every_s`
        let n = (rate as f64 * seconds) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 / rate as f64;
                let in_gap = (t % gaps_every_s) > gaps_every_s - 0.1;
                if in_gap {
                    0
                } else {
                    ((t * 220.0 * std::f64::consts::TAU).sin() * 9000.0) as i16
                }
            })
            .collect()
    }

    fn stereo(mono: &[i16]) -> Pcm16 {
        Pcm16 {
            rate: 48_000,
            channels: 2,
            samples: mono.iter().flat_map(|&v| [v, v / 2]).collect(),
        }
    }

    #[test]
    fn wav_round_trip_and_refusals() {
        let m = speechy(16_000, 1.0, 0.5);
        let bytes = write_wav_mono16(16_000, &m);
        let p = read_pcm16(&bytes).unwrap();
        assert_eq!((p.rate, p.channels), (16_000, 1));
        assert_eq!(p.samples, m);
        let mut float = bytes.clone();
        float[20] = 3; // tag 3 = IEEE float
        assert!(read_pcm16(&float).is_err());
        assert!(read_pcm16(b"ID3\x04 not a wav").is_err());
    }

    #[test]
    fn downmix_averages_without_gain() {
        let p = Pcm16 {
            rate: 8000,
            channels: 2,
            samples: vec![100, 300, -3, -4, i16::MAX, i16::MAX],
        };
        assert_eq!(downmix(&p), vec![200, -4, i16::MAX]);
    }

    #[test]
    fn long_stereo_clip_is_cut_to_the_gate_shape() {
        // 17.4 s stereo at 48 kHz, word gaps every 0.9 s: the shape of the old refs.
        let p = stereo(&speechy(48_000, 17.4, 0.9));
        let cut = recut(&p, None).unwrap();
        assert!(
            cut.seconds() >= DURATION_MIN + MARGIN_S && cut.seconds() <= DURATION_MAX - MARGIN_S,
            "{}",
            cut.seconds()
        );
        assert!(
            (cut.seconds() - TARGET_S).abs() < 0.5,
            "closest to 10 s: {}",
            cut.seconds()
        );
        let bytes = cut.wav_bytes();
        // The gate's own parser agrees: mono, 16-bit, in bounds.
        let tmp = std::env::temp_dir().join(format!("recut-{}.wav", std::process::id()));
        std::fs::write(&tmp, &bytes).unwrap();
        let (secs, ch, w) = crate::wav_info(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        assert_eq!((ch, w), (1, 2));
        assert!((DURATION_MIN..=DURATION_MAX).contains(&secs));
        // Cut starts at a quiet frame, not mid-word.
        let start = &cut.mono[..48];
        assert!(start.iter().all(|v| v.abs() < 200), "cut lands in a gap");
    }

    #[test]
    fn operator_window_and_bounds() {
        let p = Pcm16 {
            rate: 16_000,
            channels: 1,
            samples: speechy(16_000, 20.0, 1.0),
        };
        let cut = recut(&p, Some((2.0, 12.0))).unwrap();
        assert_eq!(cut.mono.len(), 160_000);
        assert_eq!(cut.start_sample, 32_000);
        assert!(recut(&p, Some((0.0, 7.0)))
            .unwrap_err()
            .contains("gate needs"));
        assert!(recut(&p, Some((0.0, 12.5))).is_err());
        assert!(recut(&p, Some((5.0, 4.0))).is_err());
    }

    #[test]
    fn short_clip_is_refused_not_padded() {
        let p = Pcm16 {
            rate: 16_000,
            channels: 1,
            samples: speechy(16_000, 6.4, 0.8),
        };
        assert!(recut(&p, None).unwrap_err().contains("no 8-12 s window"));
    }

    #[test]
    fn deterministic_hash() {
        let p = stereo(&speechy(48_000, 15.0, 0.7));
        let a = recut(&p, None).unwrap();
        let b = recut(&p, None).unwrap();
        assert_eq!(sha256_hex(&a.wav_bytes()), sha256_hex(&b.wav_bytes()));
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn refs_row_replacement_keeps_the_old_row() {
        let text = "# header\nsamantha aaaa 17.41 44100 2 16 docs/galion-voice-v2/personas/refs/samantha.wav samantha/reference.wav preserve\njarvis bbbb 15.95 48000 2 16 x y preserve-hub\n";
        let cut = Cut {
            rate: 44_100,
            start_sample: 0,
            end_sample: 441_000,
            mono: vec![0; 441_000],
        };
        let row = refs_row("samantha", "cccc", &cut, "preserve");
        assert_eq!(row, "samantha cccc 10.00 44100 1 16 docs/galion-voice-v2/personas/refs/samantha.wav samantha/reference.wav preserve");
        let (out, old) = replace_refs_row(text, "samantha", &row, "2026-11-04").unwrap();
        assert_eq!(old, "aaaa");
        assert!(out.contains("# superseded 2026-11-04: samantha aaaa 17.41"));
        assert!(out.contains(&row));
        assert!(out.contains("jarvis bbbb"));
        assert_eq!(
            out.lines().filter(|l| l.starts_with("samantha ")).count(),
            1
        );
        assert!(replace_refs_row(text, "armstrong", &row, "x").is_err());
    }

    #[test]
    fn blocked_voice_source_is_refused() {
        assert!(refuse_blocked(Path::new("/x/Armstrong-en.wav")).is_err());
        assert!(refuse_blocked(Path::new("/x/samantha.wav")).is_ok());
    }
}
