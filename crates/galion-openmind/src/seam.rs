// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Local Open Mind / student turn seam.
//!
//! A voice turn may call [`TurnSeam::consult`] at a turn boundary. It must not
//! be called from `Heartbeat::step`. This module does not call Hermes, does
//! not open a socket, and does not create `/opt/galion/openmind`.
//!
//! Two operator-placed files:
//!
//! * `GALION_OPENMIND_STORE` — an existing file, or an existing directory.
//!   A missing path is not created. The directory form uses `turns.jsonl`
//!   inside it. Lines are exact text this process wrote. A later turn can
//!   read an earlier line back. Nothing is paraphrased.
//! * `GALION_STUDENT_GQ` — an existing non-empty `.gq` file. Presence is a
//!   file check. This crate does not load the file and does not speak from it.
//!
//! `remember` defaults to deny (`GALION_OPENMIND_REMEMBER` unset). A denied
//! bit does not write and does not return stored text.
//!
//! If the store or the weights file is absent, the decision is
//! [`PathState::Blocked`] and names the missing piece. The spoken fallback
//! stays the phone lane (canned, or unreachable when that lane already
//! selected Hermes). There is no thought trace.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};

/// Env: existing store file or directory. Unset or missing → no store.
pub const STORE_ENV: &str = "GALION_OPENMIND_STORE";
/// Env: existing non-empty student `.gq`. Unset or missing → no weights.
pub const WEIGHTS_ENV: &str = "GALION_STUDENT_GQ";
/// Env: `1` / `true` / `on` / `yes` grants `remember` for this seam. Default deny.
pub const REMEMBER_ENV: &str = "GALION_OPENMIND_REMEMBER";
/// Env: `1` / `true` / `on` / `yes` makes the phone worker consult this seam.
pub const SEAM_ENV: &str = "GALION_OPENMIND_SEAM";

const MAX_TEXT: usize = 2_000;
const FILE_NAME: &str = "turns.jsonl";

/// Why a write or a recall refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeamBlock {
    /// `GALION_OPENMIND_STORE` is unset or the path is not on disk.
    MissingStore,
    /// `GALION_STUDENT_GQ` is unset, empty, or not a `.gq` file.
    MissingWeights,
    /// Consent `remember` is deny. Nothing was written and nothing was read.
    RememberDenied,
    /// The store could not be read or appended. The detail is for the caller.
    /// It is not turned into a memory.
    StoreUnreadable(String),
    /// The path is `/opt/galion/openmind` or under it. This crate does not
    /// write there and does not create that tree.
    RefusedPath,
    /// Blank text. Not stored.
    EmptyText,
    /// Longer than [`MAX_TEXT`] characters. Not stored, so a prefix cannot
    /// masquerade as the whole fact.
    TooLong,
    /// Role is not `user` or `voice`.
    BadRole,
}

impl std::fmt::Display for SeamBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeamBlock::MissingStore => write!(f, "missing store"),
            SeamBlock::MissingWeights => write!(f, "missing weights"),
            SeamBlock::RememberDenied => write!(f, "remember denied"),
            SeamBlock::StoreUnreadable(_) => write!(f, "store unreadable"),
            SeamBlock::RefusedPath => write!(f, "refused path"),
            SeamBlock::EmptyText => write!(f, "empty text"),
            SeamBlock::TooLong => write!(f, "text too long"),
            SeamBlock::BadRole => write!(f, "bad role"),
        }
    }
}

/// Which piece is absent. Both absent is named, not collapsed into a success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPiece {
    Store,
    Weights,
    StoreAndWeights,
}

impl MissingPiece {
    pub fn as_str(self) -> &'static str {
        match self {
            MissingPiece::Store => "store",
            MissingPiece::Weights => "weights",
            MissingPiece::StoreAndWeights => "store+weights",
        }
    }
}

/// Whether the local Open Mind / student path is on disk.
///
/// [`PathState::Present`] means both files exist. It does not mean the
/// student produced words. This crate does not run the `.gq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathState {
    Present { store: PathBuf, weights: PathBuf },
    Blocked { missing: MissingPiece },
}

/// What a later turn can honestly say it found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryWitness {
    /// Exact text from an earlier line. Not a paraphrase.
    Hit { turn: u64, text: String },
    /// The store is readable and `remember` is granted, and no earlier line exists.
    Empty,
    /// Named piece is missing. `missing` is `store`, `remember`, or `store-unreadable`.
    Blocked { missing: &'static str },
}

/// Spoken fallback when the local path is blocked. This seam does not
/// synthesize either sentence. The phone lane already owns those words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fallback {
    Canned,
    Unreachable,
}

/// What the voice turn should speak. [`ReplyFrom::Stored`] is the exact
/// earlier line. [`ReplyFrom::Lane`] keeps the lane's own reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyFrom {
    Lane,
    Stored { turn: u64, text: String },
}

/// One consult. `hermes_called` is always false. `thought_trace` is always
/// `None`. There is no model monologue in this value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamDecision {
    pub path: PathState,
    pub memory: MemoryWitness,
    pub fallback: Fallback,
    pub reply_from: ReplyFrom,
    pub hermes_called: bool,
}

impl SeamDecision {
    /// No thought trace is produced. A missing model does not get a substitute.
    pub fn thought_trace(&self) -> Option<&str> {
        None
    }

    /// Log token. No transcript, no stored sentence, no path.
    pub fn stage_token(&self) -> String {
        let path = match &self.path {
            PathState::Present { .. } => "present".to_string(),
            PathState::Blocked { missing } => format!("blocked:{}", missing.as_str()),
        };
        let memory = match &self.memory {
            MemoryWitness::Hit { turn, .. } => format!("hit:{turn}"),
            MemoryWitness::Empty => "empty".to_string(),
            MemoryWitness::Blocked { missing } => format!("blocked:{missing}"),
        };
        let fallback = match self.fallback {
            Fallback::Canned => "canned",
            Fallback::Unreachable => "unreachable",
        };
        format!("seam={path} memory={memory} fallback={fallback}")
    }
}

/// Local seam. Constructed from paths that already exist, or from the environment.
#[derive(Debug, Clone)]
pub struct TurnSeam {
    store: Option<PathBuf>,
    weights: Option<PathBuf>,
    remember: bool,
}

impl TurnSeam {
    /// Probe the environment. Does not create a file or a directory.
    pub fn from_env() -> Self {
        let store = std::env::var(STORE_ENV)
            .ok()
            .map(|raw| PathBuf::from(raw.trim()))
            .filter(|p| !p.as_os_str().is_empty());
        let weights = std::env::var(WEIGHTS_ENV)
            .ok()
            .map(|raw| PathBuf::from(raw.trim()))
            .filter(|p| !p.as_os_str().is_empty());
        Self::new(store, weights, remember_flag())
    }

    /// `store` counts only when it is an existing file or directory.
    /// `weights` counts only when [`weights_file_ok`] is true.
    /// A path that is not on disk is dropped. It is not created.
    pub fn new(store: Option<PathBuf>, weights: Option<PathBuf>, remember: bool) -> Self {
        let store = store.filter(|p| p.is_file() || p.is_dir());
        let weights = weights.filter(|p| weights_file_ok(p));
        Self {
            store,
            weights,
            remember,
        }
    }

    pub fn weights_present(&self) -> bool {
        self.weights.is_some()
    }

    pub fn store_present(&self) -> bool {
        self.store.is_some()
    }

    pub fn remember_granted(&self) -> bool {
        self.remember
    }

    /// File check used by the phone host. Same rule as [`TurnSeam::new`].
    pub fn weights_from_env() -> bool {
        std::env::var(WEIGHTS_ENV)
            .ok()
            .map(|raw| PathBuf::from(raw.trim()))
            .is_some_and(|p| weights_file_ok(&p))
    }

    pub fn path_state(&self) -> PathState {
        match (self.store.clone(), self.weights.clone()) {
            (Some(store), Some(weights)) => PathState::Present { store, weights },
            (None, None) => PathState::Blocked {
                missing: MissingPiece::StoreAndWeights,
            },
            (None, Some(_)) => PathState::Blocked {
                missing: MissingPiece::Store,
            },
            (Some(_), None) => PathState::Blocked {
                missing: MissingPiece::Weights,
            },
        }
    }

    /// Append one line when `remember` is granted and the store is already
    /// on disk. Does not create a missing store. Does not write under
    /// `/opt/galion/openmind`.
    pub fn note_turn(&self, turn: u64, role: &str, text: &str) -> Result<(), SeamBlock> {
        if !self.remember {
            return Err(SeamBlock::RememberDenied);
        }
        if role != "user" && role != "voice" {
            return Err(SeamBlock::BadRole);
        }
        let text = sanitize(text)?;
        let path = self.store_file(true)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| SeamBlock::StoreUnreadable(e.to_string()))?;
        writeln!(file, "v1\t{turn}\t{role}\t{text}")
            .map_err(|e| SeamBlock::StoreUnreadable(e.to_string()))?;
        Ok(())
    }

    /// Exact text of the latest line whose turn id is strictly less than
    /// `turn`. `Ok(None)` means the store was read and no earlier line exists.
    /// A missing store or a denied `remember` is an error, not an empty memory.
    pub fn recall_previous(&self, turn: u64) -> Result<Option<(u64, String)>, SeamBlock> {
        if !self.remember {
            return Err(SeamBlock::RememberDenied);
        }
        let path = self.store_file(false)?;
        if !path.is_file() {
            return Ok(None);
        }
        let file = fs::File::open(&path).map_err(|e| SeamBlock::StoreUnreadable(e.to_string()))?;
        let mut found: Option<(u64, String)> = None;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|e| SeamBlock::StoreUnreadable(e.to_string()))?;
            let Some((id, _role, text)) = parse_line(&line) else {
                continue;
            };
            if id < turn && found.as_ref().map(|(prev, _)| id >= *prev).unwrap_or(true) {
                found = Some((id, text.to_string()));
            }
        }
        Ok(found)
    }

    /// Turn-boundary decision. Does not write, does not call Hermes, and
    /// does not invent text. `hermes_selected` only chooses the fallback
    /// label when the path is blocked. The Hermes client is not invoked.
    pub fn consult(&self, turn: u64, hermes_selected: bool) -> SeamDecision {
        let path = self.path_state();
        let (memory, reply_from) = match self.recall_previous(turn) {
            Ok(Some((id, text))) => (
                MemoryWitness::Hit {
                    turn: id,
                    text: text.clone(),
                },
                ReplyFrom::Stored { turn: id, text },
            ),
            Ok(None) => (MemoryWitness::Empty, ReplyFrom::Lane),
            Err(SeamBlock::RememberDenied) => (
                MemoryWitness::Blocked {
                    missing: "remember",
                },
                ReplyFrom::Lane,
            ),
            Err(SeamBlock::StoreUnreadable(_)) => (
                MemoryWitness::Blocked {
                    missing: "store-unreadable",
                },
                ReplyFrom::Lane,
            ),
            Err(_) => (MemoryWitness::Blocked { missing: "store" }, ReplyFrom::Lane),
        };
        let fallback = if hermes_selected && matches!(path, PathState::Blocked { .. }) {
            Fallback::Unreachable
        } else {
            Fallback::Canned
        };
        SeamDecision {
            path,
            memory,
            fallback,
            reply_from,
            hermes_called: false,
        }
    }

    /// `write` refuses `/opt/galion/openmind`. A read of a file a human
    /// already placed there is allowed and does not create anything.
    fn store_file(&self, write: bool) -> Result<PathBuf, SeamBlock> {
        let Some(store) = self.store.as_ref() else {
            return Err(SeamBlock::MissingStore);
        };
        let path = if store.is_file() {
            store.clone()
        } else if store.is_dir() {
            store.join(FILE_NAME)
        } else {
            return Err(SeamBlock::MissingStore);
        };
        if write && (under_opt(&path) || under_opt(store)) {
            return Err(SeamBlock::RefusedPath);
        }
        Ok(path)
    }
}

/// `true` when `GALION_OPENMIND_SEAM` asks the phone worker to consult the seam.
pub fn seam_enabled_from_env() -> bool {
    flag_on(&std::env::var(SEAM_ENV).unwrap_or_default())
}

fn remember_flag() -> bool {
    std::env::var(REMEMBER_ENV)
        .ok()
        .is_some_and(|v| flag_on(&v))
}

fn flag_on(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "on" | "yes"
    )
}

/// Non-empty file whose name ends in `.gq`. Contents are not interpreted.
pub fn weights_file_ok(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path.extension().and_then(|e| e.to_str()) != Some("gq") {
        return false;
    }
    fs::metadata(path).map(|m| m.len() > 0).unwrap_or(false)
}

fn sanitize(text: &str) -> Result<String, SeamBlock> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(SeamBlock::EmptyText);
    }
    if trimmed.chars().count() > MAX_TEXT {
        return Err(SeamBlock::TooLong);
    }
    Ok(trimmed
        .chars()
        .map(|c| match c {
            '\n' | '\r' | '\t' => ' ',
            other => other,
        })
        .collect())
}

fn parse_line(line: &str) -> Option<(u64, &str, &str)> {
    let mut parts = line.splitn(4, '\t');
    let ver = parts.next()?;
    if ver != "v1" {
        return None;
    }
    let turn: u64 = parts.next()?.parse().ok()?;
    let role = parts.next()?;
    if role != "user" && role != "voice" {
        return None;
    }
    let text = parts.next()?;
    if text.is_empty() {
        return None;
    }
    Some((turn, role, text))
}

fn under_opt(path: &Path) -> bool {
    let mut parts = path.components();
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (
            Some(Component::RootDir),
            Some(Component::Normal(a)),
            Some(Component::Normal(b)),
            Some(Component::Normal(c)),
        ) if a == "opt" && b == "galion" && c == "openmind"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opt_prefix_is_the_deploy_tree() {
        assert!(under_opt(Path::new("/opt/galion/openmind")));
        assert!(under_opt(Path::new("/opt/galion/openmind/turns.jsonl")));
        assert!(!under_opt(Path::new("/tmp/galion-openmind")));
        assert!(!under_opt(Path::new("/opt/galion/other")));
        assert!(!under_opt(Path::new("opt/galion/openmind")));
    }

    #[test]
    fn empty_and_non_gq_weights_are_absent() {
        let dir = std::env::temp_dir().join(format!("galion-seam-w-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("model.gq");
        let text = dir.join("notes.txt");
        fs::write(&empty, b"").unwrap();
        fs::write(&text, b"not-weights").unwrap();
        assert!(!weights_file_ok(&empty));
        assert!(!weights_file_ok(&text));
        assert!(!weights_file_ok(dir.join("missing.gq").as_path()));
        let _ = fs::remove_dir_all(&dir);
    }
}
