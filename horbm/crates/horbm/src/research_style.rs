// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Dialogue style examples. Not a truth source.
//!
//! The directory comes only from `GALION_HORBM_RESEARCH_DIR`. Unset, empty,
//! or not a directory means this root is off: no hits and no file reads.
//! There is no default path. The knowledge directory is never opened here.
//!
//! Cards live at `<root>/cards/diary/dialogue/<dataset>/`. A bundle is one
//! JSON value, at most 246 KiB. Larger files are not opened. Bundles are
//! streamed one file at a time, and only for the query language. An English
//! query does not open a Chinese LCCC bundle. The index is tantivy, in RAM,
//! built on the first query for that language.
//!
//! Samantha may use a hit as a style mark. Aria is parked and gets nothing.
//! Every hit is tagged `not_for` answer and cite. The example text is not
//! an answer and not a citation.
//!
//! Portable `std::fs` and an in-memory index. No platform path and no
//! `.exe`. Nothing here is cfg-gated. The tests run on Linux.

use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::{Map, Value};
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{
    IndexRecordOption, Schema, TantivyDocument, Term, TextFieldIndexing, TextOptions, Value as _,
    STORED, STRING,
};
use tantivy::tokenizer::SimpleTokenizer;
use tantivy::{Index, IndexReader};

use crate::cards::{index_tokens, valid_id, CardError};

/// How many style hits a turn may take. The dialogue manager still refuses
/// to speak them.
pub const STYLE_HIT_CAP: usize = 3;

/// A research bundle larger than this is not opened. 246 KiB.
pub const MAX_RESEARCH_BUNDLE_BYTES: u64 = 246 * 1024;

const NEAR_DUP_JACCARD: f32 = 0.8;
const TOKENIZER_NAME: &str = "style";

/// Who is asking. Only Samantha searches. Aria stays parked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylePersona {
    Samantha,
    Aria,
}

/// One style example. `allows_answer` and `allows_cite` are always false.
#[derive(Clone, Debug, PartialEq)]
pub struct ResearchHit {
    pub id: String,
    /// Example text. It shapes a style mark. It is not spoken.
    pub text: String,
    pub licence: String,
    pub dataset: String,
    /// `en`, `zh`, or `und`.
    pub lang: String,
    pub score: f32,
    /// Always contains `answer` and `cite` on a hit this search returns.
    pub not_for: Vec<String>,
}

impl ResearchHit {
    /// Style examples are never answer content.
    pub fn allows_answer(&self) -> bool {
        self.permits("answer")
    }

    /// Style examples are never a citation.
    pub fn allows_cite(&self) -> bool {
        self.permits("cite")
    }

    pub fn tagged_not_for_answer_and_cite(&self) -> bool {
        has_name(&self.not_for, "answer") && has_name(&self.not_for, "cite")
    }

    fn permits(&self, use_as: &str) -> bool {
        if matches!(use_as, "answer" | "cite") {
            return false;
        }
        !self.not_for.iter().any(|item| item == use_as)
    }
}

/// What one style search returned.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleSearch {
    pub hits: Vec<ResearchHit>,
    pub took_us: u32,
    /// Bundle files opened while answering this call. Zero when the language
    /// index is already built, and zero when research is off.
    pub bundles_opened: usize,
    /// Catalogued bundles left unread because the query language does not match.
    pub bundles_skipped_lang: usize,
    /// Bundles left unread because they are larger than [`MAX_RESEARCH_BUNDLE_BYTES`].
    pub bundles_skipped_size: usize,
}

impl StyleSearch {
    fn empty(skipped_size: usize) -> Self {
        Self {
            hits: Vec::new(),
            took_us: 0,
            bundles_opened: 0,
            bundles_skipped_lang: 0,
            bundles_skipped_size: skipped_size,
        }
    }
}

/// The dialogue card that matched. The id is a style mark, not a clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleHint {
    pub id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum QueryLang {
    En,
    Zh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BundleLang {
    En,
    Zh,
    Und,
}

struct BundleMeta {
    path: PathBuf,
    rel: String,
    dataset: String,
    lang: BundleLang,
}

struct Accepted {
    id: String,
    example: String,
    indexed: String,
    licence: String,
    dataset: String,
    lang: String,
}

struct LangIndex {
    _index: Index,
    reader: IndexReader,
    body: tantivy::schema::Field,
    id: tantivy::schema::Field,
    example: tantivy::schema::Field,
    licence: tantivy::schema::Field,
    dataset: tantivy::schema::Field,
    lang: tantivy::schema::Field,
    cards: usize,
}

#[derive(Default)]
struct BuildState {
    indexes: HashMap<QueryLang, LangIndex>,
    opened: Vec<String>,
}

enum Inner {
    Off,
    Ready {
        bundles: Vec<BundleMeta>,
        skipped_size: usize,
        state: Mutex<BuildState>,
    },
}

/// Optional style-example root. Off until a research directory is passed in.
pub struct ResearchRoot {
    inner: Inner,
}

impl Default for ResearchRoot {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::fmt::Debug for ResearchRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            Inner::Off => f.write_str("ResearchRoot::Off"),
            Inner::Ready {
                bundles,
                skipped_size,
                ..
            } => f
                .debug_struct("ResearchRoot")
                .field("bundles", &bundles.len())
                .field("skipped_size", skipped_size)
                .finish(),
        }
    }
}

impl ResearchRoot {
    pub fn empty() -> Self {
        Self { inner: Inner::Off }
    }

    /// Cards indexed so far. Zero until a Samantha query builds a language index.
    pub fn len(&self) -> usize {
        let Inner::Ready { state, .. } = &self.inner else {
            return 0;
        };
        state
            .lock()
            .expect("research style")
            .indexes
            .values()
            .map(|index| index.cards)
            .sum()
    }

    /// No catalogued bundles. Off is empty. An oversize-only tree is empty.
    pub fn is_empty(&self) -> bool {
        self.bundle_count() == 0
    }

    pub fn bundle_count(&self) -> usize {
        match &self.inner {
            Inner::Off => 0,
            Inner::Ready { bundles, .. } => bundles.len(),
        }
    }

    /// Bundle paths opened so far, relative to `cards/diary/dialogue`.
    pub fn opened_bundles(&self) -> Vec<String> {
        let Inner::Ready { state, .. } = &self.inner else {
            return Vec::new();
        };
        state.lock().expect("research style").opened.clone()
    }

    /// How many bundle files have been opened. Zero when research is off.
    pub fn files_read(&self) -> usize {
        self.opened_bundles().len()
    }

    /// `GALION_HORBM_RESEARCH_DIR` only. Any other state, including an unset
    /// variable, is [`Self::empty`] and does not read a file. Nothing is created.
    pub fn from_env() -> Self {
        match std::env::var("GALION_HORBM_RESEARCH_DIR") {
            Ok(value) => Self::from_flag(Some(value.as_str())),
            Err(_) => Self::empty(),
        }
    }

    pub fn from_flag(flag: Option<&str>) -> Self {
        let Some(raw) = flag.map(str::trim).filter(|raw| !raw.is_empty()) else {
            return Self::empty();
        };
        let path = PathBuf::from(raw);
        if !path.is_dir() {
            return Self::empty();
        }
        match Self::load(&path) {
            Ok(root) => root,
            Err(err) => {
                eprintln!("horbm research cards: {err}");
                Self::empty()
            }
        }
    }

    /// Catalogue `dir`. Does not read bundle bytes. The directory is not created.
    pub fn load(dir: &Path) -> Result<Self, CardError> {
        let (bundles, skipped_size) = catalog(dir)?;
        Ok(Self {
            inner: Inner::Ready {
                bundles,
                skipped_size,
                state: Mutex::new(BuildState::default()),
            },
        })
    }

    /// Top style id for Samantha, if the query matches. The gist is not returned.
    pub fn lookup(&self, query: &str) -> Option<StyleHint> {
        self.search(StylePersona::Samantha, query)
            .hits
            .into_iter()
            .next()
            .map(|hit| StyleHint { id: hit.id })
    }

    pub fn search(&self, persona: StylePersona, query: &str) -> StyleSearch {
        self.search_limit(persona, query, STYLE_HIT_CAP)
    }

    pub fn search_limit(&self, persona: StylePersona, query: &str, limit: usize) -> StyleSearch {
        let Inner::Ready {
            bundles,
            skipped_size,
            state,
        } = &self.inner
        else {
            return StyleSearch::empty(0);
        };
        if persona != StylePersona::Samantha {
            return StyleSearch::empty(*skipped_size);
        }
        let started = Instant::now();
        let Some(lang) = query_lang(query) else {
            return finished(Vec::new(), started, 0, 0, *skipped_size);
        };
        let skipped_lang = bundles
            .iter()
            .filter(|bundle| !eligible(bundle, lang))
            .count();
        let tokens = unique_tokens(query);
        if tokens.is_empty() || limit == 0 {
            return finished(Vec::new(), started, 0, skipped_lang, *skipped_size);
        }
        let mut guard = state.lock().expect("research style");
        let before = guard.opened.len();
        if ensure(&mut guard, bundles, lang).is_err() {
            let opened = guard.opened.len().saturating_sub(before);
            return finished(Vec::new(), started, opened, skipped_lang, *skipped_size);
        }
        let opened = guard.opened.len().saturating_sub(before);
        let Some(index) = guard.indexes.get(&lang) else {
            return finished(Vec::new(), started, opened, skipped_lang, *skipped_size);
        };
        let hits = match rank(index, &tokens, limit) {
            Ok(found) => dedupe(found).into_iter().take(limit).collect(),
            Err(()) => Vec::new(),
        };
        finished(hits, started, opened, skipped_lang, *skipped_size)
    }
}

fn finished(
    hits: Vec<ResearchHit>,
    started: Instant,
    bundles_opened: usize,
    bundles_skipped_lang: usize,
    bundles_skipped_size: usize,
) -> StyleSearch {
    StyleSearch {
        hits,
        took_us: micros(started.elapsed()),
        bundles_opened,
        bundles_skipped_lang,
        bundles_skipped_size,
    }
}

fn micros(elapsed: std::time::Duration) -> u32 {
    u32::try_from(elapsed.as_micros()).unwrap_or(u32::MAX)
}

fn catalog(root: &Path) -> Result<(Vec<BundleMeta>, usize), CardError> {
    if !root.is_dir() {
        return Err(CardError::NotADirectory(root.display().to_string()));
    }
    let dialogue = root.join("cards").join("diary").join("dialogue");
    if !dialogue.is_dir() {
        return Ok((Vec::new(), 0));
    }
    let mut bundles = Vec::new();
    let mut skipped_size = 0usize;
    walk(&dialogue, &dialogue, &mut bundles, &mut skipped_size)?;
    bundles.sort_by(|left, right| left.rel.cmp(&right.rel));
    Ok((bundles, skipped_size))
}

fn walk(
    dialogue: &Path,
    dir: &Path,
    out: &mut Vec<BundleMeta>,
    skipped_size: &mut usize,
) -> Result<(), CardError> {
    let entries = std::fs::read_dir(dir).map_err(|err| CardError::Io(err.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|err| CardError::Io(err.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|err| CardError::Io(err.to_string()))?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            walk(dialogue, &path, out, skipped_size)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".json")
            || matches!(
                name.as_ref(),
                "manifest.json" | "catalog.json" | "schema.json"
            )
        {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|err| CardError::Io(err.to_string()))?;
        if meta.len() > MAX_RESEARCH_BUNDLE_BYTES {
            *skipped_size = skipped_size.saturating_add(1);
            continue;
        }
        let rel = rel_path(dialogue, &path);
        let Some(dataset) = rel.split('/').next() else {
            continue;
        };
        if dataset.is_empty() {
            continue;
        }
        out.push(BundleMeta {
            lang: bundle_lang(dataset, &rel),
            dataset: dataset.to_string(),
            path,
            rel,
        });
    }
    Ok(())
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn ensure(state: &mut BuildState, bundles: &[BundleMeta], lang: QueryLang) -> Result<(), ()> {
    if state.indexes.contains_key(&lang) {
        return Ok(());
    }
    let built = build_index(bundles, lang, &mut state.opened)?;
    state.indexes.insert(lang, built);
    Ok(())
}

fn build_index(
    bundles: &[BundleMeta],
    lang: QueryLang,
    opened: &mut Vec<String>,
) -> Result<LangIndex, ()> {
    let mut schema_builder = Schema::builder();
    let body_options = TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(TOKENIZER_NAME)
            .set_index_option(IndexRecordOption::WithFreqs),
    );
    let body = schema_builder.add_text_field("body", body_options);
    let example = schema_builder.add_text_field("example", TextOptions::default().set_stored());
    let id = schema_builder.add_text_field("id", STRING | STORED);
    let licence = schema_builder.add_text_field("licence", STRING | STORED);
    let dataset = schema_builder.add_text_field("dataset", STRING | STORED);
    let lang_field = schema_builder.add_text_field("lang", STRING | STORED);
    let schema = schema_builder.build();
    let index = Index::create_in_ram(schema);
    // Tokens are already split and lowercased. This tokenizer only
    // separates them. It does not stem, and it does not drop long tokens.
    index
        .tokenizers()
        .register(TOKENIZER_NAME, SimpleTokenizer::default());
    let mut writer = index.writer(16 * 1024 * 1024).map_err(|_| ())?;
    let mut cards = 0usize;
    for bundle in bundles {
        if !eligible(bundle, lang) {
            continue;
        }
        let parsed = match read_bundle(bundle, lang) {
            Ok(parsed) => parsed,
            Err(ReadFail::Invalid) => {
                opened.push(bundle.rel.clone());
                continue;
            }
            Err(ReadFail::NotOpened) => continue,
        };
        opened.push(bundle.rel.clone());
        for card in parsed {
            let mut doc = TantivyDocument::new();
            doc.add_text(id, &card.id);
            doc.add_text(body, &card.indexed);
            doc.add_text(example, &card.example);
            doc.add_text(licence, &card.licence);
            doc.add_text(dataset, &card.dataset);
            doc.add_text(lang_field, &card.lang);
            if writer.add_document(doc).is_ok() {
                cards = cards.saturating_add(1);
            }
        }
    }
    writer.commit().map_err(|_| ())?;
    drop(writer);
    let reader = index.reader().map_err(|_| ())?;
    Ok(LangIndex {
        _index: index,
        reader,
        body,
        id,
        example,
        licence,
        dataset,
        lang: lang_field,
        cards,
    })
}

enum ReadFail {
    NotOpened,
    Invalid,
}

fn read_bundle(bundle: &BundleMeta, lang: QueryLang) -> Result<Vec<Accepted>, ReadFail> {
    let file = File::open(&bundle.path).map_err(|_| ReadFail::NotOpened)?;
    let reader = BufReader::new(file);
    let value: Value = serde_json::from_reader(reader).map_err(|_| ReadFail::Invalid)?;
    Ok(accept_value(&value, bundle, lang))
}

fn accept_value(value: &Value, bundle: &BundleMeta, query: QueryLang) -> Vec<Accepted> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(|item| accept_card(item, None, None, bundle, query))
            .collect(),
        Value::Object(map) => {
            if let Some(cards) = map.get("cards") {
                let licence = json_str(map, &["licence", "license"]);
                let lang = json_str(map, &["lang", "language"]);
                return match cards {
                    Value::Array(items) => items
                        .iter()
                        .filter_map(|item| {
                            accept_card(item, licence.as_deref(), lang.as_deref(), bundle, query)
                        })
                        .collect(),
                    _ => Vec::new(),
                };
            }
            accept_card(value, None, None, bundle, query)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn accept_card(
    value: &Value,
    bundle_licence: Option<&str>,
    bundle_lang_name: Option<&str>,
    bundle: &BundleMeta,
    query: QueryLang,
) -> Option<Accepted> {
    let obj = value.as_object()?;
    if explicitly_not_research(obj) || is_fact_claim(obj) || !use_for_allows(obj) {
        return None;
    }
    let id = json_str(obj, &["id", "card_id"])?;
    if id.len() > 128 || !valid_id(&id) {
        return None;
    }
    if !bars_answer_and_cite(&string_array(obj.get("not_for"))) {
        return None;
    }
    let licence = json_str(obj, &["licence", "license"]).or_else(|| {
        bundle_licence
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })?;
    if licence.len() > 200 {
        return None;
    }
    let example = example_of(obj);
    if example.is_empty() {
        return None;
    }
    let lang = lang_label(
        json_str(obj, &["lang", "language"]).as_deref(),
        bundle_lang_name,
        bundle.lang,
    );
    if lang_conflicts(&lang, query) {
        return None;
    }
    let title = json_str(obj, &["title", "summary", "gist"]).unwrap_or_default();
    let indexed_source = if title.is_empty() {
        example.clone()
    } else {
        format!("{example}\n{title}")
    };
    let tokens = unique_tokens(&indexed_source);
    if tokens.is_empty() {
        return None;
    }
    Some(Accepted {
        id,
        indexed: tokens.join(" "),
        example,
        licence,
        dataset: bundle.dataset.clone(),
        lang,
    })
}

fn rank(index: &LangIndex, tokens: &[String], limit: usize) -> Result<Vec<ResearchHit>, ()> {
    if index.cards == 0 || tokens.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let searcher = index.reader.searcher();
    let query = boolean_query(index.body, tokens);
    let window = limit.saturating_mul(8).max(limit);
    let top = searcher
        .search(&query, &TopDocs::with_limit(window))
        .map_err(|_| ())?;
    let mut hits = Vec::with_capacity(top.len());
    for (score, address) in top {
        let doc = searcher.doc::<TantivyDocument>(address).map_err(|_| ())?;
        let Some(id) = stored(&doc, index.id) else {
            continue;
        };
        let Some(text) = stored(&doc, index.example) else {
            continue;
        };
        let Some(licence) = stored(&doc, index.licence) else {
            continue;
        };
        let Some(dataset) = stored(&doc, index.dataset) else {
            continue;
        };
        let Some(lang) = stored(&doc, index.lang) else {
            continue;
        };
        hits.push(ResearchHit {
            id,
            text,
            licence,
            dataset,
            lang,
            score,
            not_for: vec!["answer".to_string(), "cite".to_string()],
        });
    }
    Ok(hits)
}

fn boolean_query(field: tantivy::schema::Field, tokens: &[String]) -> BooleanQuery {
    let clauses = tokens
        .iter()
        .map(|token| {
            let term = Term::from_field_text(field, token);
            let query: Box<dyn Query> =
                Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
            (Occur::Should, query)
        })
        .collect();
    BooleanQuery::new(clauses)
}

fn stored(doc: &TantivyDocument, field: tantivy::schema::Field) -> Option<String> {
    let value = doc.get_first(field)?;
    value
        .as_str()
        .map(str::to_string)
        .filter(|text| !text.is_empty())
}

fn dedupe(mut hits: Vec<ResearchHit>) -> Vec<ResearchHit> {
    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut kept = Vec::new();
    let mut seen: Vec<BTreeSet<String>> = Vec::new();
    for hit in hits {
        if kept.iter().any(|prev: &ResearchHit| prev.id == hit.id) {
            continue;
        }
        let tokens: BTreeSet<String> = index_tokens(&hit.text).into_iter().collect();
        if seen
            .iter()
            .any(|prev| jaccard(prev, &tokens) >= NEAR_DUP_JACCARD)
        {
            continue;
        }
        seen.push(tokens);
        kept.push(hit);
    }
    kept
}

fn jaccard(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f32 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let overlap = left.intersection(right).count();
    let union = left.len() + right.len() - overlap;
    if union == 0 {
        1.0
    } else {
        overlap as f32 / union as f32
    }
}

fn eligible(bundle: &BundleMeta, lang: QueryLang) -> bool {
    match lang {
        QueryLang::En => bundle.lang != BundleLang::Zh,
        QueryLang::Zh => bundle.lang == BundleLang::Zh,
    }
}

fn query_lang(query: &str) -> Option<QueryLang> {
    let mut han = false;
    let mut latin = false;
    for ch in query.chars() {
        if is_han(ch) {
            han = true;
        } else if ch.is_ascii_alphabetic() {
            latin = true;
        }
    }
    if han {
        Some(QueryLang::Zh)
    } else if latin {
        Some(QueryLang::En)
    } else {
        None
    }
}

fn is_han(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
    )
}

fn bundle_lang(dataset: &str, rel: &str) -> BundleLang {
    if let Some(lang) = dataset_lang(dataset) {
        if lang != BundleLang::Und {
            return lang;
        }
    }
    for part in rel.split('/') {
        let stem = part.trim_end_matches(".json").to_ascii_lowercase();
        if is_zh_marker(&stem) {
            return BundleLang::Zh;
        }
        if is_en_marker(&stem) {
            return BundleLang::En;
        }
    }
    BundleLang::Und
}

fn dataset_lang(dataset: &str) -> Option<BundleLang> {
    let name = dataset.to_ascii_lowercase();
    if name == "lccc"
        || name.starts_with("lccc-")
        || name.starts_with("lccc_")
        || name.starts_with("lccc.")
    {
        return Some(BundleLang::Zh);
    }
    named_lang(&name)
}

fn named_lang(raw: &str) -> Option<BundleLang> {
    let name = raw.trim().to_ascii_lowercase();
    if name.is_empty() {
        return None;
    }
    let head = name.split(['_', '-']).next().unwrap_or(name.as_str());
    Some(match head {
        "zh" | "chinese" | "cmn" | "lccc" => BundleLang::Zh,
        "en" | "eng" | "english" => BundleLang::En,
        _ => BundleLang::Und,
    })
}

fn is_zh_marker(stem: &str) -> bool {
    matches!(
        stem,
        "zh" | "zh-cn" | "zh-hans" | "zh-hant" | "chinese" | "lccc" | "cmn"
    ) || stem.starts_with("zh-")
        || stem.starts_with("lccc-")
        || stem.starts_with("lccc.")
        || stem.ends_with("-zh")
        || stem.ends_with(".zh")
}

fn is_en_marker(stem: &str) -> bool {
    matches!(stem, "en" | "en-us" | "en-gb" | "english" | "eng")
        || stem.starts_with("en-")
        || stem.ends_with("-en")
        || stem.ends_with(".en")
}

fn lang_label(card: Option<&str>, bundle: Option<&str>, path: BundleLang) -> String {
    if let Some(lang) = card.and_then(named_lang) {
        return lang_name(lang);
    }
    if let Some(lang) = bundle.and_then(named_lang) {
        return lang_name(lang);
    }
    lang_name(path)
}

fn lang_name(lang: BundleLang) -> String {
    match lang {
        BundleLang::En => "en".to_string(),
        BundleLang::Zh => "zh".to_string(),
        BundleLang::Und => "und".to_string(),
    }
}

fn lang_conflicts(label: &str, query: QueryLang) -> bool {
    match query {
        QueryLang::En => label == "zh",
        QueryLang::Zh => label == "en",
    }
}

fn explicitly_not_research(obj: &Map<String, Value>) -> bool {
    matches!(obj.get("research_only"), Some(Value::Bool(false)))
}

fn is_fact_claim(obj: &Map<String, Value>) -> bool {
    let Some(raw) = json_str(obj, &["claim_type", "epistemic"]) else {
        return false;
    };
    matches!(raw.to_ascii_lowercase().as_str(), "fact" | "said")
}

fn use_for_allows(obj: &Map<String, Value>) -> bool {
    match obj.get("use_for") {
        None => true,
        Some(Value::Array(items)) if !items.is_empty() => items.iter().any(|item| {
            item.as_str().is_some_and(|text| {
                matches!(
                    text.trim().to_ascii_lowercase().as_str(),
                    "style" | "empathy" | "moves"
                )
            })
        }),
        Some(_) => false,
    }
}

fn bars_answer_and_cite(items: &[String]) -> bool {
    let answer = items.iter().any(|item| item.eq_ignore_ascii_case("answer"));
    let cite = items.iter().any(|item| item.eq_ignore_ascii_case("cite"));
    answer && cite
}

fn example_of(obj: &Map<String, Value>) -> String {
    if let Some(text) = json_str(obj, &["example", "utterance", "body", "text"]) {
        return text;
    }
    let mut parts = Vec::new();
    for key in ["turns", "dialogue", "messages", "utterances"] {
        let Some(Value::Array(items)) = obj.get(key) else {
            continue;
        };
        for item in items {
            match item {
                Value::String(text) => {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                }
                Value::Object(map) => {
                    if let Some(text) = json_str(map, &["text", "utterance", "body", "example"]) {
                        parts.push(text);
                    }
                }
                _ => {}
            }
        }
    }
    parts.join("\n")
}

fn json_str(obj: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(Value::String(text)) = obj.get(*key) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .collect()
}

fn unique_tokens(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for token in index_tokens(raw) {
        if seen.insert(token.clone()) {
            out.push(token);
        }
    }
    out
}

fn has_name(items: &[String], want: &str) -> bool {
    items.iter().any(|item| item == want)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_queries_skip_lccc_and_stopwords_are_not_a_query() {
        assert_eq!(bundle_lang("lccc", "lccc/pack.json"), BundleLang::Zh);
        assert_eq!(
            bundle_lang("synthetic", "synthetic/soft.json"),
            BundleLang::Und
        );
        assert_eq!(bundle_lang("notes", "notes/pack-zh.json"), BundleLang::Zh);
        assert_eq!(query_lang("i am tired"), Some(QueryLang::En));
        assert_eq!(query_lang("你好"), Some(QueryLang::Zh));
        assert_eq!(query_lang("..."), None);
        assert_eq!(query_lang("the and of"), Some(QueryLang::En));
        assert!(unique_tokens("the and of").is_empty());
        assert_eq!(MAX_RESEARCH_BUNDLE_BYTES, 246 * 1024);
        assert_eq!(STYLE_HIT_CAP, 3);
    }

    #[test]
    fn near_duplicate_hits_collapse_to_the_higher_score() {
        let keep = hit("keep", "keep the evening line short and warm", 2.0);
        let copy = hit("copy", "keep the evening line short and warm please", 1.5);
        let other = hit("other", "talk about the harbour gate", 0.2);
        let kept = dedupe(vec![copy, other.clone(), keep.clone()]);
        assert_eq!(kept[0].id, "keep");
        assert!(kept.iter().all(|item| item.id != "copy"));
        assert!(kept.iter().any(|item| item.id == "other"));
        assert!(!keep.allows_answer());
        assert!(!other.allows_cite());
        assert!(keep.tagged_not_for_answer_and_cite());
    }

    fn hit(id: &str, text: &str, score: f32) -> ResearchHit {
        ResearchHit {
            id: id.to_string(),
            text: text.to_string(),
            licence: "synthetic".to_string(),
            dataset: "synthetic".to_string(),
            lang: "en".to_string(),
            score,
            not_for: vec!["answer".to_string(), "cite".to_string()],
        }
    }

    #[test]
    fn an_unset_flag_reads_nothing() {
        assert!(std::env::var_os("GALION_HORBM_RESEARCH_DIR").is_none());
        let off = ResearchRoot::from_flag(None);
        assert!(off.is_empty());
        assert_eq!(off.files_read(), 0);
        let found = off.search(StylePersona::Samantha, "tired");
        assert!(found.hits.is_empty());
        assert_eq!(found.bundles_opened, 0);
        assert_eq!(off.files_read(), 0);
        let aria = ResearchRoot::from_env();
        assert!(aria.is_empty());
        assert_eq!(aria.files_read(), 0);
    }
}
