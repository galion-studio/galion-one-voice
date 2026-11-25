// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read-only HORBM knowledge cards.
//!
//! `open` reads `.md` and `.json` files. It does not create the directory
//! or write a card. Remember stays deny unless a later dev host profile sets
//! the grant word. The person diary file is created only then, and only when
//! a turn writes. A query that shares no word
//! with a card is [`UNKNOWN_REPLY`]. [`CardEngine`] asks [`horbm::Brain`],
//! which reads these cards first. This engine does not bind a socket and
//! does not dial the Node desk.

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::chunks::contains_forbidden;
use crate::engine::{Reply, ReplyEngine};
use crate::index::{CardLoader, CardMatch};
use crate::pad::Pad;
use crate::research;
use crate::router::{speakable_line, ResearchAnswer, ResearchPort};

/// Spoken when no card matches. Not a guess and not a model completion.
pub use horbm::UNKNOWN_REPLY;

const MAX_CARD_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug)]
struct Card {
    id: String,
    text: String,
    blob: String,
}

/// Cards loaded once. The directory is not opened for writing.
#[derive(Clone, Debug, Default)]
pub struct CardBrain {
    cards: Vec<Card>,
}

impl CardBrain {
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Read `dir` if it is already a directory. A missing path is an empty
    /// set, not an error, and it is not created.
    pub fn open_readonly(dir: &Path) -> Self {
        let mut cards = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Self { cards };
        };
        let Ok(root) = dir.canonicalize() else {
            return Self { cards };
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() || meta.len() > MAX_CARD_BYTES {
                continue;
            }
            let Ok(canon) = path.canonicalize() else {
                continue;
            };
            if !canon.starts_with(&root) {
                continue;
            }
            let Some(id_raw) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let Ok(text) = read_to_string(&path) else {
                continue;
            };
            let (id, body) = match ext.as_str() {
                "md" => (clean_id(id_raw), markdown_body(&text)),
                "json" => {
                    let id = json_string(&text, "id").unwrap_or_else(|| clean_id(id_raw));
                    let body = json_string(&text, "text")
                        .or_else(|| json_string(&text, "body"))
                        .unwrap_or_default();
                    (clean_id(&id), body)
                }
                _ => continue,
            };
            let body = body.trim().to_string();
            if body.is_empty() || id.is_empty() {
                continue;
            }
            let blob = format!("{id} {body}").to_ascii_lowercase();
            cards.push(Card {
                id,
                text: body,
                blob,
            });
        }
        cards.sort_by(|a, b| a.id.cmp(&b.id));
        Self { cards }
    }

    pub fn answer(&self, query: &str) -> Reply {
        let started = Instant::now();
        let tokens = tokens(query);
        let mut best: Option<(&Card, usize)> = None;
        for card in &self.cards {
            let score = tokens
                .iter()
                .filter(|t| card.blob.contains(t.as_str()))
                .count();
            if score == 0 {
                continue;
            }
            let replace = match best {
                None => true,
                Some((prev, prev_score)) => {
                    score > prev_score || (score == prev_score && card.id < prev.id)
                }
            };
            if replace {
                best = Some((card, score));
            }
        }
        let took_us = elapsed_us(started);
        match best {
            Some((card, _)) => Reply {
                text: card.text.clone(),
                mood: Pad::RESTING,
                sources: vec![format!("card:{}", card.id)],
                evidence: vec![format!("card:{}", card.id)],
                propensity: 0.0,
                violations: usize::from(contains_forbidden(&card.text)),
                lookup_us: took_us,
                took_us,
            },
            None => Reply {
                text: UNKNOWN_REPLY.to_string(),
                mood: Pad::RESTING,
                sources: vec!["horbm:none".into()],
                evidence: Vec::new(),
                propensity: 0.0,
                violations: 0,
                lookup_us: took_us,
                took_us,
            },
        }
    }
}

/// Cards the 7881 router reads. JSON uses the galion-knowledge loader.
/// Markdown cards, including the shipped kettle note, answer when JSON has
/// no hit.
struct LocalCards {
    json: horbm::KnowledgeCards,
    prose: CardBrain,
}

impl CardLoader for LocalCards {
    fn lookup(&self, query: &str) -> Option<CardMatch> {
        card_match(self, query)
    }
}

impl horbm::CardLookup for LocalCards {
    fn len(&self) -> usize {
        self.json.len() + self.prose.len()
    }

    fn lookup(&self, query: &str) -> Option<horbm::CardHit> {
        card_match(self, query).map(|hit| horbm::CardHit {
            id: hit.id,
            gist: hit.gist,
        })
    }
}

fn card_match(cards: &LocalCards, query: &str) -> Option<CardMatch> {
    if let Some(hit) = horbm::CardLookup::lookup(&cards.json, query) {
        return Some(CardMatch {
            id: hit.id,
            gist: hit.gist,
        });
    }
    let reply = cards.prose.answer(query);
    let id = reply
        .sources
        .iter()
        .find_map(|source| source.strip_prefix("card:"))?;
    if reply.text.trim().is_empty() {
        return None;
    }
    Some(CardMatch {
        id: id.to_string(),
        gist: reply.text,
    })
}

struct PortAsk(Box<dyn ResearchPort>);

impl horbm::ResearchAsk for PortAsk {
    fn ask(&mut self, question: &str, cap: Duration) -> horbm::AskOutcome {
        match self.0.ask(question, cap) {
            ResearchAnswer::Answer { text, source } => horbm::AskOutcome::Answer { text, source },
            ResearchAnswer::Miss => horbm::AskOutcome::Miss,
            ResearchAnswer::Timeout => horbm::AskOutcome::Timeout,
            ResearchAnswer::Unreachable => horbm::AskOutcome::Unreachable,
        }
    }
}

/// Read-only cards plus the optional research port, as one `horbm` brain.
/// Prints one startup line: card count, load time, and manifest state.
pub(crate) fn brain_from_dir(knowledge_dir: &Path, bind: Option<&str>) -> horbm::Brain {
    let started = Instant::now();
    let (json, manifest) = match horbm::KnowledgeCards::load(knowledge_dir) {
        Ok(loaded) => (loaded.cards, loaded.report.manifest),
        Err(horbm::CardError::NotADirectory(_)) => {
            (horbm::KnowledgeCards::empty(), "absent".to_string())
        }
        Err(err) => {
            eprintln!("horbm cards: {err}");
            (horbm::KnowledgeCards::empty(), "rejected".to_string())
        }
    };
    let prose = CardBrain::open_readonly(knowledge_dir);
    let report = horbm::LoadReport {
        cards: json.len() + prose.len(),
        elapsed: started.elapsed(),
        path: knowledge_dir.to_path_buf(),
        manifest,
    };
    eprintln!("{}", report.log_line());
    let research = research::research_port(bind)
        .map(|port| Box::new(PortAsk(port)) as Box<dyn horbm::ResearchAsk + Send>);
    let mut brain = horbm::Brain::with_cards(LocalCards { json, prose }, research)
        .with_style(horbm::ResearchRoot::from_env());
    brain.set_speakable(speakable_line);
    attach_retrieval(brain, knowledge_dir)
}

/// The 7881 turn reads the retrieval cell. Remember stays deny. This library
/// does not set `GALION_HORBM_REMEMBER`, and `LOCKS.toml` `[consent] remember`
/// stays deny. A later dev host profile is what passes the word `grant`.
/// A denied spawn does not open the diary.
fn attach_retrieval(brain: horbm::Brain, knowledge_dir: &Path) -> horbm::Brain {
    let mut config = horbm::RetrievalConfig::from_env();
    config.knowledge_dir = Some(knowledge_dir.to_path_buf());
    config.fixture_rows = 0;
    match horbm::RetrievalThread::spawn(config) {
        Ok(thread) => brain.with_retrieval(thread),
        Err(err) => {
            eprintln!("horbm retrieval: {}", err.message());
            brain
        }
    }
}

/// HORBM reply engine. A turn is [`horbm::Brain::turn`]: the retrieval cell
/// when `remember` is granted, otherwise cards and the clause fixture, then
/// the gateway, then [`UNKNOWN_REPLY`]. The memory path is the person diary,
/// attached with [`horbm::EpisodicStore::open_from_flag`].
///
/// Library defaults stay deny, and `LOCKS.toml` `[consent] remember` stays
/// deny. This engine does not grant remember or learn. A later dev host
/// profile is what sets `GALION_HORBM_REMEMBER`. Only the trimmed word
/// `grant` lets a later turn create the diary file. That is the same word
/// [`horbm::RetrievalConfig::from_env`] reads for the retrieval thread.
/// [`CardEngine::open`] reads it once. Any other value does not
/// create the file. The cases path is accepted so the `Brain::open` shape
/// stays, and it is not opened.
pub struct CardEngine {
    brain: horbm::Brain,
    knowledge_dir: std::path::PathBuf,
    cards: usize,
}

impl CardEngine {
    /// Read `GALION_HORBM_REMEMBER` once and open the engine.
    /// Unset stays deny. This does not grant consent.
    /// [`Self::open_with_remember`] is the same path with the flag passed in.
    pub fn open(memory_db: &Path, knowledge_dir: &Path, cases: &Path) -> Self {
        let remember = std::env::var("GALION_HORBM_REMEMBER").ok();
        Self::open_with_remember(memory_db, knowledge_dir, cases, remember.as_deref())
    }

    /// [`Self::open`] with an explicit remember flag.
    ///
    /// This does not read `GALION_HORBM_REMEMBER` and does not grant consent.
    /// `None` and any value other than the trimmed word `grant` leave the
    /// diary file uncreated. Learn consent is a different flag and is not read.
    pub fn open_with_remember(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        remember: Option<&str>,
    ) -> Self {
        Self::open_bound(
            memory_db,
            knowledge_dir,
            cases,
            research::bind_from_env().as_deref(),
            remember,
        )
    }

    fn open_bound(
        memory_db: &Path,
        knowledge_dir: &Path,
        cases: &Path,
        bind: Option<&str>,
        remember: Option<&str>,
    ) -> Self {
        let _ = cases;
        let brain = brain_from_dir(knowledge_dir, bind)
            .with_memory(horbm::EpisodicStore::open_from_flag(memory_db, remember));
        let cards = brain.cards();
        Self {
            brain,
            knowledge_dir: knowledge_dir.to_path_buf(),
            cards,
        }
    }

    /// Clauses available to this engine. The example directory is the kettle card.
    /// `G:\galion\horbm\knowledge` is the real pack when that directory is opened.
    pub fn cards(&self) -> usize {
        self.cards
    }

    pub fn knowledge_dir(&self) -> &std::path::Path {
        &self.knowledge_dir
    }

    pub fn mood_meta(&self) -> horbm::MoodMeta {
        self.brain.mood_meta()
    }

    /// Shared brain. The host reads the persona and the person id here.
    pub fn brain(&self) -> &horbm::Brain {
        &self.brain
    }

    /// Mutable brain. The host calls [`horbm::Brain::inner_turn`],
    /// [`horbm::Brain::inner_nudge`], and [`horbm::Brain::person_memory`]
    /// so a granted diary can end its session. Neither call grants remember
    /// or learn.
    pub fn brain_mut(&mut self) -> &mut horbm::Brain {
        &mut self.brain
    }
}

impl ReplyEngine for CardEngine {
    fn turn(&mut self, text: &str) -> Reply {
        let out = self.brain.turn(text);
        Reply {
            text: out.reply,
            mood: Pad::new(out.mood.arousal, out.mood.valence, out.mood.dominance),
            sources: out.sources,
            evidence: out.evidence,
            propensity: out.propensity,
            violations: out.violations,
            lookup_us: out.lookup_us,
            took_us: out.took_us,
        }
    }

    fn tick(&mut self) -> Pad {
        let meta = self.brain.tick_frame();
        Pad::new(meta.arousal, meta.valence, meta.dominance)
    }

    fn mood(&self) -> Pad {
        let meta = self.brain.mood_meta();
        Pad::new(meta.arousal, meta.valence, meta.dominance)
    }
}

fn read_to_string(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

fn markdown_body(text: &str) -> String {
    let text = text.trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    let Some(first) = lines.next() else {
        return String::new();
    };
    if first.trim_start().starts_with("# ") {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    }
}

fn json_string(src: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let start = src.find(&pattern)?;
    let rest = src[start + pattern.len()..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => break,
            }
        } else if c == '"' {
            break;
        } else {
            out.push(c);
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn clean_id(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn tokens(query: &str) -> Vec<String> {
    query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.to_ascii_lowercase().as_str()))
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

const STOP: &[&str] = &[
    "the", "and", "for", "are", "was", "not", "but", "with", "this", "that", "from", "have",
    "what", "when", "where", "who", "how", "why", "about", "you", "your", "does", "did",
];

fn elapsed_us(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_card(dir: &Path, name: &str, body: &str) {
        fs::write(dir.join(name), body).unwrap();
    }

    #[test]
    fn matching_card_is_quoted_and_a_miss_is_i_dont_know() {
        let dir = std::env::temp_dir().join(format!(
            "galion-cards-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        fs::create_dir_all(&dir).unwrap();
        write_card(&dir, "kettle.md", "# Kettle\nThe kettle is on the stove.\n");
        write_card(
            &dir,
            "gate.json",
            "{\"id\":\"gate\",\"text\":\"The gate sticks on the left.\"}\n",
        );
        let before = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        let brain = CardBrain::open_readonly(&dir);
        let hit = brain.answer("where is the kettle");
        assert_eq!(hit.text, "The kettle is on the stove.");
        assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
        assert_eq!(hit.violations, 0);
        assert!(!hit.text.contains("don't know"));
        let other = brain.answer("what about the gate");
        assert_eq!(other.text, "The gate sticks on the left.");
        assert_eq!(other.sources, vec!["card:gate".to_string()]);
        let miss = brain.answer("who won the match");
        assert_eq!(miss.text, UNKNOWN_REPLY);
        assert_eq!(miss.sources, vec!["horbm:none".to_string()]);
        assert_eq!(miss.evidence.len(), 0);
        let after = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(before.len(), after.len());
        let kettle = fs::read_to_string(dir.join("kettle.md")).unwrap();
        assert!(kettle.contains("The kettle is on the stove."));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn open_does_not_create_a_missing_directory() {
        let dir = std::env::temp_dir().join(format!(
            "galion-cards-missing-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        assert!(!dir.exists());
        let brain = CardBrain::open_readonly(&dir);
        let miss = brain.answer("hello there");
        assert_eq!(miss.text, UNKNOWN_REPLY);
        assert!(!dir.exists());
    }

    #[test]
    fn engine_asks_the_router_then_the_cards() {
        let dir = std::env::temp_dir().join(format!(
            "galion-engine-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        fs::create_dir_all(&dir).unwrap();
        write_card(&dir, "kettle.md", "The kettle is on the stove.\n");
        let memory = dir.join("memory.db");
        let cases = dir.join("cases");
        let mut engine =
            CardEngine::open_bound(&memory, &dir, &cases, Some("127.0.0.1:8787"), None);
        let hit = engine.turn("where is the kettle");
        assert_eq!(hit.text, "The kettle is on the stove.");
        assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
        assert!(!memory.exists());
        assert!(!cases.exists());
        let started = Instant::now();
        let miss = engine.turn("why is the spare fuse missing from the drawer?");
        assert!(started.elapsed().as_millis() < 500);
        assert_eq!(miss.text, UNKNOWN_REPLY);
        assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
        assert!(!miss.text.to_ascii_lowercase().contains("ollama"));
        assert!(!dir.join("revisions.sqlite").exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_real_json_card_answers_with_its_id_and_a_miss_is_exact() {
        let dir = std::env::temp_dir().join(format!(
            "galion-json-pack-{}-{}",
            std::process::id(),
            elapsed_us(Instant::now())
        ));
        fs::create_dir_all(dir.join("horbm")).unwrap();
        fs::write(
            dir.join("horbm").join("fixture-kettle.json"),
            r#"{
  "id": "fixture-kettle",
  "title": "The kettle",
  "section": "horbm",
  "category": "fixture",
  "summary": "The kettle is on the hob.",
  "body": "The kettle is on the hob.",
  "claim_type": "fact",
  "sources": [{"title": "Fixture", "url": "https://github.com/galion-studio/galion-knowledge", "author": "Galion Labs", "date": "2026-11-18", "accessed": "2026-11-22"}],
  "evidence_strength": "strong",
  "said_by": "Galion Labs",
  "as_of_date": "2026-11-18",
  "status": "draft",
  "contested_views": [],
  "related": []
}
"#,
        )
        .unwrap();
        let memory = dir.join("memory.db");
        let mut engine = CardEngine::open_bound(&memory, &dir, &dir.join("cases"), None, None);
        let hit = engine.turn("where is the kettle");
        assert_eq!(hit.text, "The kettle is on the hob.");
        assert_eq!(hit.sources, vec!["card:fixture-kettle".to_string()]);
        let miss = engine.turn("who won the match");
        assert_eq!(miss.text, "I don't know.");
        assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
        assert!(!memory.exists());
        assert!(!dir.join("revisions.sqlite").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
}
