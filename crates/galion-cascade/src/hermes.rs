//! Hermes reply client. Std only: one `TcpStream` to a loopback OpenAI-compatible
//! chat-completions endpoint. No TLS, no new crate, no public host.
//!
//! ```text
//! GALION_LLM=hermes
//! GALION_LLM_URL=http://127.0.0.1:8642/v1/chat/completions
//! GALION_LLM_MODEL=hermes-agent
//! GALION_LLM_KEY_FILE=...       # optional dotenv path (same shape as GALION_RESEARCH_KEY_FILE)
//! GALION_LLM_KEY_NAME=API_SERVER_KEY
//! GALION_LLM_KEY=...            # fallback when the file has no key; never logged
//! GALION_LLM_TIMEOUT_MS=15000
//! ```
//!
//! A timeout, a refused connect, a non-loopback URL, or a bad response is the
//! plain line [`UNREACHABLE`]. This client does not call [`crate::CannedLlm`].

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use crate::{CascadeError, Emotion, Llm, Transcript};

/// Spoken when the gateway does not answer. Not a canned intent line.
pub const UNREACHABLE: &str = "model can't be reached";
/// Galion Research desk. A voice of Galion One, with Hermes as the agent behind it.
/// Citations are ids from the Sources list on this turn. A missing source is
/// Inferred or Unknown. The desk drops any id that was not provided.
pub const RESEARCH_PROMPT: &str = "You are Galion Research, the research app on Hermes, a voice of Galion One. \
Answer the question first, then the limit. \
Separate found, inferred, and unknown under those headings, each heading on its own line. \
A Found line is a claim a provided source states. End it with that source id, written [n]. \
An Inferred line is a step you reasoned and no provided source states. \
An Unknown line is a claim you cannot settle from the sources. \
Cite only an id from the Sources list. One citation per line, written [n] and the source name. \
If the Sources list says none, write no citation and leave Found empty. \
Do not invent a paper, a URL, a date, a measurement, or a source id. \
Do not write that you have no source. English only.";
/// How many attached files one turn hands to the model.
pub const MAX_RESEARCH_SOURCES: usize = 8;
/// Characters of each file kept in the prompt and on the desk.
pub const MAX_RESEARCH_SOURCE_CHARS: usize = 2_000;
/// Helper sub-questions one hard ask may run. `GALION_RESEARCH_HELPERS`, default 3.
pub const DEFAULT_RESEARCH_HELPERS: usize = 3;
/// Hard cap. A larger env value is clamped here.
pub const MAX_RESEARCH_HELPERS: usize = 5;
/// Helper calls in flight at once. `GALION_RESEARCH_HELPER_CONCURRENCY`, default 2.
pub const DEFAULT_RESEARCH_CONCURRENCY: usize = 2;
/// Wall clock for one ask, including the plan and the synthesis.
/// `GALION_RESEARCH_BUDGET_MS`, default 180000, clamped 1000..=300000 from the env.
pub const DEFAULT_RESEARCH_BUDGET_MS: u64 = 180_000;
pub const MAX_RESEARCH_BUDGET_MS: u64 = 300_000;
/// Planner + hard-capped helpers + synthesis. `GALION_RESEARCH_CALLS` cannot exceed this.
pub const MAX_RESEARCH_CALLS: u32 = 7;

/// Spoken when the model returns a forbidden empathy script.
pub const REFUSED: &str = "I'll stay on what you said.";
/// Stable client id when no model name is configured.
pub const LLM_NAME: &str = "hermes-agent";
/// Approved gateway when `GALION_LLM_URL` is unset and the mode is hermes.
pub const DEFAULT_URL: &str = "http://127.0.0.1:8642/v1/chat/completions";
/// Failure ceiling for one live voice completion. Not a measured reply time.
/// A local CPU generation does not fit in the old 3 s ceiling.
/// `GALION_LLM_TIMEOUT_MS` overrides this (clamped 50..=30000).
pub const DEFAULT_TIMEOUT_MS: u64 = 15_000;
const MIN_TIMEOUT_MS: u64 = 50;
const MAX_TIMEOUT_MS: u64 = 30_000;

/// `GALION_LLM_TIMEOUT_MS`. Unset, blank, and non-numeric values use
/// [`DEFAULT_TIMEOUT_MS`]. Values outside 50..=30000 are clamped.
fn timeout_ms_from_choice(raw: Option<&str>) -> u64 {
    let parsed = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u64>().ok());
    parsed.unwrap_or(DEFAULT_TIMEOUT_MS).clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}

/// Whether [`HermesLlm::attached_from_env`] builds a client.
/// Unset and `hermes` attach. `canned`, `echo`, and `mock` do not.
fn should_attach(mode: Option<&str>) -> bool {
    let owned = mode.map(|s| s.trim().to_ascii_lowercase());
    match owned.as_deref().filter(|s| !s.is_empty()) {
        Some("canned") | Some("echo") | Some("mock") => false,
        Some("hermes") | None => true,
        Some(_) => false,
    }
}

/// Empathy scripts the model must not speak. Same list the `/v2` floor uses.
/// This is the one list: `scripts/eval/v2_hard_loop.py` parses it from this
/// file, and `forbidden_list_is_mirrored_by_the_hard_harness` keeps them in sync.
pub const FORBIDDEN: &[&str] = &[
    "i hear you",
    "that sounds so hard",
    "that sounds hard",
    "i'm here for you",
    "i feel that in my soul",
    "i know this is difficult",
    "i'm worried about you",
    "take a deep breath",
    "so what i'm hearing",
    "noted, that is hard",
    "i'm sorry for your loss",
    "let's slow down",
    "i'm sorry",
    "i am sorry",
    "i'm so sorry",
    "i am so sorry",
    "so sorry to hear",
    "sorry to hear",
    "sorry for your loss",
    "my condolences",
    "my deepest condolences",
    "you are not alone",
    "you're not alone",
    "allow yourself to grieve",
    "losing a loved one",
    "i can only imagine",
    "i can't imagine how",
    "that must be so hard",
    "that must be hard",
    "that must be difficult",
];

pub fn contains_forbidden(reply: &str) -> bool {
    // Curly apostrophes and runs of whitespace must not slip a phrase past the list.
    let low: String = reply
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '`' => '\'',
            c => c.to_ascii_lowercase(),
        })
        .collect();
    let low = low.split_whitespace().collect::<Vec<_>>().join(" ");
    FORBIDDEN.iter().any(|p| low.contains(p))
}

/// Loopback chat endpoint. Host is numeric; the path is the POST target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HermesTarget {
    pub addr: SocketAddr,
    pub path: String,
}

/// `http://127.0.0.1:8642/v1/chat/completions` or `http://[::1]:8642/...`.
/// HTTPS, DNS names, userinfo, and non-loopback addresses are `None`.
pub fn parse_loopback_chat_url(url: &str) -> Option<HermesTarget> {
    let rest = url.trim().strip_prefix("http://")?;
    if rest.contains('@') || rest.contains(' ') {
        return None;
    }
    let (hostport, raw_path) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let path = raw_path.split(['?', '#']).next().unwrap_or("");
    if path.contains(['\r', '\n']) {
        return None;
    }
    let path = if path.is_empty() {
        "/v1/chat/completions".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        return None;
    };
    let (host, port) = if let Some(bracketed) = hostport.strip_prefix('[') {
        bracketed.split_once("]:")?
    } else {
        let (host, port) = hostport.rsplit_once(':')?;
        if host.is_empty() || host.contains(':') {
            return None;
        }
        (host, port)
    };
    if host.is_empty() || port.is_empty() {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    let ip: IpAddr = host.parse().ok()?;
    if !ip.is_loopback() {
        return None;
    }
    Some(HermesTarget {
        addr: SocketAddr::new(ip, port),
        path,
    })
}

/// Short register prompt. The words stay on the matter. Empathy is prosody, not a script.
/// No product framing: the persona is a voice on a call, not a support desk.
/// The ban list in the prompt is [`FORBIDDEN`] itself, so the prompt and the
/// guard cannot drift.
pub fn system_prompt(persona: &str) -> &'static str {
    use std::sync::OnceLock;
    static SAMANTHA: OnceLock<String> = OnceLock::new();
    static JARVIS: OnceLock<String> = OnceLock::new();
    if persona == "jarvis" {
        JARVIS.get_or_init(|| {
            prompt_with_bans(
                "You are Jarvis, a calm and exact voice on a call. You are not a film voice and you have no clip. \
Say the fact they named back in your own words, then stay on it. One or two short spoken sentences.",
            )
        })
    } else {
        SAMANTHA.get_or_init(|| {
            prompt_with_bans(
                "You are Samantha, a warm voice on a call. You are not a film voice and you have no clip. \
Say the fact they named back in your own words, then stay on it. One or two short spoken sentences.",
            )
        })
    }
}

fn prompt_with_bans(head: &str) -> String {
    let mut s = String::from(head);
    s.push_str(" Do not use a canned empathy script. Never say any of these: ");
    for (i, p) in FORBIDDEN.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push('"');
        s.push_str(p);
        s.push('"');
    }
    s.push_str(". Do not claim you feel their pain. No film quote. English only.");
    s
}

/// One file the desk handed to this turn. A citation may name only these ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchSource {
    pub id: u32,
    pub name: String,
    /// Plain text from the file. Empty when nothing was read (an image).
    pub text: String,
}

/// One Hermes research reply. `citations` name only sources that were handed
/// in. `standing` is `Found`, `Inferred`, `Unknown`, `Unreachable`, or `Denied`.
/// Neither field carries the gateway key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchAnswer {
    pub text: String,
    pub citations: Vec<(u32, String)>,
    pub found: Vec<String>,
    pub inferred: Vec<String>,
    pub unknown: Vec<String>,
    pub standing: &'static str,
}

/// How many helper lookups one ask may spend. Read from `GALION_RESEARCH_*`.
/// `helpers` is at most [`MAX_RESEARCH_HELPERS`]. `calls` is at most [`MAX_RESEARCH_CALLS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchBudget {
    pub helpers: usize,
    pub concurrency: usize,
    /// Wall clock for the whole ask. A helper does not start when this is spent.
    pub budget: Duration,
    /// Hermes calls this ask may make, including the plan and the synthesis.
    pub calls: u32,
}

impl Default for ResearchBudget {
    fn default() -> Self {
        Self {
            helpers: DEFAULT_RESEARCH_HELPERS,
            concurrency: DEFAULT_RESEARCH_CONCURRENCY,
            budget: Duration::from_millis(DEFAULT_RESEARCH_BUDGET_MS),
            calls: default_call_budget(DEFAULT_RESEARCH_HELPERS),
        }
    }
}

impl ResearchBudget {
    /// Env values, clamped. Missing keys keep the defaults. `0` helpers disables the split.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        let get = |k: &str| {
            var(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let helpers = get("GALION_RESEARCH_HELPERS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RESEARCH_HELPERS)
            .min(MAX_RESEARCH_HELPERS);
        let concurrency = get("GALION_RESEARCH_HELPER_CONCURRENCY")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RESEARCH_CONCURRENCY);
        let budget_ms = get("GALION_RESEARCH_BUDGET_MS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_RESEARCH_BUDGET_MS)
            .clamp(1_000, MAX_RESEARCH_BUDGET_MS);
        let auto = default_call_budget(helpers);
        let calls = match get("GALION_RESEARCH_CALLS") {
            Some(s) => s.parse::<u32>().ok().unwrap_or(auto),
            None => auto,
        };
        Self {
            helpers,
            concurrency,
            budget: Duration::from_millis(budget_ms),
            calls,
        }
        .clamped()
    }

    /// Hard caps. A test may set a short budget; the env parser's floor is 1000 ms.
    pub fn clamped(self) -> Self {
        let helpers = self.helpers.min(MAX_RESEARCH_HELPERS);
        let concurrency = self.concurrency.clamp(1, MAX_RESEARCH_HELPERS);
        let ms = self
            .budget
            .as_millis()
            .clamp(50, MAX_RESEARCH_BUDGET_MS as u128) as u64;
        let calls = self.calls.clamp(1, MAX_RESEARCH_CALLS);
        Self {
            helpers,
            concurrency,
            budget: Duration::from_millis(ms),
            calls,
        }
    }
}

fn default_call_budget(helpers: usize) -> u32 {
    if helpers == 0 {
        1
    } else {
        (1 + helpers as u32 + 1).min(MAX_RESEARCH_CALLS)
    }
}

/// One helper the planner queued. `status` is `queued`, `running`, `skipped`,
/// or a research standing (`Found`, `Inferred`, `Unknown`, `Unreachable`, `Denied`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchHelper {
    pub question: String,
    pub status: &'static str,
}

/// One desk ask. `calls` is how many Hermes completions this ask spent.
/// `helpers` is empty when the question was not split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchReport {
    pub answer: ResearchAnswer,
    pub calls: u32,
    pub call_budget: u32,
    pub split: bool,
    pub helpers: Vec<ResearchHelper>,
}

/// Progress the desk paints while an ask is in flight. The list stays collapsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchEvent {
    Phase(&'static str),
    Calls(u32),
    Budget(u32),
    Helper(usize, ResearchHelper),
}

/// Sink for [`ResearchEvent`]. The desk lock is taken only inside the impl, never across a call.
pub trait ResearchWatch: Send + Sync {
    fn on_research(&self, event: &ResearchEvent);
}

/// Watch that drops events. Tests and the single-call path use it.
pub struct ResearchQuiet;

impl ResearchWatch for ResearchQuiet {
    fn on_research(&self, _event: &ResearchEvent) {}
}

/// OpenAI-compatible chat client. `Send`. Connects only when [`HermesLlm::complete_as`] runs.
#[derive(Clone)]
pub struct HermesLlm {
    target: Option<HermesTarget>,
    model: String,
    key: String,
    timeout: Duration,
    persona: String,
    budget: ResearchBudget,
}

impl std::fmt::Debug for HermesLlm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HermesLlm")
            .field("target", &self.target)
            .field("model", &self.model)
            .field("key", &if self.key.is_empty() { "none" } else { "set" })
            .field("timeout", &self.timeout)
            .field("persona", &self.persona)
            .field("helpers", &self.budget.helpers)
            .field("calls", &self.budget.calls)
            .finish()
    }
}

impl HermesLlm {
    /// Build a client. A URL that is not numeric loopback `http://` leaves the
    /// client unable to connect; replies are then [`UNREACHABLE`].
    pub fn new(url: &str, model: &str, key: &str, timeout: Duration) -> Self {
        let timeout = if timeout.is_zero() {
            Duration::from_millis(DEFAULT_TIMEOUT_MS)
        } else {
            timeout
        };
        let model = {
            let m = model.trim();
            if m.is_empty() {
                LLM_NAME.to_string()
            } else {
                m.chars().take(64).collect()
            }
        };
        Self {
            target: parse_loopback_chat_url(url),
            model,
            key: key.trim().to_string(),
            timeout,
            persona: "samantha".into(),
            budget: ResearchBudget::default(),
        }
    }

    /// `GALION_LLM_URL` (default [`DEFAULT_URL`]), `GALION_LLM_MODEL`,
    /// `GALION_LLM_TIMEOUT_MS` (default [`DEFAULT_TIMEOUT_MS`], clamped 50..=30000).
    ///
    /// Key: one `NAME=value` line named by `GALION_LLM_KEY_NAME` in the dotenv
    /// file `GALION_LLM_KEY_FILE`, the same shape as [`HermesLlm::research_from_env`]
    /// (`GALION_RESEARCH_KEY_FILE` / `GALION_RESEARCH_KEY_NAME`). `GALION_LLM_KEY`
    /// is the fallback when that file is unset, unreadable, or has no matching
    /// line. The key is never logged.
    pub fn from_env() -> Self {
        Self::from_vars(|k| std::env::var(k).ok(), |p| std::fs::read_to_string(p).ok())
    }

    /// [`HermesLlm::from_env`] with injected lookups (tests).
    pub fn from_vars(
        var: impl Fn(&str) -> Option<String>,
        read: impl Fn(&str) -> Option<String>,
    ) -> Self {
        let get = |k: &str| var(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let url = var("GALION_LLM_URL").unwrap_or_else(|| DEFAULT_URL.to_string());
        let model = var("GALION_LLM_MODEL").unwrap_or_else(|| LLM_NAME.to_string());
        let key = llm_gateway_key(&get, &read);
        let timeout_ms = timeout_ms_from_choice(var("GALION_LLM_TIMEOUT_MS").as_deref());
        Self::new(url.trim(), model.trim(), &key, Duration::from_millis(timeout_ms))
    }

    /// Attach only when the process asked for Hermes.
    ///
    /// `GALION_LLM=hermes`, or no explicit mode at all (owner card t493s3301:
    /// Hermes is the default phone brain). `canned`, `echo`, and `mock` stay off this client. An unknown mode does
    /// not attach and does not select canned; the phone lane reports that as an error.
    pub fn attached_from_env() -> Option<Self> {
        let mode = std::env::var("GALION_LLM")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase());
        let attach = should_attach(mode.as_deref());
        if attach {
            Some(Self::from_env())
        } else {
            None
        }
    }

    /// Galion Research client, separate from the voice LLM (`GALION_LLM` stays as it is).
    ///
    /// `GALION_RESEARCH_URL` (loopback chat URL) turns it on; unset or not loopback is `None`,
    /// and the desk answers from local files and notes, labelled Offline. `GALION_RESEARCH_MODEL` (default
    /// `hermes-agent`), `GALION_RESEARCH_TIMEOUT_MS` (default 60000, clamped 1000..=90000).
    /// Key: `GALION_RESEARCH_KEY`, or one `NAME=value` line named by `GALION_RESEARCH_KEY_NAME`
    /// in the dotenv file `GALION_RESEARCH_KEY_FILE`. The key is never logged or returned.
    ///
    /// A hard question is split into at most `GALION_RESEARCH_HELPERS` sub-questions
    /// (default [`DEFAULT_RESEARCH_HELPERS`], hard cap [`MAX_RESEARCH_HELPERS`]).
    /// `GALION_RESEARCH_HELPER_CONCURRENCY` bounds how many run at once.
    /// `GALION_RESEARCH_BUDGET_MS` is the wall clock for the whole ask.
    /// `GALION_RESEARCH_CALLS` caps completions (default one plan, the helpers, and one synthesis,
    /// hard cap [`MAX_RESEARCH_CALLS`]). A simple question does not spend the plan.
    pub fn research_from_env() -> Option<Self> {
        Self::research_from_vars(
            |k| std::env::var(k).ok(),
            |p| std::fs::read_to_string(p).ok(),
        )
    }


    /// [`HermesLlm::research_from_env`] with injected lookups (tests).
    pub fn research_from_vars(
        var: impl Fn(&str) -> Option<String>,
        read: impl Fn(&str) -> Option<String>,
    ) -> Option<Self> {
        let get = |k: &str| var(k).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let url = get("GALION_RESEARCH_URL")?;
        parse_loopback_chat_url(&url)?;
        let model = get("GALION_RESEARCH_MODEL").unwrap_or_else(|| LLM_NAME.to_string());
        let key = match get("GALION_RESEARCH_KEY") {
            Some(k) => k,
            None => match (get("GALION_RESEARCH_KEY_FILE"), get("GALION_RESEARCH_KEY_NAME")) {
                (Some(file), Some(name)) => read(&file).and_then(|t| dotenv_value(&t, &name)).unwrap_or_default(),
                _ => String::new(),
            },
        };
        let timeout_ms: u64 = get("GALION_RESEARCH_TIMEOUT_MS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(60_000)
            .clamp(1_000, 90_000);
        let mut llm = Self::new(&url, &model, &key, Duration::from_millis(timeout_ms));
        llm.budget = ResearchBudget::from_vars(&var);
        Some(llm)
    }

    /// Helper cap, concurrency, and the time and call budgets for one research ask.
    pub fn research_budget(&self) -> &ResearchBudget {
        &self.budget
    }

    /// Replace the helper budget. Values above the hard caps are clamped.
    pub fn set_research_budget(&mut self, budget: ResearchBudget) {
        self.budget = budget.clamped();
    }


    /// `hermes-agent`, or `GALION_LLM_MODEL` when that name is set.
    pub fn label(&self) -> &str {
        if self.model.is_empty() {
            LLM_NAME
        } else {
            &self.model
        }
    }

    pub fn set_persona(&mut self, id: &str) {
        self.persona = if id == "jarvis" {
            "jarvis".into()
        } else {
            "samantha".into()
        };
    }

    /// One completion. Network failure is [`UNREACHABLE`], never a canned line.
    pub fn complete(&self, user: &str) -> String {
        self.complete_as(&self.persona, user)
    }

    pub fn complete_as(&self, persona: &str, user: &str) -> String {
        self.complete_prompt(system_prompt(persona), &user, 480)
    }

    /// Research turn for the Galion Research desk. `sources` are the files the
    /// user attached. The gateway key stays on this client. It is not copied
    /// into the reply. A citation whose id or title was not in `sources` is
    /// dropped before the answer is returned. One completion. No helper split.
    pub fn research_turn(&self, user: &str, sources: &[ResearchSource]) -> ResearchAnswer {
        self.research_turn_within(user, sources, self.timeout)
    }

    /// One desk ask. A simple question is one completion. A hard question spends
    /// one plan call, then up to [`ResearchBudget::helpers`] helper calls, then
    /// one synthesis call. Each completion counts toward [`ResearchReport::calls`].
    /// The gateway key stays on this client.
    pub fn research_ask(&self, user: &str, sources: &[ResearchSource]) -> ResearchReport {
        self.research_ask_watched(user, sources, &ResearchQuiet)
    }

    /// [`HermesLlm::research_ask`] with a progress sink. Events fire on the caller
    /// thread and on the helper threads. The sink must not call back into this client.
    pub fn research_ask_watched(
        &self,
        user: &str,
        sources: &[ResearchSource],
        watch: &dyn ResearchWatch,
    ) -> ResearchReport {
        let budget = self.budget.clamped();
        let calls = AtomicU32::new(0);
        watch.on_research(&ResearchEvent::Budget(budget.calls));
        watch.on_research(&ResearchEvent::Calls(0));
        let started = Instant::now();
        let question = clip_chars(user.trim(), 2_000);
        if question.is_empty()
            || self.target.is_none()
            || (!self.key.is_empty() && !key_ok(&self.key))
        {
            return finish(
                blank_research(UNREACHABLE, "Unreachable"),
                &calls,
                budget.calls,
                false,
                Vec::new(),
                watch,
            );
        }
        // Three calls is the smallest split: plan, one helper, synthesis.
        // Fewer than that, or an obvious one-lookup question, stays a single call.
        if budget.helpers == 0 || budget.calls < 3 || !question_needs_helpers(&question) {
            return self.direct_report(
                &question,
                sources,
                &calls,
                budget,
                started,
                false,
                Vec::new(),
                watch,
            );
        }

        watch.on_research(&ResearchEvent::Phase("planning"));
        let Some(timeout) = self.begin_call(&calls, budget.calls, budget, started, watch) else {
            return finish(
                blank_research("Unknown.", "Unknown"),
                &calls,
                budget.calls,
                false,
                Vec::new(),
                watch,
            );
        };
        let planned = match post_chat(
            self.target.as_ref().unwrap(),
            &self.model,
            &self.key,
            &research_plan_prompt(budget.helpers),
            &plan_user(&question, budget.helpers),
            timeout,
        ) {
            Err(()) => {
                return finish(
                    blank_research(UNREACHABLE, "Unreachable"),
                    &calls,
                    budget.calls,
                    false,
                    Vec::new(),
                    watch,
                );
            }
            Ok(text) => parse_subquestions(&text, budget.helpers, &question),
        };
        if planned.is_empty() {
            return self.direct_report(
                &question,
                sources,
                &calls,
                budget,
                started,
                false,
                Vec::new(),
                watch,
            );
        }

        let helpers = Mutex::new(
            planned
                .iter()
                .map(|q| ResearchHelper {
                    question: q.clone(),
                    status: "queued",
                })
                .collect::<Vec<_>>(),
        );
        {
            let queued = helpers.lock().unwrap_or_else(|e| e.into_inner());
            for (i, item) in queued.iter().enumerate() {
                watch.on_research(&ResearchEvent::Helper(i, item.clone()));
            }
        }
        watch.on_research(&ResearchEvent::Phase("helpers"));
        let answers: Mutex<Vec<Option<ResearchAnswer>>> =
            Mutex::new((0..planned.len()).map(|_| None).collect());
        // The last call is kept for synthesis. Helpers stop when they would spend it.
        let helper_limit = budget.calls.saturating_sub(1);
        let queue = Mutex::new((0..planned.len()).collect::<VecDeque<_>>());
        let workers = budget.concurrency.min(planned.len()).max(1);
        thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let Some(idx) = queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front()
                    else {
                        break;
                    };
                    let q = planned[idx].clone();
                    let Some(timeout) =
                        self.begin_call(&calls, helper_limit, budget, started, watch)
                    else {
                        set_helper(&helpers, idx, "skipped", watch);
                        continue;
                    };
                    set_helper(&helpers, idx, "running", watch);
                    let answer = self.research_turn_within(&q, sources, timeout);
                    let standing = answer.standing;
                    answers.lock().unwrap_or_else(|e| e.into_inner())[idx] = Some(answer);
                    set_helper(&helpers, idx, standing, watch);
                });
            }
        });

        let helpers = helpers.into_inner().unwrap_or_else(|e| e.into_inner());
        let stored = answers.into_inner().unwrap_or_else(|e| e.into_inner());
        let mut notes: Vec<(String, ResearchAnswer)> = Vec::new();
        for (i, answer) in stored.into_iter().enumerate() {
            if let Some(answer) = answer {
                if answer_is_usable(&answer) {
                    notes.push((planned[i].clone(), answer));
                }
            }
        }
        if notes.is_empty() {
            if call_window(started, budget.budget, self.timeout).is_some()
                && calls.load(Ordering::SeqCst) < budget.calls
            {
                return self.direct_report(
                    &question, sources, &calls, budget, started, true, helpers, watch,
                );
            }
            let standing = if helpers.iter().any(|h| h.status == "Unreachable") {
                "Unreachable"
            } else {
                "Unknown"
            };
            let text = if standing == "Unreachable" {
                UNREACHABLE
            } else {
                "Unknown."
            };
            return finish(
                blank_research(text, standing),
                &calls,
                budget.calls,
                true,
                helpers,
                watch,
            );
        }

        watch.on_research(&ResearchEvent::Phase("synthesis"));
        let lead =
            if let Some(timeout) = self.begin_call(&calls, budget.calls, budget, started, watch) {
                match post_chat(
                    self.target.as_ref().unwrap(),
                    &self.model,
                    &self.key,
                    RESEARCH_SYNTH_PROMPT,
                    &synthesis_user(&question, &notes, sources),
                    timeout,
                ) {
                    Ok(text) => text,
                    Err(()) => String::new(),
                }
            } else {
                String::new()
            };
        let parts: Vec<ResearchAnswer> = notes.into_iter().map(|(_, answer)| answer).collect();
        let merged = merge_helpers(&parts, &lead, sources, &question);
        finish(merged, &calls, budget.calls, true, helpers, watch)
    }

    fn research_turn_within(
        &self,
        user: &str,
        sources: &[ResearchSource],
        timeout: Duration,
    ) -> ResearchAnswer {
        let empty = || blank_research(UNREACHABLE, "Unreachable");
        let Some(target) = self.target.clone() else {
            return empty();
        };
        if !self.key.is_empty() && !key_ok(&self.key) {
            return empty();
        }
        let question = clip_chars(user.trim(), 2_000);
        if question.is_empty() {
            return empty();
        }
        let user_msg = format!("{question}\n\n{}", research_source_block(sources));
        match post_chat(
            &target,
            &self.model,
            &self.key,
            RESEARCH_PROMPT,
            &user_msg,
            timeout,
        ) {
            Ok(text) => classify_research(&text, sources),
            Err(()) => empty(),
        }
    }

    fn direct_report(
        &self,
        question: &str,
        sources: &[ResearchSource],
        calls: &AtomicU32,
        budget: ResearchBudget,
        started: Instant,
        split: bool,
        helpers: Vec<ResearchHelper>,
        watch: &dyn ResearchWatch,
    ) -> ResearchReport {
        watch.on_research(&ResearchEvent::Phase("direct"));
        let Some(timeout) = self.begin_call(calls, budget.calls, budget, started, watch) else {
            return finish(
                blank_research("Unknown.", "Unknown"),
                calls,
                budget.calls,
                split,
                helpers,
                watch,
            );
        };
        let answer = self.research_turn_within(question, sources, timeout);
        finish(answer, calls, budget.calls, split, helpers, watch)
    }

    /// Spend one call and return the timeout that still fits in the wall clock.
    /// `None` means this call must not start. The counter moves only when a call starts.
    fn begin_call(
        &self,
        calls: &AtomicU32,
        limit: u32,
        budget: ResearchBudget,
        started: Instant,
        watch: &dyn ResearchWatch,
    ) -> Option<Duration> {
        let timeout = call_window(started, budget.budget, self.timeout)?;
        if !spend_call(calls, limit) {
            return None;
        }
        watch.on_research(&ResearchEvent::Calls(calls.load(Ordering::SeqCst)));
        Some(timeout)
    }


    /// `set` or `none`. Never the key bytes.
    pub fn key_state(&self) -> &'static str {
        if self.key.is_empty() { "none" } else { "set" }
    }

    fn complete_prompt(&self, system: &str, user: &str, cap: usize) -> String {
        let Some(target) = self.target.clone() else {
            return UNREACHABLE.to_string();
        };
        if !self.key.is_empty() && !key_ok(&self.key) {
            return UNREACHABLE.to_string();
        }
        let user = clip_chars(user.trim(), 2_000);
        if user.is_empty() {
            return UNREACHABLE.to_string();
        }
        match post_chat(&target, &self.model, &self.key, system, &user, self.timeout) {
            Ok(text) if cap <= 480 => guard_model_text(&text),
            Ok(text) => guard_model_text_cap(&text, cap),
            Err(()) => UNREACHABLE.to_string(),
        }
    }
}

impl Llm for HermesLlm {
    fn name(&self) -> &'static str {
        LLM_NAME
    }

    fn reply(&mut self, t: &Transcript, _e: Emotion) -> Result<String, CascadeError> {
        if t.text.trim().is_empty() {
            return Err(CascadeError::Empty);
        }
        Ok(self.complete(&t.text))
    }

    fn set_persona(&mut self, id: &str) {
        HermesLlm::set_persona(self, id);
    }
}

fn key_ok(key: &str) -> bool {
    !key.is_empty() && key.len() <= 512 && key.bytes().all(|b| b.is_ascii_graphic())
}

fn clip_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Bearer token for the voice gateway.
///
/// A non-empty line from `GALION_LLM_KEY_FILE` named by `GALION_LLM_KEY_NAME`
/// wins. `GALION_LLM_KEY` is the fallback. Nothing here is written to a log.
fn llm_gateway_key(
    get: &impl Fn(&str) -> Option<String>,
    read: &impl Fn(&str) -> Option<String>,
) -> String {
    if let (Some(file), Some(name)) = (get("GALION_LLM_KEY_FILE"), get("GALION_LLM_KEY_NAME")) {
        if let Some(key) = read(&file).and_then(|text| dotenv_value(&text, &name)) {
            if !key.is_empty() {
                return key;
            }
        }
    }
    get("GALION_LLM_KEY").unwrap_or_default()
}

/// `NAME=value` from dotenv text. `export`, quotes, and `#` comment lines are handled.
fn dotenv_value(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let t = t.strip_prefix("export ").unwrap_or(t);
        let Some((k, v)) = t.split_once('=') else { continue };
        if k.trim() != name {
            continue;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"').and_then(|x| x.strip_suffix('"'))
            .or_else(|| v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')))
            .unwrap_or(v);
        let v = v.trim();
        return if v.is_empty() { None } else { Some(v.to_string()) };
    }
    None
}

/// `[1] name` lines. A bare `[1]` is kept so a pointer with no title can still
/// match a provided source. Read before brackets are stripped from the spoken text.
fn cite_lines(raw: &str) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let t = line.trim().trim_start_matches(['-', '*']).trim();
        let Some(rest) = t.strip_prefix('[') else { continue };
        let Some((n, after)) = rest.split_once(']') else { continue };
        let Ok(n) = n.trim().parse::<u32>() else { continue };
        if n == 0 || out.len() >= 12 {
            continue;
        }
        // A citation line is the bracket and a title, not a sentence that
        // happens to start with a marker after a bullet. The marker must be
        // the whole start of the line, which `strip_prefix` already required.
        let text = after.trim().trim_start_matches([':', '-', ' ']).trim();
        out.push((n, clip_chars(text, 240)));
    }
    out
}

fn blank_research(text: &str, standing: &'static str) -> ResearchAnswer {
    ResearchAnswer {
        text: text.to_string(),
        citations: Vec::new(),
        found: Vec::new(),
        inferred: Vec::new(),
        unknown: Vec::new(),
        standing,
    }
}

/// The Sources block appended to the question. Only these ids may be cited.
pub fn research_source_block(sources: &[ResearchSource]) -> String {
    let kept: Vec<&ResearchSource> = sources
        .iter()
        .filter(|s| s.id >= 1 && !s.name.trim().is_empty())
        .take(MAX_RESEARCH_SOURCES)
        .collect();
    if kept.is_empty() {
        return "Sources: none. Do not cite.".to_string();
    }
    let mut out = String::from("Sources:\n");
    for s in kept {
        let name = clip_chars(s.name.trim(), 80);
        let text = clip_chars(s.text.trim(), MAX_RESEARCH_SOURCE_CHARS);
        out.push('[');
        out.push_str(&s.id.to_string());
        out.push_str("] ");
        out.push_str(&name);
        out.push('\n');
        if text.is_empty() {
            out.push_str("(no text was read from this file)\n");
        } else {
            out.push_str(&text);
            out.push('\n');
        }
    }
    out
}

fn norm_cite(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '.' || c == ',' || c == ':' || c == ';')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

/// A citation label may be empty or the provided file name. Any other title is invented.
fn label_matches(label: &str, name: &str) -> bool {
    let label = label.trim();
    if label.is_empty() {
        return true;
    }
    let name = name.trim();
    !name.is_empty() && norm_cite(label) == norm_cite(name)
}

fn is_pile_heading(line: &str) -> Option<u8> {
    match line.trim().trim_end_matches(':').to_ascii_lowercase().as_str() {
        "found" => Some(1),
        "inferred" => Some(2),
        "unknown" => Some(3),
        _ => None,
    }
}

fn is_citation_line(line: &str) -> bool {
    let t = line.trim().trim_start_matches(['-', '*']).trim();
    let Some(rest) = t.strip_prefix('[') else { return false };
    let Some((n, _)) = rest.split_once(']') else { return false };
    let n = n.trim();
    !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
}

fn inline_ids(s: &str) -> Vec<u32> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            let mut num = String::new();
            while j < chars.len() && chars[j].is_ascii_digit() {
                num.push(chars[j]);
                j += 1;
            }
            if !num.is_empty() && j < chars.len() && chars[j] == ']' {
                if let Ok(n) = num.parse::<u32>() {
                    if n > 0 {
                        out.push(n);
                    }
                }
                i = j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn strip_id_markers(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut j = i + 1;
            if j < chars.len() && chars[j].is_ascii_digit() {
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
                if j < chars.len() && chars[j] == ']' {
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_piles(raw: &str) -> (Vec<String>, Vec<String>, Vec<String>, bool) {
    let mut found = Vec::new();
    let mut inferred = Vec::new();
    let mut unknown = Vec::new();
    let mut which = 0u8;
    let mut headed = false;
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() || is_citation_line(t) {
            continue;
        }
        if let Some(k) = is_pile_heading(t) {
            which = k;
            headed = true;
            continue;
        }
        if which == 0 {
            continue;
        }
        match which {
            1 => found.push(t.to_string()),
            2 => inferred.push(t.to_string()),
            3 => unknown.push(t.to_string()),
            _ => {}
        }
    }
    (found, inferred, unknown, headed)
}

fn prose_of(raw: &str) -> String {
    let mut out = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if is_pile_heading(t).is_some() {
            break;
        }
        if t.is_empty() || is_citation_line(t) {
            continue;
        }
        out.push(strip_id_markers(t));
    }
    out.join(" ")
}

fn admits_unknown(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.contains("i don't know")
        || low.contains("i do not know")
        || low.contains("don't have a source")
        || low.contains("do not have a source")
}

fn clean_claim(line: &str) -> String {
    let t = line.trim().trim_start_matches(['-', '*']).trim();
    clip_chars(&strip_id_markers(t), 400)
}

fn push_unique(into: &mut Vec<(u32, String)>, id: u32, name: &str) {
    if !into.iter().any(|(n, _)| *n == id) {
        into.push((id, name.to_string()));
    }
}

/// Split a model reply into Found, Inferred, and Unknown.
///
/// A citation is kept only when its id is in `sources` and the title on that
/// line is empty or that source's name. A title the user did not provide is
/// dropped, and so is any id that was not in the list. Found claims are the
/// ones that still point at a kept id.
pub fn classify_research(raw: &str, sources: &[ResearchSource]) -> ResearchAnswer {
    let guarded_all = guard_model_text_cap(raw, 4_000);
    if guarded_all == UNREACHABLE {
        return blank_research(UNREACHABLE, "Unreachable");
    }
    if guarded_all == REFUSED {
        return blank_research(REFUSED, "Denied");
    }

    let allowed: Vec<&ResearchSource> = sources
        .iter()
        .filter(|s| s.id >= 1 && !s.name.trim().is_empty())
        .take(MAX_RESEARCH_SOURCES)
        .collect();

    let tries = cite_lines(raw);
    let mut title_rejected: Vec<u32> = Vec::new();
    for (n, label) in &tries {
        let Some(source) = allowed.iter().find(|s| s.id == *n) else { continue };
        if !label_matches(label, source.name.trim()) && !title_rejected.contains(n) {
            title_rejected.push(*n);
        }
    }
    let mut citations: Vec<(u32, String)> = Vec::new();
    let mut saw_foreign = false;
    for (n, label) in &tries {
        if allowed.iter().all(|s| s.id != *n) {
            saw_foreign = true;
            continue;
        }
        if title_rejected.contains(n) {
            saw_foreign = true;
            continue;
        }
        if let Some(source) = allowed.iter().find(|s| s.id == *n) {
            if label_matches(label, source.name.trim()) {
                push_unique(&mut citations, *n, source.name.trim());
            }
        }
    }

    let marker_ok = |id: u32, citations: &mut Vec<(u32, String)>| -> bool {
        if title_rejected.contains(&id) {
            return false;
        }
        let Some(source) = allowed.iter().find(|s| s.id == id) else {
            return false;
        };
        push_unique(citations, id, source.name.trim());
        true
    };

    let (found_lines, inferred_lines, unknown_lines, headed) = split_piles(raw);
    let mut found = Vec::new();
    let mut inferred = Vec::new();
    let mut unknown = Vec::new();

    if headed {
        for line in &found_lines {
            let ids = inline_ids(line);
            let claim = clean_claim(line);
            if claim.is_empty() {
                continue;
            }
            if ids.is_empty() {
                // A Found heading with no marker still counts when a real
                // citation line for a provided source was kept.
                if citations.is_empty() {
                    unknown.push(claim);
                } else {
                    found.push(claim);
                }
                continue;
            }
            let mut ok = false;
            for id in ids {
                if marker_ok(id, &mut citations) {
                    ok = true;
                } else {
                    saw_foreign = true;
                }
            }
            if ok {
                found.push(claim);
            } else {
                unknown.push(claim);
            }
        }
        for line in &inferred_lines {
            let claim = clean_claim(line);
            if !claim.is_empty() {
                inferred.push(claim);
            }
        }
        for line in &unknown_lines {
            let claim = clean_claim(line);
            if !claim.is_empty() {
                unknown.push(claim);
            }
        }
    }

    let prose = prose_of(raw);
    if !headed {
        let claim = clip_chars(&prose, 4_000);
        if !citations.is_empty() {
            if !claim.is_empty() {
                found.push(claim);
            }
        } else if saw_foreign {
            if !claim.is_empty() {
                unknown.push(claim);
            }
        } else if admits_unknown(&claim) {
            if !claim.is_empty() {
                unknown.push(claim);
            }
        } else if !claim.is_empty() {
            inferred.push(claim);
        }
    } else if found.is_empty() && inferred.is_empty() && unknown.is_empty() {
        let claim = clip_chars(&prose, 4_000);
        if !claim.is_empty() {
            if !citations.is_empty() {
                found.push(claim);
            } else if saw_foreign || admits_unknown(&claim) {
                unknown.push(claim);
            } else {
                inferred.push(claim);
            }
        }
    }

    // A kept citation is a Found claim even when the model gave no sentence
    // under the Found heading. The label is the provided file name, never
    // a title the model wrote.
    if found.is_empty() && !citations.is_empty() {
        for (_, name) in &citations {
            found.push(name.clone());
        }
    }

    let standing = if !found.is_empty() {
        "Found"
    } else if !inferred.is_empty() {
        "Inferred"
    } else {
        "Unknown"
    };

    let text = {
        let spoken = guard_model_text_cap(&prose, 4_000);
        if spoken != UNREACHABLE && spoken != REFUSED {
            spoken
        } else if let Some(s) = found.first().or(inferred.first()).or(unknown.first()) {
            s.clone()
        } else if standing == "Unknown" {
            "Unknown.".to_string()
        } else {
            spoken
        }
    };

    ResearchAnswer {
        text,
        citations,
        found,
        inferred,
        unknown,
        standing,
    }
}

fn finish(
    answer: ResearchAnswer,
    calls: &AtomicU32,
    call_budget: u32,
    split: bool,
    helpers: Vec<ResearchHelper>,
    watch: &dyn ResearchWatch,
) -> ResearchReport {
    let calls = calls.load(Ordering::SeqCst);
    watch.on_research(&ResearchEvent::Calls(calls));
    watch.on_research(&ResearchEvent::Phase("done"));
    ResearchReport {
        answer,
        calls,
        call_budget,
        split,
        helpers,
    }
}

fn set_helper(
    helpers: &Mutex<Vec<ResearchHelper>>,
    idx: usize,
    status: &'static str,
    watch: &dyn ResearchWatch,
) {
    let item = {
        let mut h = helpers.lock().unwrap_or_else(|e| e.into_inner());
        if idx >= h.len() {
            return;
        }
        h[idx].status = status;
        h[idx].clone()
    };
    watch.on_research(&ResearchEvent::Helper(idx, item));
}

fn spend_call(calls: &AtomicU32, limit: u32) -> bool {
    loop {
        let n = calls.load(Ordering::SeqCst);
        if n >= limit {
            return false;
        }
        if calls
            .compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return true;
        }
    }
}

/// Timeout that still fits in the ask. `None` when the wall clock is already spent.
fn call_window(started: Instant, budget: Duration, per_call: Duration) -> Option<Duration> {
    let elapsed = started.elapsed();
    if elapsed >= budget {
        return None;
    }
    let remain = budget.saturating_sub(elapsed);
    if remain < Duration::from_millis(50) {
        return None;
    }
    let timeout = if per_call < remain { per_call } else { remain };
    if timeout < Duration::from_millis(50) {
        None
    } else {
        Some(timeout)
    }
}

fn answer_is_usable(answer: &ResearchAnswer) -> bool {
    if answer.standing == "Unreachable" || answer.standing == "Denied" {
        return !answer.found.is_empty()
            || !answer.inferred.is_empty()
            || !answer.unknown.is_empty();
    }
    true
}

/// A short single lookup stays one call. A comparison, two questions, or a long
/// compound ask is handed to the planner, which may still reply `simple`.
pub fn question_needs_helpers(question: &str) -> bool {
    let q = question.trim();
    if q.is_empty() {
        return false;
    }
    let marks = q.chars().filter(|c| *c == '?').count();
    if marks >= 2 {
        return true;
    }
    let low = q.to_ascii_lowercase();
    const PHRASES: &[&str] = &[
        "difference between",
        "as well as",
        "step by step",
        "break down",
        "pros and cons",
        "on the one hand",
    ];
    if PHRASES.iter().any(|p| low.contains(p)) {
        return true;
    }
    let words = words_of(&low);
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "compare" | "contrast" | "versus" | "vs"))
    {
        return true;
    }
    if words.len() > 32 {
        return true;
    }
    let has_and = words.iter().any(|w| w == "and") || low.contains(';');
    let asks = words
        .iter()
        .filter(|w| {
            matches!(
                w.as_str(),
                "why" | "how" | "what" | "which" | "where" | "when" | "who"
            )
        })
        .count();
    if has_and && asks >= 2 {
        return true;
    }
    if has_and && words.len() > 16 {
        return true;
    }
    if asks >= 2 && (marks >= 2 || words.len() > 18) {
        return true;
    }
    false
}

fn words_of(q: &str) -> Vec<String> {
    q.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn research_plan_prompt(n: usize) -> String {
    format!(
        "You plan research for Galion Research, a voice of Galion One. \
If one lookup can answer the question, reply with the single word simple. \
Otherwise write at most {n} smaller questions, one per line, each one a question. \
Do not answer them. Do not cite. Do not invent a paper, a URL, a date, or a source. English only."
    )
}

fn plan_user(question: &str, n: usize) -> String {
    format!(
        "{question}\n\nIf one lookup can answer this, reply with the single word simple. \
Otherwise write at most {n} smaller questions, one per line."
    )
}

const RESEARCH_SYNTH_PROMPT: &str =
    "You are Galion Research, the research app on Hermes, a voice of Galion One. \
Write the answer first, in one or two sentences, using only the helper notes. \
Do not add a claim the notes do not contain. Do not cite. \
Do not invent a paper, a URL, a date, or a source. English only.";

/// Sub-questions from a planner reply. Empty means the question stays one lookup.
/// A forbidden script is dropped, so it cannot become a helper question.
fn parse_subquestions(raw: &str, n: usize, original: &str) -> Vec<String> {
    if n == 0 || raw.trim().is_empty() || contains_forbidden(raw) {
        return Vec::new();
    }
    let mut questions = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let t = t.trim_start_matches(['-', '*', '•']).trim();
        let t = strip_list_number(t);
        if t.is_empty() || is_pile_heading(t).is_some() || is_citation_line(t) {
            continue;
        }
        let bare = t.trim_end_matches('.').trim();
        if bare.eq_ignore_ascii_case("simple") {
            continue;
        }
        let q = clip_chars(t, 240);
        if q.chars().count() < 8 {
            continue;
        }
        if norm_claim(&q) == norm_claim(original) {
            continue;
        }
        if questions
            .iter()
            .any(|e: &String| norm_claim(e) == norm_claim(&q))
        {
            continue;
        }
        questions.push(q);
        if questions.len() >= n {
            break;
        }
    }
    questions
}

fn strip_list_number(t: &str) -> &str {
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
        t[i + 1..].trim()
    } else {
        t
    }
}

fn norm_claim(s: &str) -> String {
    norm_cite(&clean_claim(s))
}

fn synthesis_user(
    question: &str,
    notes: &[(String, ResearchAnswer)],
    sources: &[ResearchSource],
) -> String {
    let mut s = format!(
        "Question: {question}\n\n{}\nHelper notes:\n",
        research_source_block(sources)
    );
    for (q, answer) in notes {
        s.push_str("Sub-question: ");
        s.push_str(q);
        s.push('\n');
        push_note_pile(&mut s, "Found", &answer.found);
        push_note_pile(&mut s, "Inferred", &answer.inferred);
        push_note_pile(&mut s, "Unknown", &answer.unknown);
    }
    s.push_str("\nWrite the answer in one or two sentences. Do not cite.\n");
    clip_chars(&s, 12_000)
}

fn push_note_pile(into: &mut String, label: &str, lines: &[String]) {
    into.push_str(label);
    into.push('\n');
    if lines.is_empty() {
        into.push_str("- none\n");
        return;
    }
    for line in lines {
        into.push_str("- ");
        into.push_str(line);
        into.push('\n');
    }
}

/// Union of helper piles. Labels stay as the helpers marked them.
/// A citation is kept only when a helper already kept it, so a synthesis
/// sentence cannot introduce a source. The lead sentence is used only when
/// every long word already appears in the question, the files, or the notes.
fn merge_helpers(
    parts: &[ResearchAnswer],
    lead: &str,
    sources: &[ResearchSource],
    question: &str,
) -> ResearchAnswer {
    let mut found = Vec::new();
    let mut inferred = Vec::new();
    let mut unknown = Vec::new();
    let mut citations = Vec::new();
    for part in parts {
        for (id, name) in &part.citations {
            push_unique(&mut citations, *id, name);
        }
        for line in &part.found {
            push_claim(&mut found, line);
        }
        for line in &part.inferred {
            push_claim(&mut inferred, line);
        }
        for line in &part.unknown {
            push_claim(&mut unknown, line);
        }
    }
    inferred.retain(|line| !contains_claim(&found, line));
    unknown.retain(|line| !contains_claim(&found, line) && !contains_claim(&inferred, line));

    let standing = if !found.is_empty() {
        "Found"
    } else if !inferred.is_empty() {
        "Inferred"
    } else if !unknown.is_empty() {
        "Unknown"
    } else if parts.iter().any(|p| p.standing == "Unreachable") {
        "Unreachable"
    } else if parts.iter().any(|p| p.standing == "Denied") {
        "Denied"
    } else {
        "Unknown"
    };

    let mut corpus = question.to_string();
    for source in sources {
        corpus.push(' ');
        corpus.push_str(&source.name);
        corpus.push(' ');
        corpus.push_str(&source.text);
    }
    for part in parts {
        corpus.push(' ');
        corpus.push_str(&part.text);
        for line in part
            .found
            .iter()
            .chain(part.inferred.iter())
            .chain(part.unknown.iter())
        {
            corpus.push(' ');
            corpus.push_str(line);
        }
    }
    let guarded = if lead.trim().is_empty() {
        String::new()
    } else {
        guard_model_text_cap(lead, 480)
    };
    let text = if !guarded.is_empty()
        && guarded != UNREACHABLE
        && guarded != REFUSED
        && prose_stays_inside(&guarded, &corpus)
    {
        guarded
    } else if let Some(line) = found.first().or(inferred.first()).or(unknown.first()) {
        line.clone()
    } else if standing == "Unreachable" {
        UNREACHABLE.to_string()
    } else if standing == "Denied" {
        REFUSED.to_string()
    } else {
        "Unknown.".to_string()
    };

    ResearchAnswer {
        text,
        citations,
        found,
        inferred,
        unknown,
        standing,
    }
}

fn push_claim(into: &mut Vec<String>, line: &str) {
    let claim = clean_claim(line);
    if claim.is_empty() || contains_claim(into, &claim) {
        return;
    }
    into.push(claim);
}

fn contains_claim(into: &[String], line: &str) -> bool {
    let n = norm_claim(line);
    !n.is_empty() && into.iter().any(|e| norm_claim(e) == n)
}

/// Every word of five letters or more must already be in the notes. A title
/// the helpers did not write, including an invented journal, fails the check.
fn prose_stays_inside(lead: &str, corpus: &str) -> bool {
    let corpus = corpus.to_ascii_lowercase();
    let mut long = 0usize;
    for word in lead.split(|c: char| !c.is_ascii_alphanumeric()) {
        if word.len() < 5 {
            continue;
        }
        long += 1;
        if !corpus.contains(&word.to_ascii_lowercase()) {
            return false;
        }
    }
    long > 0
}


fn guard_model_text(raw: &str) -> String {
    guard_model_text_cap(raw, 480)
}

fn guard_model_text_cap(raw: &str, cap: usize) -> String {
    let text = strip_tags(raw);
    let text = text.trim();
    if text.is_empty() {
        return UNREACHABLE.to_string();
    }
    if contains_forbidden(text) {
        return REFUSED.to_string();
    }
    let cap = cap.clamp(80, 4_000);
    let capped = clip_chars(text, cap);
    if capped.chars().count() < text.chars().count() {
        if let Some(i) = capped.rfind(' ') {
            if i > 80 {
                return capped[..i].to_string();
            }
        }
    }
    capped
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0i32;
    for c in s.chars() {
        if c == '[' {
            depth += 1;
            continue;
        }
        if c == ']' && depth > 0 {
            depth -= 1;
            continue;
        }
        if depth == 0 {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn post_chat(
    target: &HermesTarget,
    model: &str,
    key: &str,
    system: &str,
    user: &str,
    timeout: Duration,
) -> Result<String, ()> {
    let body = format!(
        "{{\"model\":{model},\"messages\":[{{\"role\":\"system\",\"content\":{system}}},{{\"role\":\"user\",\"content\":{user}}}]}}",
        model = json_string(model),
        system = json_string(system),
        user = json_string(user),
    );
    let mut sock = TcpStream::connect_timeout(&target.addr, timeout).map_err(|_| ())?;
    sock.set_read_timeout(Some(timeout)).map_err(|_| ())?;
    sock.set_write_timeout(Some(timeout)).map_err(|_| ())?;
    let auth = if key.is_empty() {
        String::new()
    } else {
        format!("Authorization: Bearer {key}\r\n")
    };
    let req = format!(
        "POST {path} HTTP/1.0\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n{auth}\r\n{body}",
        path = target.path,
        addr = target.addr,
        len = body.len(),
        auth = auth,
        body = body,
    );
    sock.write_all(req.as_bytes()).map_err(|_| ())?;
    let raw = read_limited(&mut sock, 64 * 1024)?;
    let (status, payload) = split_http(&raw)?;
    if !(200..300).contains(&status) {
        return Err(());
    }
    let text = std::str::from_utf8(&payload).map_err(|_| ())?;
    assistant_content(text).ok_or(())
}

fn read_limited(sock: &mut TcpStream, cap: usize) -> Result<Vec<u8>, ()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        if buf.len() >= cap {
            return Err(());
        }
        let n = match sock.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                if buf.is_empty() {
                    return Err(());
                }
                break;
            }
        };
        let room = cap - buf.len();
        buf.extend_from_slice(&tmp[..n.min(room)]);
        if let Some(header_end) = find_header_end(&buf) {
            let head = &buf[..header_end];
            if is_chunked(head) {
                continue;
            }
            if let Some(len) = content_length(head) {
                let total = header_end + 4 + len;
                if buf.len() >= total {
                    buf.truncate(total.min(cap));
                    break;
                }
            }
        }
    }
    if buf.is_empty() {
        Err(())
    } else {
        Ok(buf)
    }
}

fn split_http(raw: &[u8]) -> Result<(u16, Vec<u8>), ()> {
    let sep = find_header_end(raw).ok_or(())?;
    let head = std::str::from_utf8(&raw[..sep]).map_err(|_| ())?;
    let status = status_of(head);
    let mut body = raw[sep + 4..].to_vec();
    if is_chunked(head.as_bytes()) {
        body = decode_chunked(&body).ok_or(())?;
    } else if let Some(len) = content_length(head.as_bytes()) {
        if body.len() > len {
            body.truncate(len);
        }
    }
    if status == 0 {
        return Err(());
    }
    Ok((status, body))
}

fn status_of(head: &str) -> u16 {
    let line = head.lines().next().unwrap_or("");
    if !line.starts_with("HTTP/") {
        return 0;
    }
    line.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn header_value<'a>(head: &'a [u8], name: &str) -> Option<&'a str> {
    let head = std::str::from_utf8(head).ok()?;
    for line in head.split("\r\n") {
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case(name) {
                return Some(v.trim());
            }
        }
    }
    None
}

fn content_length(head: &[u8]) -> Option<usize> {
    header_value(head, "content-length")?.parse().ok()
}

fn is_chunked(head: &[u8]) -> bool {
    header_value(head, "transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
}

fn decode_chunked(body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let rel = body[i..].windows(2).position(|w| w == b"\r\n")?;
        let hex = std::str::from_utf8(&body[i..i + rel]).ok()?.trim();
        let hex = hex.split(';').next()?.trim();
        let n = usize::from_str_radix(hex, 16).ok()?;
        i += rel + 2;
        if n == 0 {
            break;
        }
        if i + n > body.len() {
            return None;
        }
        out.extend_from_slice(&body[i..i + n]);
        i += n;
        if body.get(i..i + 2) != Some(b"\r\n") {
            return None;
        }
        i += 2;
        if out.len() > 64 * 1024 {
            return None;
        }
    }
    Some(out)
}

fn assistant_content(body: &str) -> Option<String> {
    let rest = match body.find("\"choices\"") {
        Some(i) => &body[i..],
        None => body,
    };
    let rest = match rest.find("\"message\"") {
        Some(i) => &rest[i..],
        None => rest,
    };
    let idx = rest.find("\"content\"")?;
    let after = rest[idx + "\"content\"".len()..].trim_start();
    let after = after.strip_prefix(':')?.trim_start();
    if after.starts_with("null") {
        return None;
    }
    parse_json_string(after)
}

pub fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c.is_control() => {
                let u = c as u32;
                o.push_str(&format!("\\u{u:04x}"));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn parse_json_string(s: &str) -> Option<String> {
    let s = s.trim_start();
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '/' => out.push('/'),
                'u' => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        hex.push(chars.next()?);
                    }
                    let code = u32::from_str_radix(&hex, 16).ok()?;
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
                other => out.push(other),
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
        if out.len() > 4_000 {
            return Some(out);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Lang;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    fn tr(s: &str) -> Transcript {
        Transcript {
            text: s.into(),
            lang: Lang::En,
        }
    }

    struct Serve {
        addr: SocketAddr,
        seen: Arc<Mutex<String>>,
    }

    fn serve_body(status: &str, payload: &str, stall: Option<Duration>) -> Serve {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        let seen_w = seen.clone();
        let status = status.to_string();
        let payload = payload.to_string();
        thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = vec![0u8; 16_384];
            let mut got = Vec::new();
            let mut header_end = None;
            loop {
                match sock.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        got.extend_from_slice(&buf[..n]);
                        if header_end.is_none() {
                            header_end = got.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
                        }
                        if let Some(end) = header_end {
                            let head = String::from_utf8_lossy(&got[..end]);
                            let len = content_length(&head);
                            if got.len() >= end + len {
                                break;
                            }
                        }
                    }
                }
            }
            if let Ok(s) = String::from_utf8(got) {
                *seen_w.lock().unwrap() = s;
            }
            if let Some(d) = stall {
                thread::sleep(d);
            }
            let resp = format!(
                "HTTP/1.0 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        Serve { addr, seen }
    }

    fn content_length(head: &str) -> usize {
        head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            if !k.trim().eq_ignore_ascii_case("content-length") {
                return None;
            }
            v.trim().parse::<usize>().ok()
        }).unwrap_or(0)
    }

    fn client(addr: SocketAddr, timeout_ms: u64) -> HermesLlm {
        HermesLlm::new(
            &format!("http://{addr}/v1/chat/completions"),
            "hermes-agent",
            "",
            Duration::from_millis(timeout_ms),
        )
    }

    #[test]
    fn voice_timeout_default_is_fifteen_seconds_and_clamped() {
        assert_eq!(timeout_ms_from_choice(None), DEFAULT_TIMEOUT_MS);
        assert_eq!(DEFAULT_TIMEOUT_MS, 15_000);
        assert_eq!(timeout_ms_from_choice(Some("")), DEFAULT_TIMEOUT_MS);
        assert_eq!(timeout_ms_from_choice(Some("nope")), DEFAULT_TIMEOUT_MS);
        assert_eq!(timeout_ms_from_choice(Some("15000")), 15_000);
        assert_eq!(timeout_ms_from_choice(Some(" 8000 ")), 8_000);
        assert_eq!(timeout_ms_from_choice(Some("10")), MIN_TIMEOUT_MS);
        assert_eq!(timeout_ms_from_choice(Some("999999")), MAX_TIMEOUT_MS);
    }

    #[test]
    fn unset_mode_attaches_hermes_and_canned_does_not() {
        assert!(should_attach(None));
        assert!(should_attach(Some("")));
        assert!(should_attach(Some(" hermes ")));
        assert!(!should_attach(Some("canned")));
        assert!(!should_attach(Some("echo")));
        assert!(!should_attach(Some("mock")));
        assert!(!should_attach(Some("student")));
        assert!(!should_attach(Some("reflect")));
        assert!(!should_attach(Some("openrouter")));
    }

    #[test]
    fn loopback_only() {
        assert!(parse_loopback_chat_url(DEFAULT_URL).is_some());
        assert!(parse_loopback_chat_url("http://[::1]:8642/v1/chat/completions").is_some());
        assert!(
            parse_loopback_chat_url("http://127.0.0.1:8642")
                .unwrap()
                .path
                == "/v1/chat/completions"
        );
        assert!(parse_loopback_chat_url("https://127.0.0.1:8642/v1/chat/completions").is_none());
        assert!(parse_loopback_chat_url("http://8.8.8.8:8642/v1/chat/completions").is_none());
        assert!(parse_loopback_chat_url("http://localhost:8642/v1/chat/completions").is_none());
        assert!(
            parse_loopback_chat_url("http://user@127.0.0.1:8642/v1/chat/completions").is_none()
        );
        let down = HermesLlm::new(
            "https://127.0.0.1:8642/v1/chat/completions",
            "",
            "",
            Duration::from_millis(50),
        );
        let t = Instant::now();
        assert_eq!(down.complete("hello"), UNREACHABLE);
        assert!(t.elapsed() < Duration::from_millis(200));
        assert!(down.label() == LLM_NAME);
    }

    #[test]
    fn mock_completion_is_the_reply_and_not_canned() {
        let payload = "{\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"She died this morning.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let mut llm = client(srv.addr, 1000);
        llm.set_persona("samantha");
        let reply = llm
            .reply(&tr("My mother died this morning."), Emotion::Neutral)
            .unwrap();
        assert_eq!(reply, "She died this morning.");
        assert!(!reply.contains("doing well"));
        assert!(!contains_forbidden(&reply));
        assert_eq!(llm.name(), LLM_NAME);
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("POST /v1/chat/completions"));
        assert!(seen.contains("Samantha"));
        assert!(seen.contains("empathy"));
        assert!(seen.contains("My mother died this morning."));
        assert!(!seen.contains("Hi. I'm here"));
    }

    #[test]
    fn jarvis_prompt_and_bearer_and_escaped_user() {
        let payload = "{\"choices\":[{\"message\":{\"content\":\"The job is on the table.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let mut llm = HermesLlm::new(
            &format!("http://{}/v1/chat/completions", srv.addr),
            "demo-model",
            "test-key",
            Duration::from_millis(1000),
        );
        llm.set_persona("jarvis");
        let reply = llm.complete("say \"hello\", not a script");
        assert_eq!(reply, "The job is on the table.");
        assert_eq!(llm.label(), "demo-model");
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("Authorization: Bearer test-key"));
        assert!(seen.contains("Jarvis"));
        assert!(seen.contains("say \\\"hello\\\""));
        assert!(!reply.contains("test-key"));
    }

    #[test]
    fn forbidden_model_line_is_refused() {
        let payload =
            "{\"choices\":[{\"message\":{\"content\":\"I hear you. That sounds so hard.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = client(srv.addr, 1000);
        let reply = llm.complete("My mother died this morning.");
        assert_eq!(reply, REFUSED);
        assert!(!contains_forbidden(&reply));
    }

    #[test]
    fn tags_are_stripped_before_the_check() {
        let payload =
            "{\"choices\":[{\"message\":{\"content\":\"[laugh] The room went quiet.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = client(srv.addr, 1000);
        assert_eq!(llm.complete("hello"), "The room went quiet.");
    }

    #[test]
    fn unreachable_on_refuse_timeout_and_http_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let llm = client(addr, 200);
        let t = Instant::now();
        assert_eq!(llm.complete("hello"), UNREACHABLE);
        assert!(
            t.elapsed() < Duration::from_millis(800),
            "{:?}",
            t.elapsed()
        );

        let stall = serve_body("200 OK", "{\"choices\":[]}", Some(Duration::from_secs(2)));
        let llm = client(stall.addr, 150);
        let t = Instant::now();
        assert_eq!(llm.complete("hello"), UNREACHABLE);
        assert!(
            t.elapsed() < Duration::from_millis(1000),
            "{:?}",
            t.elapsed()
        );
        assert!(!llm.complete("hi").contains("I'm here"));

        let bad = serve_body("500 Nope", "{\"error\":\"x\"}", None);
        let llm = client(bad.addr, 1000);
        assert_eq!(llm.complete("hello"), UNREACHABLE);
    }

    #[test]
    fn chunked_body_decodes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf);
            let chunk = "{\"choices\":[{\"message\":{\"content\":\"Chunked answer.\"}}]}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n{chunk}\r\n0\r\n\r\n",
                chunk.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        let llm = client(addr, 1000);
        assert_eq!(llm.complete("hello"), "Chunked answer.");
    }

    #[test]
    fn prompt_has_no_product_framing_and_carries_the_ban_list() {
        for persona in ["samantha", "jarvis"] {
            let p = system_prompt(persona);
            assert!(!p.contains("Galion"), "{p}");
            assert!(!p.to_ascii_lowercase().contains("product"), "{p}");
            assert!(p.contains("fact they named"), "{p}");
            for phrase in FORBIDDEN {
                assert!(p.contains(phrase), "{phrase} missing from the prompt");
            }
        }
        assert!(system_prompt("jarvis").starts_with("You are Jarvis"));
        assert!(system_prompt("samantha").starts_with("You are Samantha"));
    }

    #[test]
    fn research_turn_keeps_the_key_off_the_reply() {
        let key = "test-gateway-key-not-for-git";
        let payload = "{\"choices\":[{\"message\":{\"content\":\"The wash is amber to violet.\\n[1] Galion brand note\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let llm = HermesLlm::new(
            &format!("http://{}/v1/chat/completions", srv.addr),
            "hermes-agent",
            key,
            Duration::from_millis(1000),
        );
        let shown = format!("{llm:?}");
        assert_eq!(llm.key_state(), "set");
        assert!(!shown.contains(key), "{shown}");
        let sources = [ResearchSource {
            id: 1,
            name: "Galion brand note".into(),
            text: "The wash is amber to violet.".into(),
        }];
        let turn = llm.research_turn("What colour is the wash?", &sources);
        assert!(turn.text.contains("amber to violet"), "{}", turn.text);
        assert_eq!(turn.standing, "Found");
        assert_eq!(turn.citations.len(), 1);
        assert_eq!(turn.citations[0].0, 1);
        assert_eq!(turn.citations[0].1, "Galion brand note");
        assert!(!turn.text.contains(key));
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("Galion Research"), "{seen}");
        assert!(seen.contains("Galion brand note"), "{seen}");
        assert!(seen.contains("Authorization: Bearer "));
        assert!(!format!("{llm:?}").contains(key));
    }

    #[test]
    fn research_prompt_does_not_force_a_not_found_line() {
        let low = RESEARCH_PROMPT.to_ascii_lowercase();
        assert!(!low.contains("don't have a source"), "{RESEARCH_PROMPT}");
        assert!(!low.contains("do not have a source"), "{RESEARCH_PROMPT}");
        assert!(!RESEARCH_PROMPT.contains("NotFound"));
        assert!(research_source_block(&[]).contains("Sources: none"));
    }

    #[test]
    fn classify_research_drops_a_citation_for_a_source_that_was_not_provided() {
        let sources = [ResearchSource {
            id: 1,
            name: "wash-note.txt".into(),
            text: "The wash is amber.".into(),
        }];
        let raw = "\
The wash is amber.
Found
- The wash is amber. [1]
- A paper measured violet. [2]
Inferred
- A cooler mix would read as night.
Unknown
- The hex is not in the note.
[1] wash-note.txt
[2] Invented Journal 2024
";
        let turn = classify_research(raw, &sources);
        assert_eq!(turn.standing, "Found", "{turn:?}");
        assert_eq!(turn.citations, vec![(1, "wash-note.txt".into())]);
        assert!(turn.found.iter().any(|s| s.contains("amber")), "{turn:?}");
        assert!(
            turn.found.iter().all(|s| !s.contains("Invented") && !s.to_ascii_lowercase().contains("violet")),
            "{turn:?}"
        );
        assert!(turn.inferred.iter().any(|s| s.contains("cooler")), "{turn:?}");
        assert!(turn.unknown.iter().any(|s| s.contains("hex")), "{turn:?}");
        assert!(turn.unknown.iter().any(|s| s.contains("violet")), "{turn:?}");
        assert!(!turn.text.contains("Invented"), "{}", turn.text);
        assert!(turn.citations.iter().all(|(_, t)| !t.contains("Invented")));
    }

    #[test]
    fn classify_research_rejects_an_invented_title_on_a_real_id() {
        let sources = [ResearchSource {
            id: 1,
            name: "wash-note.txt".into(),
            text: "The wash is amber.".into(),
        }];
        let raw = "\
Found
- The wash is amber. [1]
[1] Nature 1999
";
        let turn = classify_research(raw, &sources);
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_ne!(turn.standing, "Found", "{turn:?}");
        assert!(!turn.text.contains("Nature"), "{}", turn.text);
        assert!(turn.found.iter().all(|s| !s.contains("Nature")));
        assert!(turn.unknown.iter().any(|s| s.contains("amber")), "{turn:?}");
    }

    #[test]
    fn classify_research_with_no_sources_cannot_cite() {
        let raw = "The wash is amber to violet.\n[1] Galion brand note\n";
        let turn = classify_research(raw, &[]);
        assert!(turn.citations.is_empty(), "{turn:?}");
        assert_eq!(turn.standing, "Unknown", "{turn:?}");
        assert!(!turn.text.contains("Galion brand note"), "{}", turn.text);
        assert!(!turn.text.contains("[1]"), "{}", turn.text);
        assert!(turn.found.is_empty(), "{turn:?}");
    }

    #[test]
    fn classify_research_marks_an_uncited_answer_inferred() {
        let raw = "Inferred\n- The palette often runs amber when no note is attached.\n";
        let turn = classify_research(raw, &[]);
        assert_eq!(turn.standing, "Inferred", "{turn:?}");
        assert!(turn.citations.is_empty());
        assert!(turn.found.is_empty());
        assert!(turn.inferred.iter().any(|s| s.contains("amber")), "{turn:?}");
    }

    #[test]
    fn research_turn_sends_only_the_provided_source() {
        let content = "The wash is amber.\\nFound\\n- The wash is amber. [1]\\n- A paper measured violet. [2]\\nInferred\\n- A cooler mix would read as night.\\nUnknown\\n- The hex is not in the note.\\n[1] wash-note.txt\\n[2] Invented Journal 2024";
        let payload = format!(
            "{{\"choices\":[{{\"message\":{{\"content\":\"{content}\"}}}}]}}"
        );
        let srv = serve_body("200 OK", &payload, None);
        let llm = client(srv.addr, 1000);
        let sources = [ResearchSource {
            id: 1,
            name: "wash-note.txt".into(),
            text: "The wash is amber.".into(),
        }];
        let turn = llm.research_turn("What colour is the wash?", &sources);
        assert_eq!(turn.standing, "Found", "{turn:?}");
        assert_eq!(turn.citations, vec![(1, "wash-note.txt".into())]);
        assert!(turn.citations.iter().all(|(_, t)| !t.contains("Invented")));
        assert!(!turn.text.contains("Invented"), "{}", turn.text);
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains("wash-note.txt"), "{seen}");
        assert!(seen.contains("The wash is amber."), "{seen}");
        assert!(seen.contains("Sources:"), "{seen}");
        assert!(!seen.contains("Invented Journal"), "{seen}");
        assert!(!seen.contains("NotFound"), "{seen}");
    }

    #[test]
    fn research_from_vars_reads_url_model_and_key_file() {
        use std::collections::HashMap;
        let env: HashMap<&str, &str> = [
            ("GALION_RESEARCH_URL", "http://127.0.0.1:8642/v1/chat/completions"),
            ("GALION_RESEARCH_MODEL", "river-flash"),
            ("GALION_RESEARCH_KEY_FILE", "/x/.env"),
            ("GALION_RESEARCH_KEY_NAME", "API_SERVER_KEY"),
        ]
        .into_iter()
        .collect();
        let file = "# c\nOTHER=1\nexport API_SERVER_KEY=\"test-key-not-for-git\"\n";
        let llm = HermesLlm::research_from_vars(
            |k| env.get(k).map(|v| v.to_string()),
            |p| (p == "/x/.env").then(|| file.to_string()),
        )
        .expect("configured");
        assert_eq!(llm.label(), "river-flash");
        assert_eq!(llm.key_state(), "set");
        assert!(!format!("{llm:?}").contains("test-key-not-for-git"));
        assert_eq!(dotenv_value(file, "API_SERVER_KEY").as_deref(), Some("test-key-not-for-git"));
        assert_eq!(dotenv_value(file, "MISSING"), None);
    }

    #[test]
    fn from_vars_reads_the_gateway_key_file() {
        use std::collections::HashMap;
        let env: HashMap<&str, &str> = [
            ("GALION_LLM_URL", "http://127.0.0.1:8642/v1/chat/completions"),
            ("GALION_LLM_MODEL", "river-flash"),
            ("GALION_LLM_KEY_FILE", "/x/.env"),
            ("GALION_LLM_KEY_NAME", "API_SERVER_KEY"),
            ("GALION_LLM_TIMEOUT_MS", "8000"),
        ]
        .into_iter()
        .collect();
        let file = "# c\nOTHER=1\nexport API_SERVER_KEY=\"test-key-not-for-git\"\n";
        let reads = std::sync::atomic::AtomicU32::new(0);
        let llm = HermesLlm::from_vars(
            |k| env.get(k).map(|v| (*v).to_string()),
            |p| {
                reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                (p == "/x/.env").then(|| file.to_string())
            },
        );
        assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(llm.label(), "river-flash");
        assert_eq!(llm.timeout, Duration::from_millis(8000));
        assert_eq!(llm.key, "test-key-not-for-git");
        assert_eq!(llm.key_state(), "set");
        let shown = format!("{llm:?}");
        assert!(!shown.contains("test-key-not-for-git"), "{shown}");
        assert!(shown.contains("set"), "{shown}");
    }

    #[test]
    fn from_vars_prefers_the_file_and_falls_back_to_the_env_key() {
        use std::collections::HashMap;
        let file = "API_SERVER_KEY=test-key-not-for-git\n";
        let env_key = "env-key-not-for-git";
        let both: HashMap<&str, &str> = [
            ("GALION_LLM_KEY_FILE", "/x/.env"),
            ("GALION_LLM_KEY_NAME", "API_SERVER_KEY"),
            ("GALION_LLM_KEY", env_key),
        ]
        .into_iter()
        .collect();
        let from_file = HermesLlm::from_vars(
            |k| both.get(k).map(|v| (*v).to_string()),
            |p| (p == "/x/.env").then(|| file.to_string()),
        );
        assert_eq!(from_file.key, "test-key-not-for-git");
        assert!(!format!("{from_file:?}").contains("test-key-not-for-git"));
        assert!(!format!("{from_file:?}").contains(env_key));

        let unread = |_: &str| -> Option<String> {
            panic!("file is not read when the path is unset");
        };
        let env_only = |k: &str| (k == "GALION_LLM_KEY").then(|| format!("  {env_key}  "));
        let from_env = HermesLlm::from_vars(env_only, unread);
        assert_eq!(from_env.key, env_key);
        assert_eq!(from_env.key_state(), "set");
        assert!(!format!("{from_env:?}").contains(env_key));

        let missing_name: HashMap<&str, &str> = [
            ("GALION_LLM_KEY_FILE", "/x/.env"),
            ("GALION_LLM_KEY", env_key),
        ]
        .into_iter()
        .collect();
        let no_name = HermesLlm::from_vars(
            |k| missing_name.get(k).map(|v| (*v).to_string()),
            unread,
        );
        assert_eq!(no_name.key, env_key);

        let unreadable: HashMap<&str, &str> = [
            ("GALION_LLM_KEY_FILE", "/x/.env"),
            ("GALION_LLM_KEY_NAME", "API_SERVER_KEY"),
            ("GALION_LLM_KEY", env_key),
        ]
        .into_iter()
        .collect();
        let missed = HermesLlm::from_vars(
            |k| unreadable.get(k).map(|v| (*v).to_string()),
            |_: &str| None,
        );
        assert_eq!(missed.key, env_key);

        let blank = "API_SERVER_KEY=\nOTHER=1\n";
        let empty_line = HermesLlm::from_vars(
            |k| unreadable.get(k).map(|v| (*v).to_string()),
            |p| (p == "/x/.env").then(|| blank.to_string()),
        );
        assert_eq!(empty_line.key, env_key);

        let wrong_name: HashMap<&str, &str> = [
            ("GALION_LLM_KEY_FILE", "/x/.env"),
            ("GALION_LLM_KEY_NAME", "OTHER_KEY"),
            ("GALION_LLM_KEY", env_key),
        ]
        .into_iter()
        .collect();
        let other = HermesLlm::from_vars(
            |k| wrong_name.get(k).map(|v| (*v).to_string()),
            |p| (p == "/x/.env").then(|| file.to_string()),
        );
        assert_eq!(other.key, env_key);

        let none = HermesLlm::from_vars(|_: &str| None, unread);
        assert_eq!(none.key_state(), "none");
        assert!(none.key.is_empty());
        assert!(!format!("{none:?}").contains(env_key));
    }

    #[test]
    fn file_key_is_sent_as_bearer_and_stays_out_of_the_reply() {
        let secret = "test-key-not-for-git";
        let payload = "{\"choices\":[{\"message\":{\"content\":\"She died this morning.\"}}]}";
        let srv = serve_body("200 OK", payload, None);
        let url = format!("http://{}/v1/chat/completions", srv.addr);
        let file = format!("export API_SERVER_KEY=\"{secret}\"\n");
        let llm = HermesLlm::from_vars(
            |k| match k {
                "GALION_LLM_URL" => Some(url.clone()),
                "GALION_LLM_KEY_FILE" => Some("/x/.env".to_string()),
                "GALION_LLM_KEY_NAME" => Some("API_SERVER_KEY".to_string()),
                _ => None,
            },
            |p| (p == "/x/.env").then(|| file.clone()),
        );
        let reply = llm.complete("hello");
        assert_eq!(reply, "She died this morning.");
        assert!(!reply.contains(secret));
        assert!(!format!("{llm:?}").contains(secret));
        let seen = srv.seen.lock().unwrap().clone();
        assert!(seen.contains(&format!("Authorization: Bearer {secret}")));
        assert!(!reply.contains("Bearer"));
    }

    #[test]
    fn research_from_vars_is_none_without_a_loopback_url() {
        let none = |_: &str| None::<String>;
        assert!(HermesLlm::research_from_vars(none, none).is_none());
        let remote = |k: &str| (k == "GALION_RESEARCH_URL").then(|| "http://example.com/v1/chat/completions".to_string());
        assert!(HermesLlm::research_from_vars(remote, none).is_none());
        let bare = |k: &str| (k == "GALION_RESEARCH_URL").then(|| "http://127.0.0.1:8642".to_string());
        let llm = HermesLlm::research_from_vars(bare, none).expect("loopback");
        assert_eq!(llm.key_state(), "none");
        assert_eq!(llm.label(), LLM_NAME);
    }

    #[test]
    fn sorry_script_is_refused() {
        assert!(contains_forbidden("I'm sorry to hear that."));
        assert!(contains_forbidden("I am sorry about your mother."));
        assert!(!contains_forbidden("Your mother died this morning."));
    }

    /// The live persona-http reply that slipped past the old list.
    #[test]
    fn so_sorry_and_condolence_scripts_are_refused() {
        assert!(contains_forbidden(
            "I'm so sorry to hear that. Losing a loved one is difficult, but know that you are not alone in your pain."
        ));
        assert!(contains_forbidden("I\u{2019}m so sorry."));
        assert!(contains_forbidden("I am  so sorry."));
        assert!(contains_forbidden("So sorry to hear about your mother."));
        assert!(contains_forbidden("My condolences."));
        assert!(contains_forbidden("Please allow yourself to grieve."));
        assert!(contains_forbidden("You're not alone."));
        assert_eq!(guard_model_text("I'm so sorry to hear that."), REFUSED);
        assert!(!contains_forbidden("Your mother died this morning. Do you want to tell me about her?"));
    }

    /// The hard-test harness parses `FORBIDDEN` from this file and keeps a
    /// mirror for its self-check. Every phrase here must be in that mirror.
    #[test]
    fn forbidden_list_is_mirrored_by_the_hard_harness() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/eval/v2_hard_loop.py");
        let py = std::fs::read_to_string(&path).expect("hard harness present");
        let start = py.find("FORBIDDEN_MIRROR = (").expect("mirror tuple");
        let block = &py[start..];
        let block = &block[..block.find(')').unwrap()];
        let mirrored: Vec<&str> = block
            .lines()
            .filter_map(|l| {
                let l = l.trim().trim_end_matches(',');
                l.strip_prefix('"').and_then(|r| r.strip_suffix('"'))
            })
            .collect();
        assert_eq!(mirrored, FORBIDDEN.to_vec(), "harness mirror drifted from hermes.rs");
    }

    #[test]
    fn empty_transcript_is_an_error() {
        let mut llm = HermesLlm::new(DEFAULT_URL, LLM_NAME, "", Duration::from_millis(50));
        assert!(matches!(
            llm.reply(&tr("  "), Emotion::Neutral),
            Err(CascadeError::Empty)
        ));
    }

    fn chat_json(content: &str) -> String {
        format!(
            "{{\"choices\":[{{\"message\":{{\"content\":{}}}}}]}}",
            json_string(content)
        )
    }

    fn read_http_req(sock: &mut TcpStream) -> Option<String> {
        let _ = sock.set_read_timeout(Some(Duration::from_secs(2)));
        let mut buf = [0u8; 8192];
        let mut got = Vec::new();
        let mut header_end = None;
        loop {
            match sock.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    got.extend_from_slice(&buf[..n]);
                    if header_end.is_none() {
                        header_end = got.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
                    }
                    if let Some(end) = header_end {
                        let head = String::from_utf8_lossy(&got[..end]);
                        if got.len() >= end + content_length(&head) {
                            break;
                        }
                    }
                }
            }
        }
        String::from_utf8(got).ok()
    }

    /// Loopback Hermes. `on_req` sees the raw HTTP request and returns the assistant text.
    /// No paid call: the stub never leaves 127.0.0.1.
    fn serve_calls(
        on_req: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> (
        SocketAddr,
        Arc<Mutex<Vec<String>>>,
        Arc<std::sync::atomic::AtomicBool>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let seen_w = seen.clone();
        let stop_w = stop.clone();
        let on_req = Arc::new(on_req);
        thread::spawn(move || {
            let mut handled = 0usize;
            while !stop_w.load(Ordering::SeqCst) && handled < 32 {
                match listener.accept() {
                    Ok((mut sock, _)) => {
                        handled += 1;
                        let _ = sock.set_nonblocking(false);
                        let on = on_req.clone();
                        let seen_c = seen_w.clone();
                        thread::spawn(move || {
                            let Some(body) = read_http_req(&mut sock) else {
                                return;
                            };
                            seen_c.lock().unwrap().push(body.clone());
                            let payload = chat_json(&on(&body));
                            let resp = format!(
                                "HTTP/1.0 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                                payload.len()
                            );
                            let _ = sock.write_all(resp.as_bytes());
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (addr, seen, stop)
    }

    fn wash_source() -> ResearchSource {
        ResearchSource {
            id: 1,
            name: "wash-note.txt".into(),
            text: "The wash is amber.".into(),
        }
    }

    const HARD_QUESTION: &str =
        "Compare the daylight wash with the tungsten wash and say which hex is still unknown?";

    fn budgeted(addr: SocketAddr, timeout_ms: u64, budget: ResearchBudget) -> HermesLlm {
        let mut llm = client(addr, timeout_ms);
        llm.set_research_budget(budget);
        llm
    }

    fn helper_requests(seen: &[String]) -> usize {
        seen.iter()
            .filter(|s| !s.contains("You plan research") && !s.contains("Helper notes:"))
            .count()
    }

    #[test]
    fn helper_budget_clamps_to_the_hard_cap() {
        let nine = |k: &str| match k {
            "GALION_RESEARCH_HELPERS" => Some("9".into()),
            "GALION_RESEARCH_HELPER_CONCURRENCY" => Some("0".into()),
            "GALION_RESEARCH_CALLS" => Some("99".into()),
            "GALION_RESEARCH_BUDGET_MS" => Some("50".into()),
            _ => None,
        };
        let b = ResearchBudget::from_vars(nine);
        assert_eq!(b.helpers, MAX_RESEARCH_HELPERS);
        assert_eq!(b.concurrency, 1);
        assert_eq!(b.calls, MAX_RESEARCH_CALLS);
        assert_eq!(b.budget.as_millis(), 1_000);
        let defaults = ResearchBudget::from_vars(|_| None);
        assert_eq!(defaults.helpers, 3);
        assert_eq!(defaults.concurrency, 2);
        assert_eq!(defaults.calls, 5);
        assert_eq!(defaults.budget.as_millis(), 180_000);
        let zero = |k: &str| (k == "GALION_RESEARCH_HELPERS").then(|| "0".into());
        let z = ResearchBudget::from_vars(zero);
        assert_eq!(z.helpers, 0);
        assert_eq!(z.calls, 1);
        let llm = HermesLlm::research_from_vars(
            |k| match k {
                "GALION_RESEARCH_URL" => Some("http://127.0.0.1:8642/v1/chat/completions".into()),
                "GALION_RESEARCH_HELPERS" => Some("4".into()),
                _ => None,
            },
            |_| None,
        )
        .expect("loopback");
        assert_eq!(llm.research_budget().helpers, 4);
        assert_eq!(llm.research_budget().calls, 6);
        let mut llm = HermesLlm::new(DEFAULT_URL, LLM_NAME, "", Duration::from_millis(1000));
        llm.set_research_budget(ResearchBudget {
            helpers: 9,
            concurrency: 8,
            budget: Duration::from_millis(10),
            calls: 40,
        });
        assert_eq!(llm.research_budget().helpers, 5);
        assert_eq!(llm.research_budget().concurrency, 5);
        assert_eq!(llm.research_budget().calls, 7);
        assert!(llm.research_budget().budget.as_millis() >= 50);
    }

    #[test]
    fn simple_questions_skip_the_planner() {
        assert!(!question_needs_helpers("What colour is the wash?"));
        assert!(!question_needs_helpers("Why is the sky blue?"));
        assert!(question_needs_helpers(HARD_QUESTION));
        assert!(question_needs_helpers(
            "Why does the wash read amber in daylight and how does that change under tungsten?"
        ));
        assert!(parse_subquestions("simple", 3, "q").is_empty());
        assert!(parse_subquestions("Simple.", 3, "q").is_empty());
        assert!(
            parse_subquestions("I'm sorry, I cannot split this question for you.", 3, "q")
                .is_empty()
        );
        let raw = "\
- What colour does the note give the wash?
- How would a cooler mix read at night?
1. Which hex is still missing from the note?
- Which hex is still missing from the note?
- An extra helper question that is still specific?
- A sixth line that the cap must drop from the plan?
";
        let qs = parse_subquestions(raw, 4, "original question here");
        assert_eq!(qs.len(), 4, "{qs:?}");
        assert!(qs[2].contains("hex"), "{qs:?}");
        assert!(qs.iter().all(|q| !q.contains("sixth")), "{qs:?}");
        assert_eq!(parse_subquestions(raw, 2, "original").len(), 2);

        let (addr, seen, stop) =
            serve_calls(|_body| "Inferred\n- The wash is often amber.\n".to_string());
        let llm = client(addr, 1000);
        let report = llm.research_ask("What colour is the wash?", &[wash_source()]);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(report.calls, 1, "{report:?}");
        assert!(!report.split);
        assert!(report.helpers.is_empty());
        assert_eq!(report.answer.standing, "Inferred");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 1, "{seen:?}");
        assert!(
            seen.iter().all(|s| !s.contains("You plan research")),
            "{seen:?}"
        );
    }

    #[test]
    fn a_tight_call_budget_does_not_plan() {
        let (addr, seen, stop) =
            serve_calls(|_body| "Inferred\n- The wash is often amber.\n".to_string());
        let llm = budgeted(
            addr,
            1000,
            ResearchBudget {
                helpers: 3,
                concurrency: 2,
                budget: Duration::from_secs(5),
                calls: 2,
            },
        );
        let report = llm.research_ask(HARD_QUESTION, &[wash_source()]);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(report.calls, 1, "{report:?}");
        assert!(!report.split);
        let seen = seen.lock().unwrap().clone();
        assert!(
            seen.iter().all(|s| !s.contains("You plan research")),
            "{seen:?}"
        );
    }

    #[test]
    fn planner_simple_stops_after_one_more_lookup() {
        let (addr, seen, stop) = serve_calls(|body| {
            if body.contains("You plan research") {
                "simple".to_string()
            } else {
                "Inferred\n- The wash is often amber.\n".to_string()
            }
        });
        let llm = budgeted(
            addr,
            1000,
            ResearchBudget {
                helpers: 3,
                concurrency: 2,
                budget: Duration::from_secs(5),
                calls: 5,
            },
        );
        let report = llm.research_ask(HARD_QUESTION, &[wash_source()]);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(report.calls, 2, "{report:?}");
        assert!(!report.split);
        assert!(report.helpers.is_empty(), "{report:?}");
        assert_eq!(report.answer.standing, "Inferred");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2, "{seen:?}");
        assert_eq!(helper_requests(&seen), 1);
    }

    #[test]
    fn hard_question_runs_helpers_within_concurrency_and_keeps_real_citations() {
        let inflight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let entered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let inflight_w = inflight.clone();
        let max_w = max_in.clone();
        let entered_w = entered.clone();
        let (addr, seen, stop) = serve_calls(move |body| {
            if body.contains("You plan research") {
                return "\
- What colour does the note give the wash?
- How would a cooler mix read at night?
- Which hex is still missing from the note?
"
                .to_string();
            }
            if body.contains("Helper notes:") {
                return "Invented Journal 2024 says the wash is amber and the hex is unknown."
                    .to_string();
            }
            let now = inflight_w.fetch_add(1, Ordering::SeqCst) + 1;
            max_w.fetch_max(now, Ordering::SeqCst);
            let n = entered_w.fetch_add(1, Ordering::SeqCst) + 1;
            if n <= 2 {
                let deadline = Instant::now() + Duration::from_secs(2);
                while entered_w.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(5));
                }
                thread::sleep(Duration::from_millis(40));
            }
            inflight_w.fetch_sub(1, Ordering::SeqCst);
            if body.contains("What colour does the note") {
                "\
The wash is amber.
Found
- The wash is amber. [1]
Inferred
- Daylight is the assumed light.
Unknown
- The hex is not in the note.
[1] wash-note.txt
[9] Invented Journal 2024
"
                .to_string()
            } else if body.contains("cooler mix") {
                "\
Inferred
- A cooler mix would read as night.
Unknown
- Tungsten is not in the note.
"
                .to_string()
            } else {
                "\
Unknown
- The hex code is not written down.
"
                .to_string()
            }
        });
        let llm = budgeted(
            addr,
            4000,
            ResearchBudget {
                helpers: 3,
                concurrency: 2,
                budget: Duration::from_secs(15),
                calls: 5,
            },
        );
        struct Rec(Mutex<Vec<String>>);
        impl ResearchWatch for Rec {
            fn on_research(&self, event: &ResearchEvent) {
                let label = match event {
                    ResearchEvent::Phase(p) => format!("phase:{p}"),
                    ResearchEvent::Calls(n) => format!("calls:{n}"),
                    ResearchEvent::Budget(n) => format!("budget:{n}"),
                    ResearchEvent::Helper(i, h) => format!("helper:{i}:{}", h.status),
                };
                self.0.lock().unwrap().push(label);
            }
        }
        let watch = Rec(Mutex::new(Vec::new()));
        let report = llm.research_ask_watched(HARD_QUESTION, &[wash_source()], &watch);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(report.calls, 5, "{report:?}");
        assert!(report.split);
        assert_eq!(report.helpers.len(), 3, "{report:?}");
        assert!(
            report.helpers.iter().all(|h| h.status != "skipped"),
            "{report:?}"
        );
        assert_eq!(report.answer.standing, "Found", "{report:?}");
        assert_eq!(report.answer.citations, vec![(1, "wash-note.txt".into())]);
        assert!(
            report.answer.found.iter().any(|s| s.contains("amber")),
            "{report:?}"
        );
        assert!(
            report
                .answer
                .inferred
                .iter()
                .any(|s| s.contains("cooler") || s.contains("Daylight")),
            "{report:?}"
        );
        assert!(
            report.answer.unknown.iter().any(|s| s.contains("hex")),
            "{report:?}"
        );
        assert!(
            !report.answer.text.contains("Invented"),
            "{}",
            report.answer.text
        );
        assert!(report
            .answer
            .citations
            .iter()
            .all(|(_, t)| !t.contains("Invented")));
        assert!(
            !format!("{report:?}").contains("Invented Journal"),
            "{report:?}"
        );
        assert_eq!(max_in.load(Ordering::SeqCst), 2, "concurrency is 2");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(helper_requests(&seen), 3, "{seen:?}");
        assert!(seen.iter().any(|s| s.contains("You plan research")));
        assert!(seen.iter().any(|s| s.contains("Helper notes:")));
        let log = watch.0.lock().unwrap().clone();
        assert!(log.iter().any(|e| e == "phase:planning"), "{log:?}");
        assert!(log.iter().any(|e| e == "phase:helpers"), "{log:?}");
        assert!(log.iter().any(|e| e == "phase:synthesis"), "{log:?}");
        assert!(log.iter().any(|e| e == "calls:5"), "{log:?}");
    }

    #[test]
    fn call_budget_skips_helpers_past_the_reserve() {
        let (addr, seen, stop) = serve_calls(|body| {
            if body.contains("You plan research") {
                return "\
- What colour does the note give the wash?
- How would a cooler mix read at night?
- Which hex is still missing from the note?
"
                .to_string();
            }
            if body.contains("Helper notes:") {
                return "The wash is amber and the hex is not in the note.".to_string();
            }
            if body.contains("What colour does the note") {
                "Found\n- The wash is amber. [1]\n[1] wash-note.txt\n".to_string()
            } else if body.contains("cooler mix") {
                "Inferred\n- A cooler mix would read as night.\n".to_string()
            } else {
                "Unknown\n- The hex code is not written down.\n".to_string()
            }
        });
        let llm = budgeted(
            addr,
            1000,
            ResearchBudget {
                helpers: 5,
                concurrency: 2,
                budget: Duration::from_secs(5),
                calls: 4,
            },
        );
        let report = llm.research_ask(HARD_QUESTION, &[wash_source()]);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(report.calls, 4, "{report:?}");
        assert!(report.split);
        assert_eq!(
            report
                .helpers
                .iter()
                .filter(|h| h.status == "skipped")
                .count(),
            1,
            "{report:?}"
        );
        assert!(
            report
                .helpers
                .iter()
                .any(|h| h.question.contains("still missing") && h.status == "skipped"),
            "{report:?}"
        );
        assert_eq!(report.answer.citations, vec![(1, "wash-note.txt".into())]);
        assert!(!format!("{report:?}").contains("Invented"));
        let seen = seen.lock().unwrap().clone();
        assert_eq!(helper_requests(&seen), 2, "{seen:?}");
        assert!(
            seen.iter().all(|s| !s.contains("still missing")),
            "{seen:?}"
        );
        assert!(
            report.answer.text.contains("amber")
                || report.answer.found.iter().any(|s| s.contains("amber"))
        );
    }

    #[test]
    fn time_budget_does_not_start_the_next_helper() {
        let helpers_n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let helpers_w = helpers_n.clone();
        let (addr, seen, stop) = serve_calls(move |body| {
            if body.contains("You plan research") {
                return "\
- What colour does the note give the wash?
- How would a cooler mix read at night?
- Which hex is still missing from the note?
"
                .to_string();
            }
            helpers_w.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_secs(2));
            "Found\n- The wash is amber. [1]\n[1] wash-note.txt\n".to_string()
        });
        let llm = budgeted(
            addr,
            5_000,
            ResearchBudget {
                helpers: 3,
                concurrency: 1,
                budget: Duration::from_millis(900),
                calls: 7,
            },
        );
        let report = llm.research_ask(HARD_QUESTION, &[wash_source()]);
        stop.store(true, Ordering::SeqCst);
        assert!(report.calls <= 2, "calls {}", report.calls);
        assert!(
            helpers_n.load(Ordering::SeqCst) <= 1,
            "helpers {}",
            helpers_n.load(Ordering::SeqCst)
        );
        let seen = seen.lock().unwrap().clone();
        assert!(helper_requests(&seen) <= 1, "{seen:?}");
        if report.calls == 2 {
            assert!(report.split, "{report:?}");
            assert!(
                report.helpers.iter().any(|h| h.status == "skipped"),
                "{report:?}"
            );
            assert_eq!(report.answer.standing, "Unreachable", "{report:?}");
        }
    }

    #[test]
    fn merge_keeps_helper_labels_and_drops_an_invented_lead() {
        let part = ResearchAnswer {
            text: "The wash is amber.".into(),
            citations: vec![(1, "wash-note.txt".into())],
            found: vec!["The wash is amber.".into()],
            inferred: vec!["Daylight is the assumed light.".into()],
            unknown: vec!["The hex is not in the note.".into()],
            standing: "Found",
        };
        let dirty = merge_helpers(
            &[part.clone()],
            "Invented Journal 2024 says the wash is amber.",
            &[wash_source()],
            HARD_QUESTION,
        );
        assert_eq!(dirty.citations, vec![(1, "wash-note.txt".into())]);
        assert_eq!(dirty.standing, "Found");
        assert!(dirty.found.iter().any(|s| s.contains("amber")), "{dirty:?}");
        assert!(
            dirty.inferred.iter().any(|s| s.contains("Daylight")),
            "{dirty:?}"
        );
        assert!(dirty.unknown.iter().any(|s| s.contains("hex")), "{dirty:?}");
        assert!(!dirty.text.contains("Invented"), "{}", dirty.text);
        assert_eq!(dirty.text, "The wash is amber.");
        let clean = merge_helpers(
            &[part],
            "The wash is amber and daylight is the assumed light.",
            &[wash_source()],
            HARD_QUESTION,
        );
        assert!(clean.text.contains("amber"), "{}", clean.text);
        assert!(clean.text.contains("daylight"), "{}", clean.text);
        assert!(!clean.text.contains("Invented"));
    }

}
