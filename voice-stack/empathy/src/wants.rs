// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Open wants. A want is data for the router. It is not a line to speak.
//!
//! States are open, met, and dropped. The list is in memory for this process.

use crate::scan::contains_phrase;

/// Where a want sits. Closed set: open, met, dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WantState {
    Open,
    Met,
    Dropped,
}

impl WantState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Met => "met",
            Self::Dropped => "dropped",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "open" => Some(Self::Open),
            "met" => Some(Self::Met),
            "dropped" => Some(Self::Dropped),
            _ => None,
        }
    }
}

/// One want. `text` is the caller's words, clipped, not a reply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Want {
    pub id: u32,
    pub text: String,
    pub state: WantState,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WantList {
    items: Vec<Want>,
    next_id: u32,
}

impl WantList {
    const MAX: usize = 32;
    const MAX_OPEN: usize = 8;
    const TAIL: usize = 80;

    pub(crate) fn new() -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
        }
    }

    pub(crate) fn as_slice(&self) -> &[Want] {
        &self.items
    }

    pub(crate) fn counts(&self) -> (u32, u32, u32) {
        let mut open = 0u32;
        let mut met = 0u32;
        let mut dropped = 0u32;
        for want in &self.items {
            match want.state {
                WantState::Open => open += 1,
                WantState::Met => met += 1,
                WantState::Dropped => dropped += 1,
            }
        }
        (open, met, dropped)
    }

    /// Read one committed utterance. Partials must not call this.
    pub(crate) fn observe(&mut self, text: &str) {
        if let Some(tail) = open_tail(text) {
            self.push_open(tail);
            return;
        }
        if DROP.iter().any(|p| contains_phrase(text, p)) {
            self.settle_newest(WantState::Dropped);
            return;
        }
        if MET.iter().any(|p| contains_phrase(text, p)) {
            self.settle_newest(WantState::Met);
        }
    }

    pub(crate) fn open(&mut self, text: &str) -> Option<u32> {
        let tail = clip(text)?;
        self.push_open(tail)
    }

    pub(crate) fn mark(&mut self, id: u32, state: WantState) -> bool {
        if !matches!(state, WantState::Met | WantState::Dropped) {
            return false;
        }
        let Some(want) = self.items.iter_mut().find(|w| w.id == id) else {
            return false;
        };
        if want.state != WantState::Open {
            return false;
        }
        want.state = state;
        true
    }

    fn push_open(&mut self, tail: &str) -> Option<u32> {
        if self
            .items
            .iter()
            .any(|w| w.state == WantState::Open && w.text.eq_ignore_ascii_case(tail))
        {
            return self
                .items
                .iter()
                .find(|w| w.state == WantState::Open && w.text.eq_ignore_ascii_case(tail))
                .map(|w| w.id);
        }
        let open = self
            .items
            .iter()
            .filter(|w| w.state == WantState::Open)
            .count();
        if open >= Self::MAX_OPEN {
            return None;
        }
        self.make_room();
        if self.items.len() >= Self::MAX {
            return None;
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.items.push(Want {
            id,
            text: tail.to_string(),
            state: WantState::Open,
        });
        Some(id)
    }

    fn settle_newest(&mut self, state: WantState) {
        if let Some(want) = self
            .items
            .iter_mut()
            .rev()
            .find(|w| w.state == WantState::Open)
        {
            want.state = state;
        }
    }

    fn make_room(&mut self) {
        if self.items.len() < Self::MAX {
            return;
        }
        if let Some(i) = self.items.iter().position(|w| w.state != WantState::Open) {
            self.items.remove(i);
        }
    }
}

const OPEN_PREFIX: &[&str] = &["i want ", "i need ", "i wish "];
const MET: &[&str] = &[
    "that's it",
    "thats it",
    "that works",
    "you did it",
    "got it",
    "perfect",
    "that is what i wanted",
    "that's what i wanted",
];
const DROP: &[&str] = &[
    "never mind",
    "nevermind",
    "forget it",
    "drop that",
    "drop it",
    "don't bother",
    "dont bother",
    "cancel that",
    "leave it",
];

fn open_tail(text: &str) -> Option<&str> {
    let mut found: Option<&str> = None;
    for prefix in OPEN_PREFIX {
        if let Some(tail) = find_prefix(text, prefix) {
            found = Some(tail);
        }
    }
    let tail = clip(found?)?;
    if excluded(tail) {
        None
    } else {
        Some(tail)
    }
}

fn find_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let hay = text.as_bytes();
    let needle = prefix.as_bytes();
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let boundary = i == 0 || !hay[i - 1].is_ascii_alphanumeric();
        if boundary && crate::scan::eq_ci(&hay[i..], needle) {
            return Some(&text[i + needle.len()..]);
        }
        i += 1;
    }
    None
}

fn excluded(tail: &str) -> bool {
    let mut buf = [0u8; WantList::TAIL];
    let n = tail.len().min(buf.len());
    for (i, b) in tail.as_bytes()[..n].iter().enumerate() {
        buf[i] = b.to_ascii_lowercase();
    }
    let lower = std::str::from_utf8(&buf[..n]).unwrap_or("");
    lower.starts_with("to vent") || lower.starts_with("to talk") || lower.starts_with("you to")
}

fn clip(text: &str) -> Option<&str> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let end = text.find(['.', '!', '?', '\n']).unwrap_or(text.len());
    let text = text[..end].trim().trim_end_matches([',', ';', ':']).trim();
    if text.is_empty() {
        return None;
    }
    let mut end = text.len().min(WantList::TAIL);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    let text = text[..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_then_met_then_a_new_one_dropped() {
        let mut list = WantList::new();
        list.observe("I want the gate checked");
        assert_eq!(list.as_slice().len(), 1);
        assert_eq!(list.as_slice()[0].state, WantState::Open);
        assert_eq!(list.as_slice()[0].text, "the gate checked");
        list.observe("that works");
        assert_eq!(list.as_slice()[0].state, WantState::Met);
        list.observe("I wish it were quieter");
        list.observe("never mind");
        assert_eq!(list.as_slice()[1].state, WantState::Dropped);
        assert_eq!(list.as_slice()[1].text, "it were quieter");
        list.observe("I need to vent");
        assert_eq!(list.as_slice().len(), 2);
        list.observe("I need tea");
        assert_eq!(list.as_slice()[2].text, "tea");
        assert_eq!(list.as_slice()[2].state, WantState::Open);
    }

    #[test]
    fn the_same_final_does_not_open_twice_and_met_does_not_reopen() {
        let mut list = WantList::new();
        list.observe("I want tea");
        list.observe("I want tea");
        assert_eq!(list.as_slice().len(), 1);
        let id = list.as_slice()[0].id;
        assert!(list.mark(id, WantState::Met));
        assert!(!list.mark(id, WantState::Dropped));
        assert!(!list.mark(id, WantState::Open));
    }
}
