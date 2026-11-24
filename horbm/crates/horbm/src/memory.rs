// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Per-person diary. A second `horbm-memory` file, not the truth-card diary.
//!
//! Pins, the extractive session summary, and name or mood lines from the
//! state tracker live here. Each row keeps who, when, and source. Search is
//! [`crate::search_person_lexical`]: FTS5 on this file only. Truth cards are
//! not ingested.
//!
//! `remember` defaults to deny. Denied means this type does not open the
//! file, does not call the sink, and does not run [`horbm_memory::summarise_session`].
//! Forget copies the row into the diary archive and drops it from search.
//! The archive is the tombstone. This module has no delete of that table.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use galion_emotion_core::standins::memory::{
    MemoryItem, MemoryKind, MemorySink, PersonMemory, PersonState, Tombstone, TrackedName,
    WriteOutcome,
};

use crate::retrieve::{covers, search_person_lexical, MemoryHit};

const SOURCE_PIN: &str = "pin";
const SOURCE_SUMMARY: &str = "session-summary";
const SOURCE_STATE: &str = "state-tracker";

/// The person diary. `remember` false does not create `path`.
pub struct EpisodicStore {
    remember: bool,
    path: PathBuf,
    store: Option<horbm_memory::Store>,
    sink: Option<Box<dyn MemorySink + Send>>,
    turns: Vec<BufferedTurn>,
    name_row: HashMap<String, (String, String)>,
    mood_row: HashMap<String, (String, String)>,
    summary_row: HashMap<String, String>,
    issued: HashMap<String, String>,
    writes: u64,
    extractions: u64,
}

struct BufferedTurn {
    person_id: String,
    said_by: String,
    text: String,
    at: String,
}

impl EpisodicStore {
    /// `remember` follows `LOCKS.toml` `[consent] remember`. `false` is deny.
    /// The file is not created until a granted write.
    pub fn open(path: impl AsRef<Path>, remember: bool) -> Self {
        Self {
            remember,
            path: path.as_ref().to_path_buf(),
            store: None,
            sink: None,
            turns: Vec::new(),
            name_row: HashMap::new(),
            mood_row: HashMap::new(),
            summary_row: HashMap::new(),
            issued: HashMap::new(),
            writes: 0,
            extractions: 0,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn writes(&self) -> u64 {
        self.writes
    }

    pub fn extractions(&self) -> u64 {
        self.extractions
    }

    pub fn buffered_turns(&self) -> usize {
        self.turns.len()
    }

    /// True when the snippet is still in a live row, the archive, or a revision.
    /// Forget keeps the snippet. A hard delete would make this false.
    pub fn holds_text(&self, snippet: &str) -> bool {
        let Some(store) = self.store.as_ref() else {
            return false;
        };
        store.retains_text(snippet).unwrap_or(false)
    }

    fn connect(&mut self, create: bool) -> bool {
        if !self.remember {
            return false;
        }
        if self.store.is_some() {
            return true;
        }
        if !create && !self.path.is_file() {
            return false;
        }
        match horbm_memory::Store::open(&self.path) {
            Ok(store) => {
                store.pause_worker();
                self.store = Some(store);
                true
            }
            Err(_) => false,
        }
    }

    fn settle(&mut self) -> bool {
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        let ok = store.drain().is_ok();
        store.pause_worker();
        ok
    }

    fn arm_session(&mut self, person_id: &str) {
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_session(person_id);
        }
    }

    fn emit(&mut self, item: &MemoryItem) {
        self.writes = self.writes.saturating_add(1);
        if let Some(sink) = self.sink.as_mut() {
            sink.on_stored(item);
        }
    }

    fn hide(&mut self, id: &str) -> bool {
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        let result = store.mark(id);
        store.pause_worker();
        match result {
            Ok(()) => true,
            Err(horbm_memory::Error::NotFound(_)) => true,
            Err(_) => false,
        }
    }

    fn owns(&self, person_id: &str, item_id: &str) -> bool {
        let Some(store) = self.store.as_ref() else {
            return false;
        };
        match store.document_of(item_id) {
            Ok(Some(document)) => json_said_by(&document).as_deref() == Some(person_id),
            Ok(None) => self.issued.get(item_id).map(String::as_str) == Some(person_id),
            Err(_) => false,
        }
    }

    fn write_diary(
        &mut self,
        text: &str,
        person_id: &str,
        at: &str,
        source: &str,
    ) -> Option<String> {
        self.arm_session(person_id);
        let id = self
            .store
            .as_mut()?
            .diary_add(text, at, person_id, source)
            .ok()?;
        if !self.settle() {
            return None;
        }
        self.issued.insert(id.clone(), person_id.to_string());
        Some(id)
    }
}

impl PersonMemory for EpisodicStore {
    fn remember_granted(&self) -> bool {
        self.remember
    }

    fn set_remember(&mut self, grant: bool) {
        if grant == self.remember {
            return;
        }
        if !grant {
            self.store = None;
            self.turns.clear();
            self.name_row.clear();
            self.mood_row.clear();
            self.summary_row.clear();
            self.issued.clear();
            self.remember = false;
            return;
        }
        self.remember = true;
    }

    fn set_sink(&mut self, sink: Box<dyn MemorySink + Send>) {
        self.sink = Some(sink);
    }

    fn pin(&mut self, person_id: &str, text: &str, at: &str) -> WriteOutcome {
        if !self.remember {
            return WriteOutcome::Refused;
        }
        let Some((person_id, text, at)) = required(person_id, text, at) else {
            return WriteOutcome::Skipped;
        };
        if looks_like_secret(text) {
            return WriteOutcome::Skipped;
        }
        if !self.connect(true) {
            return WriteOutcome::Failed;
        }
        self.arm_session(person_id);
        let id = match self
            .store
            .as_mut()
            .expect("connected")
            .remember(text, person_id, at, SOURCE_PIN)
        {
            Ok(id) => id,
            Err(_) => return WriteOutcome::Failed,
        };
        if !self.settle() {
            return WriteOutcome::Failed;
        }
        self.issued.insert(id.clone(), person_id.to_string());
        let item = item_from(
            id.clone(),
            person_id,
            MemoryKind::Pin,
            text,
            at,
            SOURCE_PIN,
            true,
        );
        self.emit(&item);
        WriteOutcome::Stored { id }
    }

    fn note_state(&mut self, person_id: &str, state: &PersonState, at: &str) -> WriteOutcome {
        if !self.remember {
            return WriteOutcome::Refused;
        }
        let person_id = person_id.trim();
        let at = at.trim();
        if person_id.is_empty() || at.is_empty() {
            return WriteOutcome::Skipped;
        }
        let name = name_line(&state.names);
        let mood = mood_line(state);
        let name_same = match (&name, self.name_row.get(person_id)) {
            (None, _) => true,
            (Some(line), Some((_, old))) => line == old,
            (Some(_), None) => false,
        };
        let mood_same = self
            .mood_row
            .get(person_id)
            .map(|(_, old)| old == &mood)
            .unwrap_or(false);
        if name_same && mood_same {
            return WriteOutcome::Skipped;
        }
        if !self.connect(true) {
            return WriteOutcome::Failed;
        }
        let mut last = String::new();
        if let Some(line) = name.as_ref().filter(|_| !name_same) {
            if let Some((old, _)) = self.name_row.get(person_id).cloned() {
                if !self.hide(&old) {
                    return WriteOutcome::Failed;
                }
            }
            let Some(id) = self.write_diary(line, person_id, at, SOURCE_STATE) else {
                return WriteOutcome::Failed;
            };
            self.name_row
                .insert(person_id.to_string(), (id.clone(), line.clone()));
            let item = item_from(
                id.clone(),
                person_id,
                MemoryKind::Name,
                line,
                at,
                SOURCE_STATE,
                false,
            );
            self.emit(&item);
            last = id;
        }
        if !mood_same {
            if let Some((old, _)) = self.mood_row.get(person_id).cloned() {
                if !self.hide(&old) {
                    return WriteOutcome::Failed;
                }
            }
            let Some(id) = self.write_diary(&mood, person_id, at, SOURCE_STATE) else {
                return WriteOutcome::Failed;
            };
            self.mood_row
                .insert(person_id.to_string(), (id.clone(), mood.clone()));
            let item = item_from(
                id.clone(),
                person_id,
                MemoryKind::Mood,
                &mood,
                at,
                SOURCE_STATE,
                false,
            );
            self.emit(&item);
            last = id;
        }
        if last.is_empty() {
            WriteOutcome::Skipped
        } else {
            WriteOutcome::Stored { id: last }
        }
    }

    fn note_turn(&mut self, person_id: &str, said_by: &str, text: &str, at: &str) -> WriteOutcome {
        if !self.remember {
            return WriteOutcome::Refused;
        }
        let Some((person_id, text, at)) = required(person_id, text, at) else {
            return WriteOutcome::Skipped;
        };
        if looks_like_secret(text) {
            return WriteOutcome::Skipped;
        }
        let said_by = said_by.trim();
        let said_by = if said_by.is_empty() {
            person_id
        } else {
            said_by
        };
        self.turns.push(BufferedTurn {
            person_id: person_id.to_string(),
            said_by: said_by.to_string(),
            text: text.to_string(),
            at: at.to_string(),
        });
        WriteOutcome::Stored { id: String::new() }
    }

    fn end_session(&mut self, person_id: &str, at: &str) -> WriteOutcome {
        if !self.remember {
            return WriteOutcome::Refused;
        }
        let person_id = person_id.trim();
        let at = at.trim();
        if person_id.is_empty() || at.is_empty() {
            return WriteOutcome::Skipped;
        }
        let turns: Vec<horbm_memory::SummaryTurn> = self
            .turns
            .iter()
            .filter(|turn| turn.person_id == person_id)
            .map(|turn| horbm_memory::SummaryTurn {
                said_by: turn.said_by.clone(),
                date: turn.at.clone(),
                text: turn.text.clone(),
            })
            .collect();
        if turns.is_empty() {
            return WriteOutcome::Skipped;
        }
        if !self.connect(true) {
            return WriteOutcome::Failed;
        }
        let summary = horbm_memory::summarise_session(person_id, &turns);
        self.extractions = self.extractions.saturating_add(1);
        if let Some(old) = self.summary_row.get(person_id).cloned() {
            if !self.hide(&old) {
                return WriteOutcome::Failed;
            }
        }
        let Some(id) = self.write_diary(&summary, person_id, at, SOURCE_SUMMARY) else {
            return WriteOutcome::Failed;
        };
        self.summary_row.insert(person_id.to_string(), id.clone());
        self.turns.retain(|turn| turn.person_id != person_id);
        if let Some(sink) = self.sink.as_mut() {
            sink.on_extracted(person_id, &summary);
        }
        let item = item_from(
            id.clone(),
            person_id,
            MemoryKind::Summary,
            &summary,
            at,
            SOURCE_SUMMARY,
            false,
        );
        self.emit(&item);
        WriteOutcome::Stored { id }
    }

    fn forget(&mut self, person_id: &str, item_id: &str) -> WriteOutcome {
        if !self.remember {
            return WriteOutcome::Refused;
        }
        let person_id = person_id.trim();
        let item_id = item_id.trim();
        if person_id.is_empty() || item_id.is_empty() {
            return WriteOutcome::Skipped;
        }
        if !self.connect(false) {
            return WriteOutcome::Skipped;
        }
        if !self.settle() {
            return WriteOutcome::Failed;
        }
        if !self.owns(person_id, item_id) {
            return WriteOutcome::Skipped;
        }
        let forgotten = {
            let Some(store) = self.store.as_mut() else {
                return WriteOutcome::Failed;
            };
            store.forget(item_id)
        };
        if let Some(store) = self.store.as_mut() {
            store.pause_worker();
        }
        if let Err(horbm_memory::Error::NotFound(_)) = forgotten {
            return WriteOutcome::Skipped;
        }
        if forgotten.is_err() {
            return WriteOutcome::Failed;
        }
        let stone = self.tombstone(item_id);
        if stone.as_ref().map(|row| row.reason.as_str()) != Some("forget") {
            return WriteOutcome::Failed;
        }
        self.issued.remove(item_id);
        self.name_row.retain(|_, (id, _)| id != item_id);
        self.mood_row.retain(|_, (id, _)| id != item_id);
        self.summary_row.retain(|_, id| id != item_id);
        WriteOutcome::Archived {
            id: item_id.to_string(),
        }
    }

    fn recall(&mut self, person_id: &str, query: &str) -> Vec<MemoryItem> {
        if !self.remember {
            return Vec::new();
        }
        let person_id = person_id.trim();
        if person_id.is_empty() {
            return Vec::new();
        }
        if !self.connect(false) {
            return Vec::new();
        }
        let Some(store) = self.store.as_ref() else {
            return Vec::new();
        };
        let Ok(hits) = search_person_lexical(store, query) else {
            return Vec::new();
        };
        let terms = horbm_memory::index_tokens(query);
        let mut items: Vec<MemoryItem> = hits
            .into_iter()
            .filter(|hit| hit.said_by == person_id)
            .filter(|hit| covers(&haystack(hit), &terms))
            .map(item_from_hit)
            .collect();
        items.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| overlap(&right.text, &terms).cmp(&overlap(&left.text, &terms)))
                .then_with(|| left.id.cmp(&right.id))
        });
        items.dedup_by(|left, right| left.id == right.id);
        items.truncate(crate::MEMORY_HIT_CAP);
        items
    }

    fn tombstone(&self, item_id: &str) -> Option<Tombstone> {
        let store = self.store.as_ref()?;
        let rows = store.archive_list().ok()?;
        rows.into_iter()
            .find(|row| row.item_id == item_id)
            .map(|row| Tombstone {
                id: row.item_id,
                person_id: row.said_by,
                text: row.text,
                at: row.date,
                source: row.source,
                reason: row.reason,
            })
    }
}

/// The line is about the person themself. A world question is not.
pub fn about_the_person(line: &str) -> bool {
    let lower = line.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    const MARKERS: &[&str] = &[
        "my name",
        "who am i",
        "what is my",
        "what's my",
        "whats my",
        "how am i",
        "how do i feel",
        "my mood",
        "my feeling",
        "about me",
        "what do you remember",
        "do you remember",
        "did i ask you to remember",
        "what did i ask",
    ];
    MARKERS.iter().any(|marker| lower.contains(marker))
}

/// Text after an explicit "remember …" command. `None` is not a pin.
pub fn pin_command(line: &str) -> Option<&str> {
    let line = line.trim();
    let lower = line.to_ascii_lowercase();
    let after = if let Some(rest) = lower.strip_prefix("please remember") {
        if starts_with_ident(rest) {
            return None;
        }
        &line["please remember".len()..]
    } else if let Some(rest) = lower.strip_prefix("remember") {
        if starts_with_ident(rest) {
            return None;
        }
        &line["remember".len()..]
    } else {
        return None;
    };
    let rest = trim_separators(after.trim_start());
    let rest = strip_this_or_that(rest);
    let rest = rest.trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest)
    }
}

fn starts_with_ident(rest: &str) -> bool {
    rest.starts_with(|ch: char| ch.is_ascii_alphanumeric())
}

fn trim_separators(text: &str) -> &str {
    text.trim_start_matches(|ch: char| matches!(ch, ':' | '-' | ',' | ';'))
        .trim_start()
}

fn strip_this_or_that(text: &str) -> &str {
    let lower = text.to_ascii_lowercase();
    for word in ["this", "that"] {
        if let Some(rest) = lower.strip_prefix(word) {
            if starts_with_ident(rest) {
                return text;
            }
            return trim_separators(&text[word.len()..]);
        }
    }
    text
}

fn required<'a>(
    person_id: &'a str,
    text: &'a str,
    at: &'a str,
) -> Option<(&'a str, &'a str, &'a str)> {
    let person_id = person_id.trim();
    let text = text.trim();
    let at = at.trim();
    if person_id.is_empty() || text.is_empty() || at.is_empty() {
        None
    } else {
        Some((person_id, text, at))
    }
}

fn item_from(
    id: String,
    person_id: &str,
    kind: MemoryKind,
    text: &str,
    at: &str,
    source: &str,
    pinned: bool,
) -> MemoryItem {
    MemoryItem {
        id,
        person_id: person_id.to_string(),
        kind,
        text: text.to_string(),
        at: at.to_string(),
        source: source.to_string(),
        pinned,
    }
}

fn item_from_hit(hit: MemoryHit) -> MemoryItem {
    let kind = kind_of(&hit);
    MemoryItem {
        id: hit.item_id,
        person_id: hit.said_by,
        kind,
        text: hit.text,
        at: hit.date,
        source: hit.source,
        pinned: hit.pinned,
    }
}

fn kind_of(hit: &MemoryHit) -> MemoryKind {
    match hit.source.as_str() {
        SOURCE_SUMMARY => MemoryKind::Summary,
        SOURCE_STATE if hit.text.starts_with("my name is") => MemoryKind::Name,
        SOURCE_STATE => MemoryKind::Mood,
        _ => MemoryKind::Pin,
    }
}

fn haystack(hit: &MemoryHit) -> String {
    format!("{} {} {}", hit.text, hit.said_by, hit.source)
}

fn overlap(text: &str, terms: &[String]) -> usize {
    let have = horbm_memory::index_tokens(text);
    terms
        .iter()
        .filter(|term| have.iter().any(|token| token == *term))
        .count()
}

fn json_said_by(document: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(document).ok()?;
    value
        .get("said_by")
        .and_then(|field| field.as_str())
        .map(str::to_string)
}

pub(crate) fn name_line(names: &[TrackedName]) -> Option<String> {
    let mut cleaned = Vec::new();
    for name in names {
        let Some(text) = clean_name(&name.name) else {
            continue;
        };
        let class = clean_class(&name.class_label);
        let key = text.to_ascii_lowercase();
        if cleaned.iter().any(|(have, _, _)| have == &key) {
            continue;
        }
        cleaned.push((key, text, class));
    }
    if cleaned.is_empty() {
        return None;
    }
    cleaned.sort_by(|left, right| left.0.cmp(&right.0).then(left.2.cmp(&right.2)));
    let rendered: Vec<String> = cleaned
        .into_iter()
        .map(|(_, text, class)| format!("{text} ({class})"))
        .collect();
    Some(format!("my name is {}", rendered.join(" and ")))
}

pub(crate) fn mood_line(state: &PersonState) -> String {
    format!(
        "i am feeling {} arousal {} valence {} dominance {}",
        clean_feeling(state.feeling.as_deref()),
        fmt3(state.arousal),
        fmt3(state.valence),
        fmt3(state.dominance)
    )
}

fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    let count = name.chars().count();
    if count == 0 || count > 64 {
        return None;
    }
    if !name.chars().any(|ch| ch.is_alphabetic()) {
        return None;
    }
    if name.chars().any(|ch| ch.is_control()) {
        return None;
    }
    if looks_like_secret(name) {
        return None;
    }
    Some(name.to_string())
}

fn clean_class(label: &str) -> &'static str {
    match label.trim().to_ascii_lowercase().as_str() {
        "he" => "he",
        "she" => "she",
        _ => "they",
    }
}

fn clean_feeling(feeling: Option<&str>) -> String {
    let mut out = String::new();
    for ch in feeling.unwrap_or("").chars() {
        if ch.is_ascii_alphabetic() {
            out.push(ch.to_ascii_lowercase());
        }
        if out.len() == 24 {
            break;
        }
    }
    if out.is_empty() {
        "steady".to_string()
    } else {
        out
    }
}

/// Same markers as `horbm-memory` `extract::looks_like_secret`. That function
/// is not re-exported. A pin or a buffered turn that matches is not written.
fn looks_like_secret(text: &str) -> bool {
    let lower = text.to_lowercase();
    [
        "password",
        "passphrase",
        "api key",
        "api_key",
        "apikey",
        "secret key",
        "access token",
        "private key",
        "credential",
        "social security",
        "credit card",
        "card number",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn fmt3(value: f32) -> String {
    if !value.is_finite() {
        return "0.000".to_string();
    }
    let scaled = (value * 1000.0).round();
    if !scaled.is_finite() {
        return "0.000".to_string();
    }
    let mut scaled = scaled as i64;
    let negative = scaled < 0;
    if negative {
        scaled = -scaled;
    }
    let whole = scaled / 1000;
    let frac = scaled % 1000;
    if negative {
        format!("-{whole}.{frac:03}")
    } else {
        format!("{whole}.{frac:03}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_questions_are_not_world_questions() {
        assert!(about_the_person("what is my favourite colour"));
        assert!(about_the_person("how am i feeling"));
        assert!(about_the_person("what is my name"));
        assert!(!about_the_person("where is the kettle"));
        assert!(!about_the_person("what is the capital of france"));
        assert!(!about_the_person(
            "remember that the capital of france is lyon"
        ));
    }

    #[test]
    fn remember_commands_keep_the_callers_words() {
        assert_eq!(
            pin_command("remember that my favourite colour is blue"),
            Some("my favourite colour is blue")
        );
        assert_eq!(
            pin_command("Please remember this: I prefer quiet mornings"),
            Some("I prefer quiet mornings")
        );
        assert_eq!(pin_command("remembering the song"), None);
        assert_eq!(pin_command("where is the kettle"), None);
    }

    #[test]
    fn name_and_mood_lines_are_stable() {
        let state = PersonState {
            names: vec![
                TrackedName {
                    name: "Sol".into(),
                    class_label: "she".into(),
                },
                TrackedName {
                    name: "Nia".into(),
                    class_label: "they".into(),
                },
            ],
            arousal: 0.3,
            valence: -0.25,
            dominance: 0.5,
            feeling: Some("Tired".into()),
        };
        assert_eq!(name_line(&state.names), name_line(&state.names));
        assert_eq!(
            name_line(&state.names).as_deref(),
            Some("my name is Nia (they) and Sol (she)")
        );
        assert_eq!(
            mood_line(&state),
            "i am feeling tired arousal 0.300 valence -0.250 dominance 0.500"
        );
    }
}
