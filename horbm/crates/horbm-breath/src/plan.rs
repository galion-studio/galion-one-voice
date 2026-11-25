// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pass 1. Walk the reply once and write pause slots.
//! No heap. Words are whitespace splits of the caller's string.

use crate::{clause_limit_s, group_limit_s, snap_s, Drive, CHARS_PER_S, FRAME_S, PAUSE_CAP};

/// Why the mouth should break before the next word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseWhy {
    /// The next word would pass the breath-group budget.
    Air,
    /// Sentence end, line break, or a clause that is already long enough.
    Boundary,
}

/// One phrase break in the reply.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pause {
    /// Byte index of the word that starts after the break.
    pub at_byte: u32,
    /// Alphanumeric characters spoken before the break.
    pub at_chars: u32,
    /// Spoken seconds from the start of the reply.
    pub at_s: f32,
    pub why: PauseWhy,
}

impl Pause {
    #[must_use]
    pub const fn marker() -> Self {
        Self {
            at_byte: 0,
            at_chars: 0,
            at_s: 0.0,
            why: PauseWhy::Air,
        }
    }
}

/// How many pauses were written, and the group budget used.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    pub count: usize,
    pub truncated: bool,
    pub group_limit_s: f32,
    pub group_ms: u32,
}

/// Place breath pauses for `reply` into `out`.
///
/// A sentence end or a line break always closes a group. A comma, semicolon,
/// or colon closes one only once the group has reached the clause limit.
/// A word that would run past the group limit starts a new group. The last
/// group does not add a trailing pause.
#[must_use]
pub fn plan_pauses(reply: &str, drive: Drive, out: &mut [Pause]) -> Plan {
    let limit = group_limit_s(&drive);
    let clause = clause_limit_s(&drive);
    let mut count = 0usize;
    let mut truncated = false;
    let mut index = 0usize;
    let mut cur_s = 0.0f32;
    let mut speech_t = 0.0f32;
    let mut said = 0u32;
    let mut open = false;
    let mut pending = false;
    let mut pending_why = PauseWhy::Boundary;

    while let Some(tok) = next_tok(reply, index) {
        index = tok.next;
        if tok.seconds == 0.0 {
            if open && tok.strong {
                pending = true;
                pending_why = PauseWhy::Boundary;
            }
            continue;
        }
        if pending && open {
            if !emit(out, &mut count, tok.start, said, speech_t, pending_why) {
                truncated = true;
                break;
            }
            cur_s = 0.0;
            open = false;
            pending = false;
        }
        if open && cur_s + tok.seconds > limit + 1.0e-4 {
            if !emit(out, &mut count, tok.start, said, speech_t, PauseWhy::Air) {
                truncated = true;
                break;
            }
            cur_s = 0.0;
        }
        open = true;
        cur_s += tok.seconds;
        speech_t += tok.seconds;
        said = said.saturating_add(tok.chars);
        if tok.strong || (tok.weak && cur_s + 1.0e-4 >= clause) {
            pending = true;
            pending_why = PauseWhy::Boundary;
        }
    }

    let group_ms = seconds_to_ms(limit);
    Plan {
        count,
        truncated,
        group_limit_s: limit,
        group_ms,
    }
}

fn emit(
    out: &mut [Pause],
    count: &mut usize,
    at_byte: usize,
    at_chars: u32,
    at_s: f32,
    why: PauseWhy,
) -> bool {
    if *count >= out.len() || *count >= PAUSE_CAP {
        return false;
    }
    out[*count] = Pause {
        at_byte: u32::try_from(at_byte).unwrap_or(u32::MAX),
        at_chars,
        at_s,
        why,
    };
    *count += 1;
    true
}

struct Tok {
    start: usize,
    next: usize,
    chars: u32,
    seconds: f32,
    strong: bool,
    weak: bool,
}

fn next_tok(text: &str, mut index: usize) -> Option<Tok> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    while index < n && is_ws(bytes[index]) {
        index += 1;
    }
    if index >= n {
        return None;
    }
    let start = index;
    while index < n && !is_ws(bytes[index]) {
        index += 1;
    }
    let end = index;
    let mut line = false;
    while index < n && is_ws(bytes[index]) {
        if bytes[index] == b'\n' || bytes[index] == b'\r' {
            line = true;
        }
        index += 1;
    }
    let raw = &text[start..end];
    let chars = alphanumeric(raw);
    let seconds = if chars == 0 {
        0.0
    } else {
        snap_s(chars as f32 / CHARS_PER_S).max(FRAME_S)
    };
    let strong = line || has_any(raw, strong_mark);
    let weak = !strong && has_any(raw, weak_mark);
    Some(Tok {
        start,
        next: index,
        chars,
        seconds,
        strong,
        weak,
    })
}

fn is_ws(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c)
}

fn alphanumeric(text: &str) -> u32 {
    let mut n = 0u32;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            n = n.saturating_add(1);
        }
    }
    n
}

fn strong_mark(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?' | '…')
}

fn weak_mark(ch: char) -> bool {
    matches!(ch, ',' | ';' | ':')
}

fn has_any(text: &str, mark: fn(char) -> bool) -> bool {
    text.chars().any(mark)
}

fn seconds_to_ms(seconds: f32) -> u32 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    let ms = crate::round_pos(seconds * 1_000.0);
    if ms >= u32::MAX as f32 {
        u32::MAX
    } else {
        ms as u32
    }
}
