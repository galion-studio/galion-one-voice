// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! HORBM's diary and memory stack for knowledge.galion.app.
//!
//! Three places a sentence can live:
//!
//! - a short-term RAM ring ([`SessionRing`]) plus an append-only transcript
//! - a pinned episode, written by [`Store::remember`]
//! - a candidate fact extracted on the background queue
//! - a sourced semantic row, written by ingesting a knowledge card, plus
//!   the append-only diary
//! - an archive of stored rows that an admin `forget` or a revision hid
//!   from retrieval. The text stays. Search does not read the archive.
//!
//! Consent is given by use. There is no withdrawal command.
//!
//! Search mixes SQLite FTS5 BM25 with an exact scan of one int8 vector per
//! row, fuses the two rankings with reciprocal rank fusion, and returns at
//! most [`MAX_HITS`] hits. Every hit carries `said_by`, `date`, and `source`.
//!
//! [`Stack::query`] searches memory and the knowledge cards as two lists.
//! What the user told us stays in `memory`. World cards stay in `knowledge`.
//! A miss on the cards is the answer `I don't know`. The gap row is
//! enqueued and written by the background worker.

mod card;
mod embed;
mod error;
mod extract;
mod queue;
mod rank;
mod rrf;
mod scale;
mod session;
mod stack;
mod store;
mod text;

pub use card::{card_from_document, parse_cards, sha256_hex, Card, ContestedView, Source};
pub use embed::{simd_lane, HashingEmbedder, EMB_DIM};
pub use error::{Error, Result};
pub use extract::{
    summarise_session, BuiltinExtractor, Candidate, FactExtractor, SummaryTurn, SUMMARY_IMPORTANCE,
};
pub use queue::QueueHandle;
pub use rank::{recency_factor, rescale, DEFAULT_IMPORTANCE, PIN_BOOST, RECENCY_HALF_LIFE_DAYS};
pub use scale::{rss_bytes, ScaleIndex, ScaleReport, ShardManifest, CATEGORIES};
pub use session::{estimate_tokens, fit_context, ContextMessage, SessionRing, Turn};
pub use stack::{synthetic_card, ImportReport, QueryResponse, Stack, StoredRevision};
pub use store::{
    ArchivedRow, CardRevision, DiaryEntry, GapRank, Hit, IngestReport, MemoryRevision,
    SearchResponse, Store, LAYER_DIARY, LAYER_EPISODE, LAYER_SEMANTIC, MAX_HITS, RING_CAP, RRF_K,
};
// Re-export of the existing FTS tokenizer so the voice retrieval crate can
// build the same quoted-OR query without copying it. Pin of this tree:
// galion-studio/galion-knowledge @ 761a7f225304fc010379219e5b88c6669dd48f40
// (master, which contains merged PR #16 / d5c4177b4aa61592b491080f90d76b1ec6ef4690).
pub use text::{index_tokens, is_stopword, tokens};
