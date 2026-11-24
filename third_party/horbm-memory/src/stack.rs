// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! One query over what the user told us and the knowledge cards.
//!
//! Memory (the RAM ring, episodes from `remember`, the append-only diary)
//! and knowledge (the core pack and the shards) are searched apart and
//! returned in separate lists. A memory hit is never given a card's
//! claim type, and a card is never returned as something the user said.
//! Scores are not fused across the two pools.
//!
//! When the knowledge side has no confident hit, the answer is
//! `I don't know`. The miss is enqueued for the gap log. The insert
//! itself runs on the memory worker, not inside this call.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::card::{card_from_document, parse_cards, Card};
use crate::error::{Error, Result};
use crate::scale::{dir_bytes, ScaleIndex, CATEGORIES};
use crate::store::{GapRank, Hit, Store, MAX_HITS};
use crate::text::{index_tokens, is_stopword};

const ANSWER_UNKNOWN: &str = "I don't know";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ImportReport {
    pub files_read: usize,
    pub cards: usize,
    pub inserted: usize,
    pub revised: usize,
    pub unchanged: usize,
    pub archived_absent: usize,
    pub core_rows: usize,
    pub shard_count: usize,
    pub rebuilt: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct QueryResponse {
    pub query: String,
    /// World knowledge. `I don't know` when no card covers the query.
    /// A memory hit does not fill this in.
    pub answer: String,
    pub confident: bool,
    /// Cards from the core pack and the shards. At most four, together with `memory`.
    pub knowledge: Vec<Hit>,
    /// What the user told us: the ring, an episode, or a diary line.
    pub memory: Vec<Hit>,
    /// Cards that were close when the answer is `I don't know`.
    pub nearest: Vec<Hit>,
    pub gap_id: Option<i64>,
    pub took_us: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct StoredRevision {
    pub item_id: String,
    pub content_hash: String,
    pub document: String,
    pub archived_at: String,
    pub reason: String,
}

/// Memory database plus the knowledge directory (core, shards, revisions).
pub struct Stack {
    memory: Store,
    root: PathBuf,
    index: Option<ScaleIndex>,
    rev: Connection,
}

impl Stack {
    pub fn open(memory_db: impl AsRef<Path>, knowledge_dir: impl AsRef<Path>) -> Result<Self> {
        let root = knowledge_dir.as_ref().to_path_buf();
        fs::create_dir_all(&root)?;
        let memory = Store::open(memory_db)?;
        let rev = open_revisions(&root.join("revisions.sqlite"))?;
        let index_dir = root.join("index");
        let index = if index_dir.join("manifest.json").is_file() {
            Some(ScaleIndex::open(&index_dir)?)
        } else {
            None
        };
        Ok(Self {
            memory,
            root,
            index,
            rev,
        })
    }

    pub fn import_dir(&mut self, dir: impl AsRef<Path>) -> Result<ImportReport> {
        let cards = read_card_dir(dir.as_ref())?;
        let files = cards.len();
        let mut report = self.import_cards(cards, false)?;
        report.files_read = files;
        Ok(report)
    }

    /// Same as import, and the index is rebuilt even when every hash matches.
    pub fn rebuild_dir(&mut self, dir: impl AsRef<Path>) -> Result<ImportReport> {
        let cards = read_card_dir(dir.as_ref())?;
        let files = cards.len();
        let mut report = self.import_cards(cards, true)?;
        report.files_read = files;
        Ok(report)
    }

    /// Load these cards into the core pack and the shards.
    ///
    /// Same content is a no-op. Changed content appends the previous
    /// document to the revision archive and then rebuilds the index.
    /// A card that is no longer in the set is archived the same way.
    /// Search reads the rebuilt index, which has only the latest text.
    pub fn import_cards(&mut self, cards: Vec<Card>, force: bool) -> Result<ImportReport> {
        let mut by_id: HashMap<String, Card> = HashMap::new();
        for card in cards {
            by_id.insert(card.id.clone(), card);
        }
        let cards: Vec<Card> = by_id.into_values().collect();
        let heads = live_heads(&self.rev)?;
        let mut inserted = 0usize;
        let mut revised = 0usize;
        let mut unchanged = 0usize;
        let mut seen = HashMap::new();
        let mut to_archive: Vec<(String, String)> = Vec::new();
        for card in &cards {
            seen.insert(card.id.clone(), ());
            match heads.get(&card.id) {
                Some(hash) if hash == &card.content_hash => unchanged += 1,
                Some(_) => {
                    revised += 1;
                    to_archive.push((card.id.clone(), "replaced".to_string()));
                }
                None => inserted += 1,
            }
        }
        let mut archived_absent = 0usize;
        for id in heads.keys() {
            if !seen.contains_key(id) {
                archived_absent += 1;
                to_archive.push((id.clone(), "absent".to_string()));
            }
        }
        let dirty = revised > 0 || inserted > 0 || archived_absent > 0;
        if !dirty && !force && self.index.is_some() {
            let index = self.index.as_ref().unwrap();
            return Ok(ImportReport {
                files_read: 0,
                cards: cards.len(),
                inserted: 0,
                revised: 0,
                unchanged,
                archived_absent: 0,
                core_rows: index.core_rows(),
                shard_count: index.shard_count(),
                rebuilt: false,
            });
        }
        for (id, reason) in &to_archive {
            if let Some(index) = &self.index {
                if let Some(document) = index.document_of(id)? {
                    let hash = crate::card::sha256_hex(document.as_bytes());
                    insert_revision(&self.rev, id, &hash, &document, reason)?;
                }
            }
        }
        let index_dir = self.root.join("index");
        // Drop the open shards before the directory is replaced.
        self.index = None;
        let index = ScaleIndex::build(&index_dir, cards.iter().cloned())?;
        write_heads(
            &self.rev,
            cards
                .iter()
                .map(|card| (card.id.as_str(), card.content_hash.as_str())),
        )?;
        let report = ImportReport {
            files_read: 0,
            cards: cards.len(),
            inserted,
            revised,
            unchanged,
            archived_absent,
            core_rows: index.core_rows(),
            shard_count: index.shard_count(),
            rebuilt: true,
        };
        self.index = Some(index);
        Ok(report)
    }

    /// Replace one card. The previous document is archived. The index is republished.
    pub fn apply_card(&mut self, card: &Card) -> Result<ImportReport> {
        let mut cards = self.loaded_cards()?;
        if let Some(slot) = cards.iter_mut().find(|existing| existing.id == card.id) {
            *slot = card.clone();
        } else {
            cards.push(card.clone());
        }
        self.import_cards(cards, false)
    }

    pub fn latest_card(&self, id: &str) -> Result<Card> {
        let Some(index) = &self.index else {
            return Err(Error::NotFound(id.to_string()));
        };
        let Some(document) = index.document_of(id)? else {
            return Err(Error::NotFound(id.to_string()));
        };
        card_from_document(&document)
    }

    pub fn revisions(&self, id: &str) -> Result<Vec<StoredRevision>> {
        let mut stmt = self.rev.prepare(
            "SELECT item_id, content_hash, document, archived_at, reason
             FROM card_revisions WHERE item_id = ?1 ORDER BY revision_id ASC",
        )?;
        let rows = stmt.query_map(params![id], |row| {
            Ok(StoredRevision {
                item_id: row.get(0)?,
                content_hash: row.get(1)?,
                document: row.get(2)?,
                archived_at: row.get(3)?,
                reason: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn revision_count(&self) -> Result<usize> {
        let n: i64 = self
            .rev
            .query_row("SELECT COUNT(*) FROM card_revisions", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    pub fn unload_shard(&mut self, id: &str) -> bool {
        self.index
            .as_mut()
            .is_some_and(|index| index.unload_shard(id))
    }

    pub fn query(&mut self, query: &str) -> Result<QueryResponse> {
        let started = Instant::now();
        let memory_hits = self.memory.search_memory(query)?.hits;
        let (knowledge_hits, unloaded) = if let Some(index) = &mut self.index {
            let unloaded = index.unloaded_shards(query);
            let hits = index.search(query)?.hits;
            (hits, unloaded)
        } else {
            (Vec::new(), Vec::new())
        };
        let terms = confident_terms(query);
        let knowledge_ok: Vec<Hit> = knowledge_hits
            .iter()
            .filter(|hit| covers(hit, &terms))
            .cloned()
            .collect();
        let memory_ok: Vec<Hit> = memory_hits
            .iter()
            .filter(|hit| covers(hit, &terms))
            .cloned()
            .collect();
        let (knowledge, memory) = cap_lists(knowledge_ok, memory_ok);
        let confident = !knowledge.is_empty();
        let (answer, gap_id, nearest) = if confident {
            (String::new(), None, Vec::new())
        } else {
            let nearest = knowledge_hits
                .into_iter()
                .take(MAX_HITS)
                .collect::<Vec<_>>();
            let (advice, detail) = advice_for(&unloaded, &nearest, self.index.is_some());
            let nearest_json = nearest_json(&nearest);
            self.memory.enqueue_gap(
                query,
                if nearest.is_empty() {
                    "none"
                } else {
                    "low_confidence"
                },
                &nearest_json,
                advice,
                &detail,
            );
            (ANSWER_UNKNOWN.to_string(), None, nearest)
        };
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        Ok(QueryResponse {
            query: query.to_string(),
            answer,
            confident,
            knowledge,
            memory,
            nearest,
            gap_id,
            took_us,
        })
    }

    pub fn gap_queue(&self) -> Result<Vec<GapRank>> {
        self.memory.gap_queue()
    }

    pub fn push_turn(
        &mut self,
        text: impl Into<String>,
        said_by: impl Into<String>,
        date: impl Into<String>,
    ) -> crate::session::Turn {
        self.memory.push_turn(text, said_by, date)
    }

    pub fn set_session(&mut self, session_id: &str) -> Result<()> {
        self.memory.set_session(session_id)
    }

    pub fn keep_whole_session(&mut self, session_id: &str, keep: bool) -> Result<()> {
        self.memory.keep_whole_session(session_id, keep)
    }

    pub fn session_summary(&self, session_id: &str) -> Result<String> {
        self.memory.session_summary(session_id)
    }

    pub fn revise_fact(
        &mut self,
        old_id: &str,
        text: &str,
        said_by: &str,
        date: &str,
        source: &str,
    ) -> Result<String> {
        self.memory.revise_fact(old_id, text, said_by, date, source)
    }

    pub fn search_transcript(&self, query: &str) -> Result<crate::store::SearchResponse> {
        self.memory.search_transcript(query)
    }

    pub fn pause_worker(&self) {
        self.memory.pause_worker();
    }

    pub fn resume_worker(&self) {
        self.memory.resume_worker();
    }

    pub fn queue_len(&self) -> usize {
        self.memory.queue_len()
    }

    pub fn queue_handle(&self) -> crate::queue::QueueHandle {
        self.memory.queue_handle()
    }

    /// Finish background writes. Tests and the CLI use this. `query` does not.
    pub fn drain(&mut self) -> Result<()> {
        self.memory.drain()
    }

    pub fn remember(
        &mut self,
        text: &str,
        said_by: &str,
        date: &str,
        source: &str,
    ) -> Result<String> {
        self.memory.remember(text, said_by, date, source)
    }

    pub fn diary_add(
        &mut self,
        text: &str,
        date: &str,
        said_by: &str,
        source: &str,
    ) -> Result<String> {
        self.memory.diary_add(text, date, said_by, source)
    }

    pub fn diary_list(&self) -> Result<Vec<crate::store::DiaryEntry>> {
        self.memory.diary_list()
    }

    pub fn forget(&mut self, item_id: &str) -> Result<()> {
        self.memory.forget(item_id)
    }

    pub fn mark(&mut self, item_id: &str) -> Result<()> {
        self.memory.mark(item_id)
    }

    pub fn core_rows(&self) -> usize {
        self.index
            .as_ref()
            .map(|index| index.core_rows())
            .unwrap_or(0)
    }

    pub fn shard_count(&self) -> usize {
        self.index
            .as_ref()
            .map(|index| index.shard_count())
            .unwrap_or(0)
    }

    pub fn rows(&self) -> usize {
        self.index.as_ref().map(|index| index.rows()).unwrap_or(0)
    }

    pub fn disk_bytes(&self) -> Result<u64> {
        dir_bytes(&self.root)
    }

    pub fn clear_cache(&mut self) {
        if let Some(index) = &mut self.index {
            index.clear_cache();
        }
    }

    fn loaded_cards(&self) -> Result<Vec<Card>> {
        let Some(index) = &self.index else {
            return Ok(Vec::new());
        };
        let mut cards = Vec::new();
        for document in index.documents()? {
            cards.push(card_from_document(&document)?);
        }
        Ok(cards)
    }
}

/// A generated card for the size test. `i % 5 == 0` is a shard card.
/// The other four fifths are core (`fact` and `strong`).
pub fn synthetic_card(i: usize) -> Card {
    let core = i % 5 != 0;
    let shard_cats = ["history", "biology", "philosophy", "physics"];
    let category = if core {
        CATEGORIES[i % CATEGORIES.len()]
    } else {
        shard_cats[i % shard_cats.len()]
    };
    let raw = serde_json::json!({
        "id": format!("sample-{i}"),
        "title": format!("Sample {i} {category}"),
        "section": "world",
        "category": category,
        "summary": format!("Synthetic sample {i} about {category}. marker m{i}."),
        "body": format!("This generated card {i} exists so the benchmark has a corpus. Topic {category}. marker m{i}."),
        "claim_type": if core { "fact" } else { "opinion" },
        "evidence_strength": if core { "strong" } else { "weak" },
        "sources": [{
            "title": "horbm-memory benchmark fixture",
            "url": "https://github.com/galion-studio/galion-knowledge",
            "author": "Galion Labs",
            "date": "2026-11-18",
            "accessed": "2026-11-18"
        }],
        "said_by": "Galion Labs",
        "as_of_date": "2026-11-18",
        "status": "draft",
        "contested_views": [],
        "related": [],
        "tags": ["sample"]
    });
    parse_cards(&raw, "world/sample.json")
        .expect("sample card")
        .pop()
        .expect("one card")
}

fn advice_for(unloaded: &[String], nearest: &[Hit], index_loaded: bool) -> (&'static str, String) {
    if let Some(id) = unloaded.first() {
        return (
            "load_shard",
            format!("Load shard {id}. It is in the manifest and not loaded."),
        );
    }
    if !nearest.is_empty() {
        let ids = nearest
            .iter()
            .map(|hit| hit.item_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return ("revise_card", format!("Write or revise: {ids}."));
    }
    if index_loaded {
        (
            "ask_galion_research",
            "Ask Galion Research. Write a card for this query.".to_string(),
        )
    } else {
        (
            "ask_galion_research",
            "Ask Galion Research. No knowledge index is loaded.".to_string(),
        )
    }
}

fn nearest_json(nearest: &[Hit]) -> String {
    let rows: Vec<serde_json::Value> = nearest
        .iter()
        .map(|hit| {
            serde_json::json!({
                "id": hit.item_id,
                "title": hit.title,
                "layer": hit.layer,
                "pool": hit.pool,
                "score": hit.score,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string())
}

fn confident_terms(query: &str) -> Vec<String> {
    index_tokens(query)
        .into_iter()
        .filter(|term| !is_stopword(term))
        .collect()
}

fn covers(hit: &Hit, terms: &[String]) -> bool {
    if terms.is_empty() {
        return false;
    }
    let hay: std::collections::HashSet<String> = index_tokens(&hit.haystack).into_iter().collect();
    terms.iter().all(|term| hay.contains(term))
}

/// At most four hits in total. When both pools have a hit, each keeps at least one slot.
fn cap_lists(mut knowledge: Vec<Hit>, mut memory: Vec<Hit>) -> (Vec<Hit>, Vec<Hit>) {
    if knowledge.is_empty() {
        memory.truncate(MAX_HITS);
        return (knowledge, memory);
    }
    if memory.is_empty() {
        knowledge.truncate(MAX_HITS);
        return (knowledge, memory);
    }
    knowledge.truncate(MAX_HITS);
    memory.truncate(MAX_HITS);
    if knowledge.len() + memory.len() > MAX_HITS {
        let keep_knowledge = (MAX_HITS - 1).min(knowledge.len());
        knowledge.truncate(keep_knowledge);
        memory.truncate(MAX_HITS - knowledge.len());
    }
    (knowledge, memory)
}

fn read_card_dir(dir: &Path) -> Result<Vec<Card>> {
    if !dir.is_dir() {
        return Err(Error::Invalid(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let mut files = Vec::new();
    collect_json(dir, &mut files)?;
    files.sort();
    let mut cards = Vec::new();
    for path in files {
        let raw = fs::read_to_string(&path)?;
        let value: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|err| Error::Invalid(format!("{}: {err}", path.display())))?;
        cards.extend(parse_cards(&value, &path.to_string_lossy())?);
    }
    Ok(cards)
}

fn collect_json(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

fn open_revisions(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS card_revisions (
            revision_id INTEGER PRIMARY KEY,
            item_id TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            document TEXT NOT NULL,
            archived_at TEXT NOT NULL,
            reason TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS card_revisions_once
            ON card_revisions(item_id, content_hash);
        CREATE TABLE IF NOT EXISTS heads (
            item_id TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            live INTEGER NOT NULL
        );",
    )?;
    Ok(conn)
}

fn live_heads(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT item_id, content_hash FROM heads WHERE live = 1")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out = HashMap::new();
    for row in rows {
        let (id, hash) = row?;
        out.insert(id, hash);
    }
    Ok(out)
}

fn insert_revision(
    conn: &Connection,
    item_id: &str,
    content_hash: &str,
    document: &str,
    reason: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO card_revisions (
            item_id, content_hash, document, archived_at, reason
         ) VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?4)",
        params![item_id, content_hash, document, reason],
    )?;
    Ok(())
}

fn write_heads<'a>(
    conn: &Connection,
    cards: impl Iterator<Item = (&'a str, &'a str)>,
) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        conn.execute("UPDATE heads SET live = 0", [])?;
        for (id, hash) in cards {
            conn.execute(
                "INSERT INTO heads (item_id, content_hash, live) VALUES (?1, ?2, 1)
                 ON CONFLICT(item_id) DO UPDATE SET content_hash = excluded.content_hash, live = 1",
                params![id, hash],
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}
