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
//! [`EpisodicStore::open_from_flag`] grants only when the trimmed flag is
//! the word `grant`.
//!
//! [`EpisodicStore::note_turn`] only pushes the line into RAM. There is no
//! SQLite on that path. The buffer keeps the newest lines the memory crate's
//! session ring would keep, and drops the oldest. [`EpisodicStore::end_session`],
//! when `remember` is granted and the
//! buffer has turns, writes that tail once through
//! [`horbm_memory::Store::push_turn`] and [`horbm_memory::Store::drain`],
//! then writes the extractive summary the same way as before.
//!
//! A mood line is one diary row per feeling word. A later trace that only
//! moves the three decimals does not insert another row, hide the old one,
//! or reload the embedding arena. A pin whose normalised text is already
//! live is not written again.
//!
//! Forget copies the row into the diary archive and drops it from search.
//! The archive is the tombstone. This module has no delete of that table.
//!
//! `push_turn` also writes a `session-ring` candidate beside that diary row.
//! Recall drops a name candidate when a live or forgotten state-tracker line
//! names the same person, and drops a candidate whose normalised text matches
//! a pin. The diary row stays. Other candidates stay. Forget archives the
//! candidate twin of a pin or a name row. A kept name candidate is `Name`.
//! Any other kept candidate is `Fact`, with `pinned` left false.
//! `PersonMemory::forget_matching` is the trait default: recall, then forget
//! each hit. This type does not define its own copy.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use galion_emotion_core::standins::memory::{
    MemoryItem, MemoryKind, MemorySink, PersonMemory, PersonState, Tombstone, TrackedName,
    WriteOutcome,
};

use crate::retrieve::{covers, search_person_lexical, MemoryHit};

const SOURCE_PIN: &str = "pin";
const SOURCE_SUMMARY: &str = "session-summary";
const SOURCE_STATE: &str = "state-tracker";
const SOURCE_RING: &str = "session-ring";

/// Heard lines kept in RAM until [`EpisodicStore::end_session`].
///
/// Same number as [`horbm_memory::RING_CAP`]. A longer call drops the oldest
/// line from this process. It is not a second sqlite table.
const SESSION_TURN_CAP: usize = horbm_memory::RING_CAP;

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
    /// Name objects from a forgotten state-tracker line, lowercase.
    /// Recall hides a matching session-ring twin when that twin was not archived.
    hidden_names: HashMap<String, HashSet<String>>,
    /// Normalised text of a forgotten pin. Same fallback as `hidden_names`.
    hidden_pins: HashMap<String, HashSet<String>>,
    /// Live pin text, normalised, to the diary id. A repeat does not insert.
    pinned_norms: HashMap<String, HashMap<String, String>>,
    writes: u64,
    extractions: u64,
}

/// One `session_turns` row read back from the diary file.
#[derive(Debug)]
pub struct TranscriptTurn {
    pub id: String,
    pub text: String,
    pub said_by: String,
}

/// A read of `session_turns` after reopening the diary.
///
/// `ring_len` is [`horbm_memory::Store::session_len`], the RAM ring, empty
/// on a fresh open. `rows` is [`horbm_memory::Store::transcript_len`].
/// `extracted` is [`horbm_memory::Store::count_extracted`], candidate item
/// rows that `push_turn` queues. `turns` are the query hits, sorted by turn id.
#[derive(Debug)]
pub struct TranscriptFile {
    pub ring_len: usize,
    pub rows: usize,
    pub extracted: usize,
    pub turns: Vec<TranscriptTurn>,
}

struct BufferedTurn {
    person_id: String,
    said_by: String,
    text: String,
    at: String,
    /// Already written to `session_turns`. A later `end_session` does not
    /// append it again.
    flushed: bool,
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
            hidden_names: HashMap::new(),
            hidden_pins: HashMap::new(),
            pinned_norms: HashMap::new(),
            writes: 0,
            extractions: 0,
        }
    }

    /// `grant` after trim is the only grant. Any other flag, including
    /// `None`, calls [`Self::open`] with deny. The file is not created here.
    /// This does not read the process environment.
    pub fn open_from_flag(path: impl AsRef<Path>, flag: Option<&str>) -> Self {
        if flag.map(str::trim) == Some("grant") {
            Self::open(path, true)
        } else {
            Self::open(path, false)
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

    /// Diary rows share this key: the person id. It is not per call.
    /// [`Self::transcript_session_id`] is the transcript key, so two calls do not
    /// land in one `session_turns` session.
    fn arm_session(&mut self, person_id: &str) {
        if let Some(store) = self.store.as_mut() {
            let _ = store.set_session(person_id);
        }
    }

    /// Write this call's buffered lines into `session_turns`, then wait for
    /// that worker. Lines already flushed are left as they are. The caller
    /// has already connected. The summary write comes after this returns.
    fn flush_transcript(&mut self, person_id: &str, at: &str) -> bool {
        let pending: Vec<(String, String, String)> = self
            .turns
            .iter()
            .filter(|turn| turn.person_id == person_id && !turn.flushed)
            .map(|turn| (turn.text.clone(), turn.said_by.clone(), turn.at.clone()))
            .collect();
        if pending.is_empty() {
            return true;
        }
        let session_id = Self::transcript_session_id(person_id, at);
        let Some(store) = self.store.as_mut() else {
            return false;
        };
        if store.set_session(&session_id).is_err() {
            return false;
        }
        for (text, said_by, when) in pending {
            store.push_turn(text, said_by, when);
        }
        let drained = store.drain().is_ok();
        store.pause_worker();
        if !drained {
            return false;
        }
        for turn in &mut self.turns {
            if turn.person_id == person_id {
                turn.flushed = true;
            }
        }
        true
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

    /// Transcript session for one [`PersonMemory::end_session`]. The diary key is the
    /// person id alone, which would merge every call from that person. The
    /// stamp is the `at` argument of `end_session`, not a turn's own time.
    pub fn transcript_session_id(person_id: &str, at: &str) -> String {
        format!("{person_id}@{at}")
    }

    /// Reopen `path` with [`horbm_memory::Store`] and read `session_turns`.
    ///
    /// This does not write. `None` is an open or read error.
    pub fn read_transcript_file(path: &Path, query: &str) -> Option<TranscriptFile> {
        let store = horbm_memory::Store::open(path).ok()?;
        let ring_len = store.session_len();
        let rows = store.transcript_len().ok()?;
        let extracted = store.count_extracted().ok()?;
        let response = store.search_transcript(query).ok()?;
        let mut turns: Vec<TranscriptTurn> = response
            .hits
            .into_iter()
            .map(|hit| TranscriptTurn {
                id: hit.item_id,
                text: hit.text,
                said_by: hit.said_by,
            })
            .collect();
        turns.sort_by(|left, right| {
            turn_ord(&left.id)
                .cmp(&turn_ord(&right.id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Some(TranscriptFile {
            ring_len,
            rows,
            extracted,
            turns,
        })
    }

    /// Remember a forgotten pin or name so recall can hide a twin the archive
    /// call did not take. Mood and summary rows have no candidate twin.
    fn suppress_forgotten(&mut self, person_id: &str, source: &str, text: &str) {
        if source == SOURCE_PIN {
            let norm = normalise_fact(text);
            if !norm.is_empty() {
                self.hidden_pins
                    .entry(person_id.to_string())
                    .or_default()
                    .insert(norm);
            }
            return;
        }
        if source == SOURCE_STATE && text.starts_with("my name is") {
            let slot = self.hidden_names.entry(person_id.to_string()).or_default();
            for name in names_named_by(text) {
                slot.insert(name);
            }
        }
    }

    /// Archive `cand-*` rows that repeat this pin or name line. `owns` reads
    /// `said_by` out of the candidate document. A twin that cannot be archived
    /// stays out of recall through `suppress_forgotten`.
    fn archive_twins(&mut self, person_id: &str, source: &str, text: &str) {
        if source != SOURCE_PIN && !(source == SOURCE_STATE && text.starts_with("my name is")) {
            return;
        }
        let mut queries = vec![text.to_string()];
        if source == SOURCE_STATE {
            queries.extend(names_named_by(text));
        }
        let mut ids = Vec::new();
        for query in &queries {
            if query.is_empty() {
                continue;
            }
            let Some(store) = self.store.as_ref() else {
                return;
            };
            let Ok(hits) = search_person_lexical(store, query) else {
                continue;
            };
            for hit in hits {
                if hit.said_by != person_id || hit.source != SOURCE_RING {
                    continue;
                }
                if !hit.item_id.starts_with("cand-") {
                    continue;
                }
                if !candidate_matches(
                    &hit.text,
                    &hit.said_by,
                    &hit.date,
                    &hit.source,
                    source,
                    text,
                ) {
                    continue;
                }
                if !ids.iter().any(|id: &String| id == &hit.item_id) {
                    ids.push(hit.item_id);
                }
            }
        }
        for id in ids {
            if !self.owns(person_id, &id) {
                continue;
            }
            let _archived = {
                let Some(store) = self.store.as_mut() else {
                    return;
                };
                store.forget(&id).is_ok()
            };
            if let Some(store) = self.store.as_mut() {
                store.pause_worker();
            }
        }
    }

    /// Drop session-ring twins on the hit list, before the recall cap.
    /// A live name line in `name_row` counts even when this query did not
    /// return that row. Forgotten lines count through the in-memory sets.
    ///
    /// Search keeps only [`crate::MEMORY_HIT_CAP`] rows, and a pin outranks a
    /// diary name line. The candidate can fill that window while the name
    /// line is left out. After the candidate is dropped, put the name line
    /// back when it still covers the query.
    fn drop_duplicate_candidates(
        &self,
        person_id: &str,
        items: &mut Vec<MemoryItem>,
        terms: &[String],
    ) {
        let mut live_names = Vec::new();
        if let Some((_, line)) = self.name_row.get(person_id) {
            live_names.extend(names_named_by(line));
        }
        for item in items.iter() {
            if item.source == SOURCE_STATE && item.text.starts_with("my name is") {
                for name in names_named_by(&item.text) {
                    if !live_names.iter().any(|have| have == &name) {
                        live_names.push(name);
                    }
                }
            }
        }
        let live_pins: Vec<String> = items
            .iter()
            .filter(|item| item.source == SOURCE_PIN)
            .map(|item| normalise_fact(&item.text))
            .filter(|text| !text.is_empty())
            .collect();
        let forgotten_names = self.hidden_names.get(person_id);
        let forgotten_pins = self.hidden_pins.get(person_id);
        items.retain(|item| {
            if item.source != SOURCE_RING {
                return true;
            }
            let objects = name_slot_objects(&item.text, &item.person_id, &item.at, &item.source);
            if objects.iter().any(|object| {
                live_names.iter().any(|name| name == object)
                    || forgotten_names.is_some_and(|set| set.contains(object))
            }) {
                return false;
            }
            let norm = normalise_fact(&item.text);
            if !norm.is_empty()
                && (live_pins.iter().any(|pin| pin == &norm)
                    || forgotten_pins.is_some_and(|set| set.contains(&norm)))
            {
                return false;
            }
            true
        });
        self.restore_covered_name(person_id, items, terms);
    }

    /// The live state-tracker name line, when this query covers it and search
    /// did not already return that id.
    fn restore_covered_name(&self, person_id: &str, items: &mut Vec<MemoryItem>, terms: &[String]) {
        let Some((id, line)) = self
            .name_row
            .get(person_id)
            .map(|(id, line)| (id.clone(), line.clone()))
        else {
            return;
        };
        if items.iter().any(|item| item.id == id) {
            return;
        }
        let haystack = format!("{line} {person_id} {SOURCE_STATE}");
        if !covers(&haystack, terms) {
            return;
        }
        let at = self
            .store
            .as_ref()
            .and_then(|store| store.document_of(&id).ok().flatten())
            .and_then(|document| json_string(&document, "date"))
            .unwrap_or_default();
        items.push(item_from(
            id,
            person_id,
            MemoryKind::Name,
            &line,
            &at,
            SOURCE_STATE,
            false,
        ));
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
            self.hidden_names.clear();
            self.hidden_pins.clear();
            self.pinned_norms.clear();
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
        let norm = normalise_fact(text);
        if let Some(id) = self
            .pinned_norms
            .get(person_id)
            .and_then(|norms| norms.get(&norm))
            .cloned()
        {
            return WriteOutcome::Stored { id };
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
        self.pinned_norms
            .entry(person_id.to_string())
            .or_default()
            .insert(norm, id.clone());
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
        // The feeling word is the row. The three decimals move on every
        // empathy step, and each new row stays in the sqlite page cache
        // after `mark` hides it from search.
        let mood_same = self
            .mood_row
            .get(person_id)
            .is_some_and(|(_, old)| feeling_key(old) == feeling_key(&mood));
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
        if self.turns.len() == SESSION_TURN_CAP {
            self.turns.remove(0);
        }
        self.turns.push(BufferedTurn {
            person_id: person_id.to_string(),
            said_by: said_by.to_string(),
            text: text.to_string(),
            at: at.to_string(),
            flushed: false,
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
        if !self.flush_transcript(person_id, at) {
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
        let Some(stone) = self.tombstone(item_id) else {
            return WriteOutcome::Failed;
        };
        if stone.reason != "forget" {
            return WriteOutcome::Failed;
        }
        self.issued.remove(item_id);
        if stone.source == SOURCE_PIN {
            let norm = normalise_fact(&stone.text);
            if let Some(norms) = self.pinned_norms.get_mut(person_id) {
                norms.remove(&norm);
            }
        }
        self.name_row.retain(|_, (id, _)| id != item_id);
        self.mood_row.retain(|_, (id, _)| id != item_id);
        self.summary_row.retain(|_, id| id != item_id);
        self.suppress_forgotten(person_id, &stone.source, &stone.text);
        self.archive_twins(person_id, &stone.source, &stone.text);
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
        rank_hits(&mut items, &terms);
        items.dedup_by(|left, right| left.id == right.id);
        self.drop_duplicate_candidates(person_id, &mut items, &terms);
        rank_hits(&mut items, &terms);
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

fn turn_ord(id: &str) -> u64 {
    id.rsplit_once('-')
        .and_then(|(_, tail)| tail.parse().ok())
        .unwrap_or(0)
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
    text.trim_start_matches([':', '-', ',', ';']).trim_start()
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
        // A name slot stays Name. Any other kept session-ring candidate is
        // an extracted statement, not a pin. `pinned` stays whatever the hit
        // carried, which is false for a candidate.
        SOURCE_RING if has_name_slot(&hit.text, &hit.said_by, &hit.date, &hit.source) => {
            MemoryKind::Name
        }
        SOURCE_RING => MemoryKind::Fact,
        _ => MemoryKind::Pin,
    }
}

fn has_name_slot(text: &str, said_by: &str, date: &str, source: &str) -> bool {
    !name_slot_objects(text, said_by, date, source).is_empty()
}

fn name_slot_objects(text: &str, said_by: &str, date: &str, source: &str) -> Vec<String> {
    horbm_memory::FactExtractor::extract(
        &horbm_memory::BuiltinExtractor,
        text,
        said_by,
        date,
        source,
    )
    .into_iter()
    .filter(|candidate| candidate.subject_key == "name")
    .map(|candidate| candidate.object_value.to_ascii_lowercase())
    .collect()
}

fn candidate_matches(
    text: &str,
    said_by: &str,
    date: &str,
    source: &str,
    diary_source: &str,
    diary_text: &str,
) -> bool {
    if diary_source == SOURCE_STATE && diary_text.starts_with("my name is") {
        let names = names_named_by(diary_text);
        return name_slot_objects(text, said_by, date, source)
            .iter()
            .any(|object| names.iter().any(|name| name == object));
    }
    if diary_source == SOURCE_PIN {
        let left = normalise_fact(text);
        let right = normalise_fact(diary_text);
        return !left.is_empty() && left == right;
    }
    false
}

/// Names inside a state-tracker `name_line`: `my name is {Name} ({class})`,
/// joined with ` and `.
fn names_named_by(line: &str) -> Vec<String> {
    let Some(rest) = line.strip_prefix("my name is ") else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut rest = rest;
    while !rest.is_empty() {
        let Some((end, suffix_len)) = class_suffix(rest) else {
            break;
        };
        let name = rest[..end].trim();
        if !name.is_empty() {
            names.push(name.to_ascii_lowercase());
        }
        rest = rest[end + suffix_len..].trim_start();
        if let Some(next) = rest.strip_prefix("and ") {
            rest = next;
        } else {
            break;
        }
    }
    names
}

fn class_suffix(text: &str) -> Option<(usize, usize)> {
    const SUFFIXES: &[&str] = &[" (he)", " (she)", " (they)"];
    SUFFIXES
        .iter()
        .filter_map(|suffix| text.find(suffix).map(|index| (index, suffix.len())))
        .min_by_key(|(index, _)| *index)
}

/// ASCII lowercase, whitespace collapsed, edge punctuation removed, then a
/// leading `please remember` / `remember this` / `remember` taken off.
fn normalise_fact(text: &str) -> String {
    let folded = collapse_ws_lower(text);
    let trimmed = trim_ascii_punctuation(folded.trim()).trim();
    strip_remember_prefix(trimmed)
}

fn collapse_ws_lower(text: &str) -> String {
    let mut out = String::new();
    let mut pending_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn trim_ascii_punctuation(text: &str) -> &str {
    text.trim_matches(|ch: char| ch.is_ascii_punctuation())
}

fn strip_remember_prefix(text: &str) -> String {
    const PREFIXES: &[&str] = &["please remember", "remember this", "remember"];
    for prefix in PREFIXES {
        let Some(rest) = text.strip_prefix(prefix) else {
            continue;
        };
        if rest.starts_with(|ch: char| ch.is_ascii_alphanumeric()) {
            continue;
        }
        let rest = collapse_ws_lower(rest.trim());
        return trim_ascii_punctuation(rest.trim()).trim().to_string();
    }
    text.to_string()
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
    json_string(document, "said_by")
}

fn json_string(document: &str, key: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(document).ok()?;
    value
        .get(key)
        .and_then(|field| field.as_str())
        .map(str::to_string)
}

fn rank_hits(items: &mut [MemoryItem], terms: &[String]) {
    items.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| overlap(&right.text, terms).cmp(&overlap(&left.text, terms)))
            .then_with(|| left.id.cmp(&right.id))
    });
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
    cleaned.sort_by(|left, right| left.0.cmp(&right.0).then(left.2.cmp(right.2)));
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

/// The word after `i am feeling`, before the axis trace.
fn feeling_key(line: &str) -> &str {
    let rest = line.strip_prefix("i am feeling ").unwrap_or(line);
    rest.split(" arousal ").next().unwrap_or(rest).trim()
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

    #[test]
    fn normalised_pin_text_matches_a_remember_this_line() {
        assert_eq!(
            normalise_fact("remember this my sister is Mira"),
            normalise_fact("my sister is Mira")
        );
        assert_eq!(
            normalise_fact("  Please   remember this: I prefer quiet mornings! "),
            "this: i prefer quiet mornings"
        );
        assert_eq!(
            normalise_fact("remembering the song"),
            "remembering the song"
        );
        assert_eq!(
            names_named_by("my name is Nia (they) and Sol (she)"),
            vec!["nia".to_string(), "sol".to_string()]
        );
        assert!(names_named_by("my name is Ana").is_empty());
    }

    #[test]
    fn candidate_document_carries_said_by() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("horbm-f7-cand-doc-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch");
        let path = dir.join("person-memory.sqlite");
        {
            let mut store = EpisodicStore::open(&path, true);
            let noted = store.note_turn("northwind", "northwind", "I live in Split", "t000001");
            assert!(matches!(noted, WriteOutcome::Stored { .. }), "{noted:?}");
            let ended = store.end_session("northwind", "t000099");
            assert!(matches!(ended, WriteOutcome::Stored { .. }), "{ended:?}");
        }
        let opened = horbm_memory::Store::open(&path).expect("reopen");
        let found = opened.search_lexical("Split").expect("search");
        let cand = found
            .hits
            .iter()
            .find(|hit| hit.item_id.starts_with("cand-"))
            .expect("candidate id");
        let document = opened
            .document_of(&cand.item_id)
            .expect("document read")
            .expect("candidate document");
        let value: serde_json::Value = serde_json::from_str(&document).expect("json");
        assert_eq!(value["said_by"], "northwind");
        assert_eq!(value["source"], "session-ring");
        assert_eq!(value["kind"], "candidate");
        drop(opened);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
