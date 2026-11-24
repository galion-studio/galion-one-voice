// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! In-process FTS5. The virtual table matches `horbm-memory` `store.rs`
//! (`items_fts`: title, summary, body, said_by, source, unicode61) at
//! galion-knowledge `761a7f225304fc010379219e5b88c6669dd48f40`.
//!
//! The int8 vector columns and the reciprocal-rank fusion in `Store::search`
//! are phase 3. This connection never builds them. The database is
//! `open_in_memory` with `temp_store=MEMORY`.

use rusqlite::{params, Connection};

use crate::error::Result;

/// Voice packet size. Same cap as `horbm_memory::MAX_HITS` and the Symphony
/// `MemoryHit` list.
pub const MAX_HITS: usize = 4;

const SCHEMA: &str = r#"
CREATE TABLE items (
    rowid INTEGER PRIMARY KEY,
    item_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    said_by TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT '',
    marked INTEGER NOT NULL DEFAULT 0,
    current INTEGER NOT NULL DEFAULT 1
);
CREATE VIRTUAL TABLE items_fts USING fts5(
    title,
    summary,
    body,
    said_by,
    source,
    tokenize = 'unicode61'
);
CREATE VIRTUAL TABLE items_fts_vocab USING fts5vocab(items_fts, 'row');
"#;

/// One hit Swarm 4 can turn into a clause. `score` is higher when the hit is better.
#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalHit {
    pub card_id: String,
    pub clause: String,
    pub source: String,
    pub score: f32,
}

pub(crate) struct StoredRow {
    pub id: String,
    pub title: String,
    pub clause: String,
    pub body: String,
    pub said_by: String,
    pub source: String,
    pub fts_title: String,
    pub fts_summary: String,
    pub fts_body: String,
    pub fts_said_by: String,
    pub fts_source: String,
}

/// RAM FTS index. Not `Sync` by itself; the retrieval thread wraps it.
pub struct FtsIndex {
    conn: Connection,
}

impl FtsIndex {
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "temp_store", "MEMORY")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub(crate) fn insert_rows(&mut self, rows: &[StoredRow]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut items = tx.prepare(
                "INSERT INTO items (item_id, title, summary, body, said_by, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut fts = tx.prepare(
                "INSERT INTO items_fts(rowid, title, summary, body, said_by, source)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for row in rows {
                items.execute(params![
                    row.id,
                    row.title,
                    row.clause,
                    row.body,
                    row.said_by,
                    row.source
                ])?;
                let rowid = tx.last_insert_rowid();
                fts.execute(params![
                    rowid,
                    row.fts_title,
                    row.fts_summary,
                    row.fts_body,
                    row.fts_said_by,
                    row.fts_source
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Refuse further writes on this connection. The file system is untouched
    /// either way; this only stops a later statement from mutating RAM rows.
    pub(crate) fn freeze(&self) -> Result<()> {
        self.conn.pragma_update(None, "query_only", true)?;
        Ok(())
    }

    pub fn query(&self, stable_prefix: &str) -> Result<Vec<RetrievalHit>> {
        let Some(match_q) = match_query(stable_prefix) else {
            return Ok(Vec::new());
        };
        let mut stmt = self.conn.prepare(
            "SELECT items.item_id, items.summary, items.body, items.source,
                    bm25(items_fts) AS rank
             FROM items_fts
             JOIN items ON items.rowid = items_fts.rowid
             WHERE items_fts MATCH ?1 AND items.marked = 0 AND items.current = 1
             ORDER BY rank ASC, items.item_id ASC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_q, MAX_HITS as i64], |row| {
            let summary: String = row.get(1)?;
            let body: String = row.get(2)?;
            let rank: f64 = row.get(4)?;
            Ok(RetrievalHit {
                card_id: row.get(0)?,
                clause: clause_from(&summary, &body),
                source: row.get(3)?,
                score: bm25_rank_to_score(rank),
            })
        })?;
        let mut hits = Vec::new();
        for row in rows {
            hits.push(row?);
        }
        Ok(hits)
    }
}

pub(crate) fn row_from_parts(id: &str, clause: &str, source: &str) -> StoredRow {
    StoredRow {
        id: id.to_string(),
        title: id.to_string(),
        clause: clause.to_string(),
        body: clause.to_string(),
        said_by: "fixture".to_string(),
        source: source.to_string(),
        fts_title: field(id),
        fts_summary: field(clause),
        fts_body: field(clause),
        fts_said_by: field("fixture"),
        fts_source: field(source),
    }
}

pub(crate) fn row_from_card(card: &horbm_memory::Card) -> StoredRow {
    StoredRow {
        id: card.id.clone(),
        title: card.title.clone(),
        clause: card.gist(),
        body: card.body.clone(),
        said_by: card.said_by.clone(),
        source: card.source.clone(),
        fts_title: field(&card.title),
        fts_summary: field(&card.summary),
        fts_body: field(&card.lexical_body()),
        fts_said_by: field(&card.said_by),
        fts_source: field(&card.source),
    }
}

fn field(raw: &str) -> String {
    horbm_memory::index_tokens(raw).join(" ")
}

/// Quoted OR of the same tokens `horbm-memory` writes into FTS.
/// That tokenizer is the Latin shape `openmind-memory` `build_fts_query` uses.
/// At most 32 terms. No document-frequency drop (that scan is inside
/// `Store::search`, which also runs the vector leg).
pub(crate) fn match_query(stable_prefix: &str) -> Option<String> {
    let mut terms = horbm_memory::index_tokens(stable_prefix);
    terms.sort();
    terms.dedup();
    terms.truncate(32);
    if terms.is_empty() {
        return None;
    }
    Some(
        terms
            .into_iter()
            .map(|term| format!("\"{term}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

fn clause_from(summary: &str, body: &str) -> String {
    if !summary.is_empty() {
        return summary.to_string();
    }
    let count = body.chars().count();
    if count <= 480 {
        body.to_string()
    } else {
        let truncated: String = body.chars().take(480).collect();
        format!("{truncated}…")
    }
}

/// Same map as `openmind-memory` `fts.rs` `bm25_rank_to_score`.
/// SQLite `bm25()` is more negative for a better hit. The returned score is
/// in `(0, 1]` and higher is better. The SQL `ORDER BY rank ASC` already
/// put the best hit first.
fn bm25_rank_to_score(rank: f64) -> f32 {
    if !rank.is_finite() {
        return (1.0 / (1.0 + 999.0)) as f32;
    }
    if rank < 0.0 {
        let r = -rank;
        (r / (1.0 + r)) as f32
    } else {
        (1.0 / (1.0 + rank)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stopwords_alone_do_not_query() {
        assert!(match_query("the and of").is_none());
    }

    #[test]
    fn cap_is_four_and_unique_token_returns() {
        let mut index = FtsIndex::open_in_memory().unwrap();
        let rows: Vec<_> = (0..6)
            .map(|i| row_from_parts(&format!("cap.{i}"), "shared token zeta", "src"))
            .collect();
        index.insert_rows(&rows).unwrap();
        let hits = index.query("zeta").unwrap();
        assert_eq!(hits.len(), MAX_HITS);
        let mut index = FtsIndex::open_in_memory().unwrap();
        index
            .insert_rows(&[row_from_parts("only.one", "lonely token xylophone", "src")])
            .unwrap();
        index.freeze().unwrap();
        let hits = index.query("xylophone").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].card_id, "only.one");
        assert!(hits[0].score > 0.0);
    }
}
