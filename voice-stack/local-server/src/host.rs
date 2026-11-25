// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Dev profile, absolute data paths, the session seal, and the turn log.
//!
//! Resolved once at startup into [`HostConfig`]. The running server does not
//! call [`std::env::set_var`]. Learn and remember reach the brain as the flag
//! [`galion_sam_horbm::CardEngine::open_with_remember`] already takes, and as
//! [`horbm::LearnConfig::from_flags`] / [`horbm::RetrievalConfig::from_flags`].
//! Library defaults stay deny. Only this host profile grants.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{ClauseSource, LearnConfig, RetrievalConfig, SessionSeal};

use crate::brain::HorbmMind;
use crate::reply::TurnOut;

/// Shipped development profile. Parsed only when development mode is selected.
const SHIPPED_PROFILE: &str = include_str!("../dev.env");

/// Default turn log size before rotation.
pub const DEFAULT_TURN_LOG_CAP: u64 = 8 * 1024 * 1024;
/// Rotated files kept beside the live log: `.1` … `.5`.
pub const DEFAULT_TURN_LOG_KEEP: usize = 5;
const LOG_BOUND: usize = 64;

/// One consent bit after startup resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consent {
    pub granted: bool,
    /// The word passed to `from_flags`. Only `grant` is granted.
    pub word: String,
    /// Logged at startup. Names the flag and where the word came from.
    pub source: String,
}

/// Paths and grants for one process. Owned. Not read from the environment again.
#[derive(Clone, Debug)]
pub struct HostConfig {
    pub dev: bool,
    pub learn: Consent,
    pub remember: Consent,
    pub data_root: PathBuf,
    pub memory_dir: PathBuf,
    pub learn_pack: PathBuf,
    pub diary_path: PathBuf,
    pub turn_log: PathBuf,
    pub turn_log_cap: u64,
    pub turn_log_keep: usize,
    /// Built with [`RetrievalConfig::from_flags`]. The bridge still attaches
    /// its own retrieval thread from the process environment. This host does
    /// not spawn a second thread and does not rewrite that environment.
    pub retrieval: RetrievalConfig,
    pub learn_config: LearnConfig,
}

/// Inputs for [`HostConfig::resolve`]. Borrowed so tests pass values in.
#[derive(Clone, Copy, Debug)]
pub struct HostSources<'a> {
    pub dev: bool,
    pub learn: Option<&'a str>,
    pub remember: Option<&'a str>,
    pub memory_dir: Option<&'a str>,
    pub data_root: Option<&'a str>,
    pub learn_pack: Option<&'a str>,
    pub turn_log: Option<&'a str>,
    pub turn_log_cap: Option<&'a str>,
    /// `dev.env` text. Ignored unless [`Self::dev`] is set.
    pub profile_text: Option<&'a str>,
    pub xdg_data_home: Option<&'a str>,
    pub home: Option<&'a str>,
}

impl HostConfig {
    /// Operator environment, then `--profile dev` when `cli_dev` is set.
    /// Reads each variable once. Does not write the environment.
    pub fn resolve_process(cli_dev: bool) -> Result<Self, String> {
        let profile = std::env::var("GALION_HORBM_PROFILE").ok();
        let learn = std::env::var("GALION_HORBM_LEARN").ok();
        let remember = std::env::var("GALION_HORBM_REMEMBER").ok();
        let memory_dir = std::env::var("GALION_HORBM_MEMORY_DIR").ok();
        let data_root = std::env::var("GALION_HORBM_DATA_ROOT").ok();
        let learn_pack = std::env::var("GALION_HORBM_LEARN_PACK").ok();
        let turn_log = std::env::var("GALION_HORBM_TURN_LOG").ok();
        let turn_log_cap = std::env::var("GALION_HORBM_TURN_LOG_CAP").ok();
        let xdg = std::env::var("XDG_DATA_HOME").ok();
        let home = std::env::var("HOME").ok();
        let profile_name = nonempty(profile.as_deref());
        if let Some(name) = profile_name {
            if name != "dev" && !cli_dev {
                return Err(format!("unknown profile {name}"));
            }
        }
        let dev = cli_dev || profile_name == Some("dev");
        Self::resolve(HostSources {
            dev,
            learn: learn.as_deref(),
            remember: remember.as_deref(),
            memory_dir: memory_dir.as_deref(),
            data_root: data_root.as_deref(),
            learn_pack: learn_pack.as_deref(),
            turn_log: turn_log.as_deref(),
            turn_log_cap: turn_log_cap.as_deref(),
            profile_text: if dev { Some(SHIPPED_PROFILE) } else { None },
            xdg_data_home: xdg.as_deref(),
            home: home.as_deref(),
        })
    }

    /// Pure resolution. `profile_text` is the dev file. It is not read when
    /// `dev` is false, so a grant in that file cannot leak outside development.
    pub fn resolve(sources: HostSources<'_>) -> Result<Self, String> {
        let file = if sources.dev {
            parse_profile(sources.profile_text.unwrap_or(""))
        } else {
            HashMap::new()
        };
        let learn = consent(
            "GALION_HORBM_LEARN",
            sources.learn,
            file.get("GALION_HORBM_LEARN").map(String::as_str),
            sources.dev,
        );
        let remember = consent(
            "GALION_HORBM_REMEMBER",
            sources.remember,
            file.get("GALION_HORBM_REMEMBER").map(String::as_str),
            sources.dev,
        );
        let data_root = match nonempty(sources.data_root)
            .or_else(|| file.get("GALION_HORBM_DATA_ROOT").map(String::as_str))
        {
            Some(raw) => absolute("GALION_HORBM_DATA_ROOT", raw)?,
            None => platform_data_root(sources.xdg_data_home, sources.home)?,
        };
        let memory_dir = pick_path(
            "GALION_HORBM_MEMORY_DIR",
            sources.memory_dir,
            file.get("GALION_HORBM_MEMORY_DIR").map(String::as_str),
            data_root.join("memory"),
        )?;
        let learn_pack = pick_path(
            "GALION_HORBM_LEARN_PACK",
            sources.learn_pack,
            file.get("GALION_HORBM_LEARN_PACK").map(String::as_str),
            memory_dir.join("learn"),
        )?;
        let turn_log = pick_path(
            "GALION_HORBM_TURN_LOG",
            sources.turn_log,
            file.get("GALION_HORBM_TURN_LOG").map(String::as_str),
            data_root.join("logs").join("turns.jsonl"),
        )?;
        let cap_raw = nonempty(sources.turn_log_cap)
            .or_else(|| file.get("GALION_HORBM_TURN_LOG_CAP").map(String::as_str));
        let turn_log_cap = parse_cap(cap_raw)?;
        let mut learn_config = LearnConfig::from_flags(Some(learn.word.as_str()), None);
        learn_config.pack_dir = learn_pack.clone();
        let retrieval = RetrievalConfig::from_flags(Some(remember.word.as_str()), None);
        // `from_flags` is the grant. The stored word and the bool cannot drift.
        let learn = Consent {
            granted: learn_config.granted,
            ..learn
        };
        let remember = Consent {
            granted: retrieval.remember,
            ..remember
        };
        Ok(Self {
            dev: sources.dev,
            learn,
            remember,
            data_root,
            memory_dir: memory_dir.clone(),
            learn_pack,
            diary_path: memory_dir.join("person-memory.sqlite"),
            turn_log,
            turn_log_cap,
            turn_log_keep: DEFAULT_TURN_LOG_KEEP,
            retrieval,
            learn_config,
        })
    }

    /// Absolute deny config for tests and the scripted native session.
    /// Nothing is created. Grants stay off even if the process environment
    /// would have said grant.
    pub fn deny_under(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let memory_dir = root.join("memory");
        let learn_pack = memory_dir.join("learn");
        let off = Consent {
            granted: false,
            word: "deny".into(),
            source: "GALION_HORBM_LEARN unset (off)".into(),
        };
        let mut remember = off.clone();
        remember.source = "GALION_HORBM_REMEMBER unset (off)".into();
        let mut learn_config = LearnConfig::from_flags(Some("deny"), None);
        learn_config.pack_dir = learn_pack.clone();
        Self {
            dev: false,
            learn: off,
            remember,
            data_root: root.clone(),
            memory_dir: memory_dir.clone(),
            learn_pack,
            diary_path: memory_dir.join("person-memory.sqlite"),
            turn_log: root.join("logs").join("turns.jsonl"),
            turn_log_cap: DEFAULT_TURN_LOG_CAP,
            turn_log_keep: DEFAULT_TURN_LOG_KEEP,
            retrieval: RetrievalConfig::from_flags(Some("deny"), None),
            learn_config,
        }
    }

    /// Paths and grants. No secrets and no bind address.
    /// `retrieval_remember` is the typed [`RetrievalConfig`]. The card
    /// engine's retrieval thread still reads the process environment.
    pub fn startup_line(&self) -> String {
        format!(
            "horbm host: profile={} learn={} ({}) remember={} ({}) data_root={} memory_dir={} diary={} learn_pack={} turn_log={} turn_log_cap={} retrieval_remember={} (typed config)",
            if self.dev { "dev" } else { "off" },
            if self.learn.granted { "grant" } else { "deny" },
            self.learn.source,
            if self.remember.granted { "grant" } else { "deny" },
            self.remember.source,
            self.data_root.display(),
            self.memory_dir.display(),
            self.diary_path.display(),
            self.learn_pack.display(),
            self.turn_log.display(),
            self.turn_log_cap,
            self.retrieval.remember
        )
    }
}

/// The live process: one shared mind, one seal, one log.
pub struct HostSession {
    config: HostConfig,
    knowledge: PathBuf,
    mind: Arc<Mutex<HorbmMind>>,
    seal: Mutex<SessionSeal>,
    log: TurnLog,
    session_id: String,
    turns: AtomicU64,
    finished: std::sync::atomic::AtomicBool,
}

impl HostSession {
    /// Create the memory directory only when remember or learn is granted.
    /// Opens the mind, the seal, and the log thread. Does not listen.
    pub fn boot(config: HostConfig, knowledge: PathBuf) -> Result<Arc<Self>, String> {
        if config.remember.granted || config.learn.granted {
            fs::create_dir_all(&config.memory_dir)
                .map_err(|err| format!("create memory dir: {err}"))?;
        }
        if config.learn.granted {
            fs::create_dir_all(&config.learn_pack)
                .map_err(|err| format!("create learn pack: {err}"))?;
        }
        if let Some(parent) = config.turn_log.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| format!("create turn log dir: {err}"))?;
            }
        }
        let mind = Arc::new(Mutex::new(HorbmMind::open_configured(&knowledge, &config)));
        let seal = Mutex::new(SessionSeal::open(config.learn_config.clone()));
        let log = TurnLog::open(&config.turn_log, config.turn_log_cap, config.turn_log_keep)?;
        Ok(Arc::new(Self {
            config,
            knowledge,
            mind,
            seal,
            log,
            session_id: new_session_id(),
            turns: AtomicU64::new(0),
            finished: std::sync::atomic::AtomicBool::new(false),
        }))
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn mind(&self) -> Arc<Mutex<HorbmMind>> {
        Arc::clone(&self.mind)
    }

    pub fn startup_line(&self) -> String {
        let mind = self.mind.lock().unwrap_or_else(|err| err.into_inner());
        format!(
            "{} persona={} knowledge_cards={} knowledge_dir={}",
            self.config.startup_line(),
            mind.persona_id(),
            mind.knowledge_cards(),
            self.knowledge.display()
        )
    }

    /// One finished heard line. Notes the seal when learn is granted and
    /// appends `heard` and `reply` off the turn path. A denied seal is not
    /// given the text.
    pub fn on_turn(&self, heard: &str, out: &TurnOut) {
        if self.finished.load(Ordering::SeqCst) {
            return;
        }
        let heard = heard.trim();
        if heard.is_empty() {
            return;
        }
        let turn_id = self.turns.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        let at = timestamp_now();
        if self.config.learn.granted && !withhold(heard) {
            let person = self
                .mind
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .person_id();
            let source = ClauseSource {
                session: self.session_id.clone(),
                turn_id,
                time: at.clone(),
                consent_record: self.config.learn.source.clone(),
            };
            self.seal
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .note(&person, heard, &source);
        }
        self.log.record(event_line(
            &at,
            &self.session_id,
            &serde_json::json!({
                "kind": "heard",
                "turn": turn_id,
                "text": public_text(heard),
            }),
        ));
        self.log.record(event_line(
            &at,
            &self.session_id,
            &serde_json::json!({
                "kind": "reply",
                "turn": turn_id,
                "text": public_text(&out.reply),
                "sources": out.sources,
            }),
        ));
    }

    pub fn on_inner(&self, verb: &str, lean: &str, surprise: f32, learned: bool, slots: &[String]) {
        if self.finished.load(Ordering::SeqCst) {
            return;
        }
        let at = timestamp_now();
        self.log.record(event_line(
            &at,
            &self.session_id,
            &serde_json::json!({
                "kind": "inner",
                "verb": verb,
                "lean": lean,
                "surprise": surprise,
                "learned": learned,
                "slots": slots,
            }),
        ));
    }

    pub fn on_mood(&self, arousal: f32, valence: f32, dominance: f32) {
        if self.finished.load(Ordering::SeqCst) {
            return;
        }
        let at = timestamp_now();
        self.log.record(event_line(
            &at,
            &self.session_id,
            &serde_json::json!({
                "kind": "mood",
                "arousal": arousal,
                "valence": valence,
                "dominance": dominance,
            }),
        ));
    }

    /// Diary `end_session`, then [`SessionSeal::seal`]. Both are no-ops when
    /// that consent is denied. Not called from [`Drop`]. A crash writes nothing.
    pub fn finish(&self) {
        if self.finished.swap(true, Ordering::SeqCst) {
            return;
        }
        let at = timestamp_now();
        {
            let mut mind = self.mind.lock().unwrap_or_else(|err| err.into_inner());
            let _ = mind.end_diary_session(&at);
        }
        match self
            .seal
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .seal()
        {
            Ok(_) => {}
            Err(err) if err.is_denied() => {}
            Err(err) => eprintln!("horbm learn: {err}"),
        }
        self.log.record_blocking(event_line(
            &at,
            &self.session_id,
            &serde_json::json!({ "kind": "session_end" }),
        ));
        self.log.close();
    }
}

fn event_line(at: &str, session: &str, fields: &serde_json::Value) -> String {
    let mut line = serde_json::json!({
        "ts": at,
        "session": session,
    });
    if let (Some(obj), Some(extra)) = (line.as_object_mut(), fields.as_object()) {
        for (key, value) in extra {
            obj.insert(key.clone(), value.clone());
        }
    }
    line.to_string()
}

fn public_text(text: &str) -> String {
    if withhold(text) || has_ipv4(text) {
        "withheld".into()
    } else {
        text.to_string()
    }
}

fn withhold(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
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
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn has_ipv4(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            if let Some(end) = ipv4_at(bytes, i) {
                let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
                let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
                if before_ok && after_ok {
                    return true;
                }
            }
        }
        i += 1;
    }
    false
}

fn ipv4_at(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    for part in 0..4 {
        if part > 0 {
            if i >= bytes.len() || bytes[i] != b'.' {
                return None;
            }
            i += 1;
        }
        let begin = i;
        let mut value = 0u32;
        let mut digits = 0u32;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            digits += 1;
            if digits > 3 {
                return None;
            }
            value = value * 10 + u32::from(bytes[i] - b'0');
            if value > 255 {
                return None;
            }
            i += 1;
        }
        if digits == 0 || i == begin {
            return None;
        }
    }
    Some(i)
}

/// Bounded JSONL writer. `record` never waits. A full channel drops and counts.
pub struct TurnLog {
    tx: SyncSender<LogMsg>,
    dropped: Arc<AtomicU64>,
    join: Mutex<Option<JoinHandle<()>>>,
}

enum LogMsg {
    Line(String),
    Stop,
}

impl TurnLog {
    pub fn open(path: &Path, cap: u64, keep: usize) -> Result<Self, String> {
        if !path.is_absolute() {
            return Err(format!(
                "GALION_HORBM_TURN_LOG must be an absolute path (refusing {})",
                path.display()
            ));
        }
        let path = path.to_path_buf();
        let file = open_append(&path).map_err(|err| format!("open turn log: {err}"))?;
        let (tx, rx) = sync_channel(LOG_BOUND);
        let dropped = Arc::new(AtomicU64::new(0));
        let join = std::thread::Builder::new()
            .name("horbm-turn-log".into())
            .spawn(move || writer_loop(rx, path, cap, keep.max(1), file))
            .map_err(|err| format!("spawn turn log: {err}"))?;
        Ok(Self {
            tx,
            dropped,
            join: Mutex::new(Some(join)),
        })
    }

    pub fn record(&self, line: String) {
        offer(&self.tx, &self.dropped, LogMsg::Line(line));
    }

    /// Shutdown path. May wait for a free slot. A turn does not call this.
    pub fn record_blocking(&self, line: String) {
        let _ = self.tx.send(LogMsg::Line(line));
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        let _ = self.tx.send(LogMsg::Stop);
        if let Some(join) = self
            .join
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .take()
        {
            let _ = join.join();
        }
    }
}

impl Drop for TurnLog {
    fn drop(&mut self) {
        self.close();
    }
}

fn offer(tx: &SyncSender<LogMsg>, dropped: &AtomicU64, msg: LogMsg) -> bool {
    match tx.try_send(msg) {
        Ok(()) => true,
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            dropped.fetch_add(1, Ordering::Relaxed);
            false
        }
    }
}

fn writer_loop(rx: Receiver<LogMsg>, path: PathBuf, cap: u64, keep: usize, file: File) {
    let mut slot = Some(file);
    while let Ok(msg) = rx.recv() {
        match msg {
            LogMsg::Stop => break,
            LogMsg::Line(line) => {
                if let Err(err) = write_line(&mut slot, &path, cap, keep, &line) {
                    eprintln!("horbm turn log: {err}");
                }
            }
        }
    }
}

fn write_line(
    slot: &mut Option<File>,
    path: &Path,
    cap: u64,
    keep: usize,
    line: &str,
) -> io::Result<()> {
    let len = slot
        .as_ref()
        .ok_or_else(|| io::Error::other("turn log closed"))?
        .metadata()?
        .len();
    if cap > 0 && len >= cap {
        if let Some(file) = slot.as_mut() {
            file.flush()?;
        }
        // Close before the rename so the live name is free on every platform.
        *slot = None;
        rotate(path, keep)?;
        *slot = Some(open_append(path)?);
    }
    let file = slot
        .as_mut()
        .ok_or_else(|| io::Error::other("turn log closed"))?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()?;
    Ok(())
}

fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn rotate(path: &Path, keep: usize) -> io::Result<()> {
    let _ = fs::remove_file(rotated(path, keep));
    for n in (1..keep).rev() {
        let from = rotated(path, n);
        if from.exists() {
            let _ = fs::rename(&from, rotated(path, n + 1));
        }
    }
    if path.exists() {
        fs::rename(path, rotated(path, 1))?;
    }
    Ok(())
}

fn rotated(path: &Path, n: usize) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{n}"));
    path.with_file_name(name)
}

fn consent(name: &str, operator: Option<&str>, file: Option<&str>, dev: bool) -> Consent {
    if let Some(word) = nonempty(operator) {
        return Consent {
            granted: word == "grant",
            word: word.to_string(),
            source: format!("{name}={word} (operator)"),
        };
    }
    if let Some(word) = nonempty(file) {
        return Consent {
            granted: word == "grant",
            word: word.to_string(),
            source: format!("{name}={word} (dev profile)"),
        };
    }
    if dev {
        return Consent {
            granted: true,
            word: "grant".into(),
            source: format!("{name}=grant (dev profile)"),
        };
    }
    Consent {
        granted: false,
        word: "deny".into(),
        source: format!("{name} unset (off)"),
    }
}

fn pick_path(
    label: &str,
    operator: Option<&str>,
    file: Option<&str>,
    fallback: PathBuf,
) -> Result<PathBuf, String> {
    if let Some(raw) = nonempty(operator) {
        return absolute(label, raw);
    }
    if let Some(raw) = nonempty(file) {
        return absolute(label, raw);
    }
    if !fallback.is_absolute() {
        return Err(format!(
            "{label} default is not absolute ({})",
            fallback.display()
        ));
    }
    Ok(fallback)
}

fn absolute(label: &str, raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute path (refusing {raw})"));
    }
    Ok(path)
}

fn parse_cap(raw: Option<&str>) -> Result<u64, String> {
    let Some(text) = raw else {
        return Ok(DEFAULT_TURN_LOG_CAP);
    };
    let n: u64 = text
        .parse()
        .map_err(|_| format!("GALION_HORBM_TURN_LOG_CAP must be a byte count, got {text}"))?;
    if n == 0 {
        return Err("GALION_HORBM_TURN_LOG_CAP must be greater than zero".into());
    }
    Ok(n)
}

fn parse_profile(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim().to_string();
        if value.is_empty() {
            continue;
        }
        out.insert(key.to_string(), value);
    }
    out
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|text| !text.is_empty())
}

fn platform_data_root(xdg: Option<&str>, home: Option<&str>) -> Result<PathBuf, String> {
    if let Some(xdg) = nonempty(xdg) {
        return absolute(
            "XDG_DATA_HOME",
            &PathBuf::from(xdg).join("galion/horbm").to_string_lossy(),
        );
    }
    #[cfg(target_os = "linux")]
    {
        let home = nonempty(home).ok_or_else(|| {
            "HOME is unset; set GALION_HORBM_DATA_ROOT to an absolute directory".to_string()
        })?;
        absolute(
            "HOME",
            &PathBuf::from(home)
                .join(".local/share/galion/horbm")
                .to_string_lossy(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = home;
        Err("set GALION_HORBM_DATA_ROOT to an absolute directory".into())
    }
}

fn new_session_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut left = RandomState::new().build_hasher();
    let mut right = RandomState::new().build_hasher();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    left.write_u64(nanos as u64);
    left.write_u64((nanos >> 64) as u64);
    right.write_u64(left.finish());
    format!("{:016x}{:016x}", left.finish(), right.finish())
}

fn timestamp_now() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}Z", format_unix(dur.as_secs()), dur.subsec_millis())
}

fn format_unix(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let hour = tod / 3_600;
    let minute = (tod % 3_600) / 60;
    let second = tod % 60;
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// Test hook: a full channel drops and does not block.
#[cfg(test)]
pub(crate) fn offer_drops_when_full() -> u64 {
    let (tx, _rx) = sync_channel(1);
    let dropped = AtomicU64::new(0);
    assert!(offer(&tx, &dropped, LogMsg::Line("one".into())));
    assert!(!offer(&tx, &dropped, LogMsg::Line("two".into())));
    dropped.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_formats_as_utc() {
        assert_eq!(format_unix(0), "1970-01-01T00:00:00");
    }

    #[test]
    fn a_full_log_channel_drops() {
        assert_eq!(offer_drops_when_full(), 1);
    }

    #[test]
    fn withheld_text_is_not_the_secret() {
        assert_eq!(public_text("my password is hunter2"), "withheld");
        assert_eq!(public_text("where is the kettle"), "where is the kettle");
        let octets = [10_u8, 1, 2, 3];
        let dotted = format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]);
        assert!(has_ipv4(&format!("see {dotted} now")));
        assert!(!has_ipv4("the kettle is on the stove"));
    }
}
