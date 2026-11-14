//! Galion Research splits a hard question into helper asks, then merges them.
//!
//! G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
//! Developed and distributed by Galion Labs
//! Perpetual Open MIT License
//! Copyright © 2026 Galion Labs. All rights reserved.
//!
//! The planner proposes at most N sub-questions. Each one runs as a helper
//! ask, with a concurrency bound. A synthesis step merges them. Citations in
//! that merge are kept only when they name a source from this session.
//!
//! No function in this module opens a socket. The caller supplies `ask`.
//! Tests pass a stub. The desk passes [`crate::HermesLlm`], which is the
//! only path that connects.
//!
//! One user question has a hard call cap. [`CallBudget::try_take`] is the
//! guard: a model call that does not take a slot is not made.
//!
//! ```text
//! GALION_RESEARCH_HELPERS=2                 # N, sub-questions. 0 disables the split. Clamp 0..=4
//! GALION_RESEARCH_HELPER_CONCURRENCY=2      # in-flight helper asks. Clamp 1..=4
//! GALION_RESEARCH_CALL_BUDGET=4             # Hermes calls per question. Clamp 1..=8
//! ```
//!
//! Defaults are the small end. Values above the clamp are not a budget.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::hermes::{
    classify_research, contains_forbidden, research_source_block, ResearchAnswer, ResearchSource,
    REFUSED, UNREACHABLE,
};

/// Sub-questions the planner may propose. `0` keeps the single ask.
pub const DEFAULT_HELPERS: usize = 2;
/// Helper asks in flight at once.
pub const DEFAULT_CONCURRENCY: usize = 2;
/// Hermes calls for one user question, including the planner and the synthesis.
pub const DEFAULT_CALL_BUDGET: usize = 4;
/// Env and a hand-built config cannot raise N past this.
pub const HARD_HELPERS: usize = 4;
/// Env and a hand-built config cannot raise concurrency past this.
pub const HARD_CONCURRENCY: usize = 4;
/// Env and a hand-built config cannot raise the per-question call cap past this.
pub const HARD_CALL_BUDGET: usize = 8;

/// Planner system prompt. The user message carries the question and N.
pub const PLAN_PROMPT: &str = "You are Galion Research planning a hard question. \
You do not answer it. Propose smaller questions that together answer it. \
Write one question per line, at most the number asked. No numbering, no citations, no headings. \
If it is already one step, write that one question. \
Do not invent a paper, a URL, a date, a measurement, or a source. English only.";

/// Synthesis system prompt. Found lines may cite only the Sources list on this turn.
pub const SYNTH_PROMPT: &str =
    "You are Galion Research, the research app on Hermes, a voice of Galion One. \
Merge the helper notes into one answer. Answer the question first, then the limit. \
Separate found, inferred, and unknown under those headings, each heading on its own line. \
A Found line is a claim a provided source states. End it with that source id, written [n]. \
An Inferred line is a step you reasoned and no provided source states. \
An Unknown line is a claim you cannot settle from the sources. \
Cite only an id from the Sources list. One citation per line, written [n] and the source name. \
If the Sources list says none, write no citation and leave Found empty. \
Do not invent a paper, a URL, a date, a measurement, or a source id. \
Do not add a helper's guess as Found. English only.";

/// Which step is asking. The stub and the Hermes client both branch on this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskRole {
    Plan,
    Helper,
    Synthesize,
}

/// One helper the planner proposed and the desk actually ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperAsk {
    pub question: String,
    pub answer: ResearchAnswer,
}

/// The merged answer plus the helpers that ran. `calls` is how many times
/// `ask` was invoked. It is never above the call budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplitResearch {
    pub answer: ResearchAnswer,
    pub helpers: Vec<HelperAsk>,
    pub calls: usize,
}

impl SplitResearch {
    /// No client and no stub. Nothing was asked.
    pub fn unreachable() -> Self {
        Self {
            answer: ResearchAnswer {
                text: UNREACHABLE.into(),
                citations: Vec::new(),
                found: Vec::new(),
                inferred: Vec::new(),
                unknown: Vec::new(),
                standing: "Unreachable",
            },
            helpers: Vec::new(),
            calls: 0,
        }
    }
}

/// Knobs for one split. Build with [`SplitConfig::from_env`] or [`Default`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitConfig {
    /// Maximum sub-questions. `0` is one direct ask and no planner.
    pub helpers: usize,
    /// Helper asks allowed in flight.
    pub concurrency: usize,
    /// Hard cap on `ask` calls for this question.
    pub budget: usize,
}

impl Default for SplitConfig {
    fn default() -> Self {
        Self {
            helpers: DEFAULT_HELPERS,
            concurrency: DEFAULT_CONCURRENCY,
            budget: DEFAULT_CALL_BUDGET,
        }
    }
}

impl SplitConfig {
    /// Read the three `GALION_RESEARCH_*` variables. Missing or unparsable
    /// values stay at the defaults. Numbers outside the hard range clamp.
    pub fn from_env() -> Self {
        Self::from_vars(|k| std::env::var(k).ok())
    }

    /// [`SplitConfig::from_env`] with injected lookups. Tests use this so
    /// they do not touch the process environment.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            helpers: env_usize(
                &var,
                "GALION_RESEARCH_HELPERS",
                DEFAULT_HELPERS,
                0,
                HARD_HELPERS,
            ),
            concurrency: env_usize(
                &var,
                "GALION_RESEARCH_HELPER_CONCURRENCY",
                DEFAULT_CONCURRENCY,
                1,
                HARD_CONCURRENCY,
            ),
            budget: env_usize(
                &var,
                "GALION_RESEARCH_CALL_BUDGET",
                DEFAULT_CALL_BUDGET,
                1,
                HARD_CALL_BUDGET,
            ),
        }
    }

    fn clamped(self) -> Self {
        Self {
            helpers: self.helpers.min(HARD_HELPERS),
            concurrency: self.concurrency.clamp(1, HARD_CONCURRENCY),
            budget: self.budget.clamp(1, HARD_CALL_BUDGET),
        }
    }
}

fn env_usize(
    var: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: usize,
    min: usize,
    max: usize,
) -> usize {
    let Some(raw) = var(key) else {
        return default;
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return default;
    }
    match raw.parse::<usize>() {
        Ok(n) => n.clamp(min, max),
        Err(_) => default,
    }
}

/// Hard cap on Hermes calls for one user question.
///
/// `try_take` is the guard. It returns false once `used` reaches `limit`,
/// and it never lets `used` pass `limit`. The limit itself cannot pass
/// [`HARD_CALL_BUDGET`].
pub struct CallBudget {
    limit: usize,
    used: AtomicUsize,
}

impl CallBudget {
    pub fn new(limit: usize) -> Self {
        Self {
            limit: limit.min(HARD_CALL_BUDGET),
            used: AtomicUsize::new(0),
        }
    }

    /// Reserve one call. False means the budget is spent and the caller
    /// must not invoke the model.
    pub fn try_take(&self) -> bool {
        let mut cur = self.used.load(Ordering::SeqCst);
        loop {
            if cur >= self.limit {
                return false;
            }
            match self
                .used
                .compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(seen) => cur = seen,
            }
        }
    }

    pub fn used(&self) -> usize {
        self.used.load(Ordering::SeqCst)
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn remaining(&self) -> usize {
        self.limit.saturating_sub(self.used())
    }
}

/// Call budget guard. The only place `ask` is invoked.
fn take(
    budget: &CallBudget,
    ask: &impl Fn(AskRole, &str) -> String,
    role: AskRole,
    user: &str,
) -> Option<String> {
    if !budget.try_take() {
        return None;
    }
    Some(ask(role, user))
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn norm_name(s: &str) -> String {
    s.trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn session_source<'a>(
    sources: &'a [ResearchSource],
    id: u32,
    name: &str,
) -> Option<&'a ResearchSource> {
    let name = norm_name(name);
    sources
        .iter()
        .find(|s| s.id == id && !s.name.trim().is_empty() && norm_name(&s.name) == name)
}

fn is_heading(line: &str) -> bool {
    matches!(
        line.trim()
            .trim_end_matches(':')
            .to_ascii_lowercase()
            .as_str(),
        "found" | "inferred" | "unknown" | "helpers" | "sources" | "question" | "citations"
    )
}

fn is_citation_line(line: &str) -> bool {
    let t = line.trim().trim_start_matches(['-', '*', '•']).trim();
    let Some(rest) = t.strip_prefix('[') else {
        return false;
    };
    let Some((n, _)) = rest.split_once(']') else {
        return false;
    };
    let n = n.trim();
    !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())
}

fn strip_marker(line: &str) -> &str {
    let t = line.trim().trim_start_matches(['-', '*', '•']).trim();
    let mut chars = t.chars();
    let mut digits = 0usize;
    while chars.next().is_some_and(|c| c.is_ascii_digit()) {
        digits += 1;
        if digits > 2 {
            return t;
        }
    }
    if digits == 0 {
        return t;
    }
    let rest = &t[digits..];
    if let Some(rest) = rest
        .strip_prefix('.')
        .or_else(|| rest.strip_prefix(')'))
        .or_else(|| rest.strip_prefix(':'))
    {
        return rest.trim();
    }
    t
}

/// Planner lines, capped at `n`. Headings, citation lines, and forbidden
/// scripts are dropped. The order is the planner's order.
pub fn parse_subquestions(raw: &str, n: usize) -> Vec<String> {
    if n == 0 {
        return Vec::new();
    }
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == UNREACHABLE || trimmed == REFUSED {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in raw.lines() {
        if is_heading(line) || is_citation_line(line) {
            continue;
        }
        let t = strip_marker(line);
        if t.is_empty() || is_heading(t) || is_citation_line(t) {
            continue;
        }
        if contains_forbidden(t) {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if matches!(low.as_str(), "none" | "direct" | "n/a" | "na") {
            continue;
        }
        if !t.chars().any(|c| c.is_alphabetic()) {
            continue;
        }
        let t = clip(t, 240);
        if t.chars().count() < 8 {
            continue;
        }
        if out.iter().any(|e: &String| e.eq_ignore_ascii_case(&t)) {
            continue;
        }
        out.push(t);
        if out.len() >= n {
            break;
        }
    }
    out
}

fn plan_user(question: &str, n: usize, sources: &[ResearchSource]) -> String {
    format!(
        "Question:\n{}\n\nPropose at most {n} smaller questions, one per line. Do not answer.\n\n{}",
        clip(question.trim(), 2_000),
        research_source_block(sources)
    )
}

fn helper_user(question: &str, sources: &[ResearchSource]) -> String {
    format!(
        "{}\n\n{}",
        clip(question.trim(), 2_000),
        research_source_block(sources)
    )
}

fn synth_user(question: &str, helpers: &[HelperAsk], sources: &[ResearchSource]) -> String {
    let mut s = format!("Question:\n{}\n\n", clip(question.trim(), 2_000));
    s.push_str(&research_source_block(sources));
    s.push_str("\nHelper notes:\n");
    if helpers.is_empty() {
        s.push_str("(none)\n");
    }
    for (i, h) in helpers.iter().enumerate() {
        let n = i + 1;
        s.push_str(&format!(
            "Helper {n}\nQuestion: {}\nStanding: {}\nReply: {}\n",
            clip(&h.question, 240),
            h.answer.standing,
            clip(&h.answer.text, 800)
        ));
        let cites: Vec<&(u32, String)> = h
            .answer
            .citations
            .iter()
            .filter(|(id, name)| session_source(sources, *id, name).is_some())
            .collect();
        if cites.is_empty() {
            s.push_str("Citations: none\n");
        } else {
            for (id, name) in cites {
                if let Some(src) = session_source(sources, *id, name) {
                    s.push_str(&format!("Citation: [{id}] {}\n", src.name.trim()));
                }
            }
        }
    }
    s.push_str("\nMerge these notes. Cite only an id from the Sources list.\n");
    s
}

fn push_claim(into: &mut Vec<String>, line: &str) {
    let t = clip(line.trim(), 400);
    if t.is_empty() || t == UNREACHABLE || t == REFUSED {
        return;
    }
    if into.iter().any(|e| e == &t) || into.len() >= 8 {
        return;
    }
    into.push(t);
}

/// Drop any citation that is not one of this session's sources.
/// A Found pile with nothing left to cite becomes Unknown.
fn limit_to_session(mut answer: ResearchAnswer, sources: &[ResearchSource]) -> ResearchAnswer {
    answer
        .citations
        .retain(|(id, name)| session_source(sources, *id, name).is_some());
    answer.citations.iter_mut().for_each(|(id, name)| {
        if let Some(src) = sources.iter().find(|s| s.id == *id) {
            *name = src.name.trim().to_string();
        }
    });
    if answer.citations.is_empty() && !answer.found.is_empty() {
        let moved = std::mem::take(&mut answer.found);
        for line in moved {
            push_claim(&mut answer.unknown, &line);
        }
        answer.standing = if answer.inferred.is_empty() {
            "Unknown"
        } else {
            "Inferred"
        };
    }
    answer
}

fn merge_local(helpers: &[HelperAsk], sources: &[ResearchSource]) -> ResearchAnswer {
    if helpers.is_empty() {
        return ResearchAnswer {
            text: "Unknown.".into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: vec!["Unknown.".into()],
            standing: "Unknown",
        };
    }
    if helpers.iter().all(|h| h.answer.standing == "Unreachable") {
        return ResearchAnswer {
            text: UNREACHABLE.into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing: "Unreachable",
        };
    }
    if helpers
        .iter()
        .all(|h| h.answer.standing == "Denied" || h.answer.standing == "Unreachable")
        && helpers.iter().any(|h| h.answer.standing == "Denied")
    {
        return ResearchAnswer {
            text: REFUSED.into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing: "Denied",
        };
    }
    let mut found = Vec::new();
    let mut inferred = Vec::new();
    let mut unknown = Vec::new();
    let mut citations: Vec<(u32, String)> = Vec::new();
    for h in helpers {
        if h.answer.standing == "Unreachable" || h.answer.standing == "Denied" {
            continue;
        }
        for line in &h.answer.found {
            push_claim(&mut found, line);
        }
        for line in &h.answer.inferred {
            push_claim(&mut inferred, line);
        }
        for line in &h.answer.unknown {
            push_claim(&mut unknown, line);
        }
        for (id, name) in &h.answer.citations {
            if session_source(sources, *id, name).is_some()
                && citations.iter().all(|(n, _)| n != id)
            {
                if let Some(src) = sources.iter().find(|s| s.id == *id) {
                    citations.push((*id, src.name.trim().to_string()));
                }
            }
        }
    }
    let mut answer = ResearchAnswer {
        text: String::new(),
        citations,
        found,
        inferred,
        unknown,
        standing: "Unknown",
    };
    answer = limit_to_session(answer, sources);
    answer.standing = if !answer.found.is_empty() {
        "Found"
    } else if !answer.inferred.is_empty() {
        "Inferred"
    } else {
        "Unknown"
    };
    answer.text = answer
        .found
        .first()
        .or(answer.inferred.first())
        .or(answer.unknown.first())
        .cloned()
        .unwrap_or_else(|| "Unknown.".into());
    answer
}

fn finish_synth(raw: &str, helpers: &[HelperAsk], sources: &[ResearchSource]) -> ResearchAnswer {
    let t = raw.trim();
    if t.is_empty() || t == UNREACHABLE {
        return merge_local(helpers, sources);
    }
    if t == REFUSED {
        return ResearchAnswer {
            text: REFUSED.into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing: "Denied",
        };
    }
    let answer = limit_to_session(classify_research(raw, sources), sources);
    if answer.standing == "Unreachable" {
        return merge_local(helpers, sources);
    }
    answer
}

fn one_helper(
    question: &str,
    sources: &[ResearchSource],
    budget: &CallBudget,
    ask: &impl Fn(AskRole, &str) -> String,
) -> Option<HelperAsk> {
    let raw = take(
        budget,
        ask,
        AskRole::Helper,
        &helper_user(question, sources),
    )?;
    let answer = limit_to_session(classify_research(&raw, sources), sources);
    Some(HelperAsk {
        question: question.to_string(),
        answer,
    })
}

fn run_helpers(
    questions: &[String],
    sources: &[ResearchSource],
    concurrency: usize,
    budget: &CallBudget,
    ask: &(impl Fn(AskRole, &str) -> String + Sync),
) -> Vec<HelperAsk> {
    if questions.is_empty() {
        return Vec::new();
    }
    let n = questions.len();
    let workers = concurrency.max(1).min(n);
    if workers == 1 {
        return questions
            .iter()
            .filter_map(|q| one_helper(q, sources, budget, ask))
            .collect();
    }
    let slots: Vec<Mutex<Option<HelperAsk>>> = (0..n).map(|_| Mutex::new(None)).collect();
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= n {
                    break;
                }
                if let Some(got) = one_helper(&questions[i], sources, budget, ask) {
                    *slots[i].lock().unwrap() = Some(got);
                }
            });
        }
    });
    slots
        .into_iter()
        .filter_map(|m| m.into_inner().unwrap())
        .collect()
}

fn direct(
    question: &str,
    sources: &[ResearchSource],
    budget: &CallBudget,
    ask: &impl Fn(AskRole, &str) -> String,
) -> SplitResearch {
    let Some(raw) = take(
        budget,
        ask,
        AskRole::Helper,
        &helper_user(question, sources),
    ) else {
        return SplitResearch {
            answer: merge_local(&[], sources),
            helpers: Vec::new(),
            calls: budget.used(),
        };
    };
    let answer = if raw.trim() == UNREACHABLE {
        ResearchAnswer {
            text: UNREACHABLE.into(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing: "Unreachable",
        }
    } else {
        limit_to_session(classify_research(&raw, sources), sources)
    };
    SplitResearch {
        answer,
        helpers: Vec::new(),
        calls: budget.used(),
    }
}

fn stopped(text: &str, standing: &'static str, calls: usize) -> SplitResearch {
    SplitResearch {
        answer: ResearchAnswer {
            text: text.to_string(),
            citations: Vec::new(),
            found: Vec::new(),
            inferred: Vec::new(),
            unknown: Vec::new(),
            standing,
        },
        helpers: Vec::new(),
        calls,
    }
}

/// Plan, run helper asks, synthesise.
///
/// `ask` is the Hermes stand-in. It is called only through [`CallBudget::try_take`].
/// The returned `calls` count is that many invocations, and it is at most the
/// configured budget, which itself is at most [`HARD_CALL_BUDGET`].
pub fn split_question(
    question: &str,
    sources: &[ResearchSource],
    cfg: SplitConfig,
    ask: &(impl Fn(AskRole, &str) -> String + Sync),
) -> SplitResearch {
    let cfg = cfg.clamped();
    let budget = CallBudget::new(cfg.budget);
    let question = question.trim();
    if question.is_empty() {
        return stopped(UNREACHABLE, "Unreachable", 0);
    }
    // A budget of one, or helpers turned off, is the single ask. No planner.
    if cfg.helpers == 0 || budget.limit() < 2 {
        return direct(question, sources, &budget, ask);
    }

    let Some(plan_raw) = take(
        &budget,
        ask,
        AskRole::Plan,
        &plan_user(question, cfg.helpers, sources),
    ) else {
        return stopped(UNREACHABLE, "Unreachable", budget.used());
    };
    let plan_trim = plan_raw.trim();
    if plan_trim == UNREACHABLE {
        return stopped(UNREACHABLE, "Unreachable", budget.used());
    }
    if plan_trim == REFUSED {
        return stopped(REFUSED, "Denied", budget.used());
    }

    let mut subs = parse_subquestions(&plan_raw, cfg.helpers);
    let room = budget.remaining();
    // Leave one call for the model synthesis when the budget can still hold
    // a helper and that merge. Otherwise the last call is a helper and the
    // merge is local, still limited to this session's sources.
    let helper_slots = if room >= 2 { room - 1 } else { room };
    if subs.is_empty() && helper_slots > 0 {
        subs.push(clip(question, 240));
    }
    subs.truncate(helper_slots);

    let helpers = run_helpers(&subs, sources, cfg.concurrency, &budget, ask);
    let answer = match take(
        &budget,
        ask,
        AskRole::Synthesize,
        &synth_user(question, &helpers, sources),
    ) {
        Some(raw) => finish_synth(&raw, &helpers, sources),
        None => merge_local(&helpers, sources),
    };
    SplitResearch {
        answer,
        helpers,
        calls: budget.used(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    fn wash() -> ResearchSource {
        ResearchSource {
            id: 1,
            name: "wash-note.txt".into(),
            text: "The wash is amber.".into(),
        }
    }

    #[test]
    fn call_budget_guard_refuses_past_the_cap() {
        let budget = CallBudget::new(2);
        assert!(budget.try_take());
        assert!(budget.try_take());
        assert!(
            !budget.try_take(),
            "the guard must refuse the call past the cap"
        );
        assert_eq!(budget.used(), 2);
        assert_eq!(budget.remaining(), 0);
        let capped = CallBudget::new(10_000);
        assert_eq!(capped.limit(), HARD_CALL_BUDGET);
        let mut taken = 0;
        while capped.try_take() {
            taken += 1;
            assert!(taken <= HARD_CALL_BUDGET);
        }
        assert_eq!(taken, HARD_CALL_BUDGET);
        assert!(!capped.try_take());
    }

    #[test]
    fn config_defaults_are_small_and_env_cannot_raise_the_hard_cap() {
        let d = SplitConfig::default();
        assert_eq!(d.helpers, 2);
        assert_eq!(d.concurrency, 2);
        assert_eq!(d.budget, 4);
        let raised = SplitConfig::from_vars(|k| match k {
            "GALION_RESEARCH_HELPERS" => Some("99".into()),
            "GALION_RESEARCH_HELPER_CONCURRENCY" => Some("0".into()),
            "GALION_RESEARCH_CALL_BUDGET" => Some("1000".into()),
            _ => None,
        });
        assert_eq!(raised.helpers, HARD_HELPERS);
        assert_eq!(raised.concurrency, 1);
        assert_eq!(raised.budget, HARD_CALL_BUDGET);
        let off = SplitConfig::from_vars(|k| (k == "GALION_RESEARCH_HELPERS").then(|| "0".into()));
        assert_eq!(off.helpers, 0);
        assert_eq!(off.concurrency, DEFAULT_CONCURRENCY);
        assert_eq!(off.budget, DEFAULT_CALL_BUDGET);
        let messy =
            SplitConfig::from_vars(|k| (k == "GALION_RESEARCH_CALL_BUDGET").then(|| "nope".into()));
        assert_eq!(messy.budget, DEFAULT_CALL_BUDGET);
    }

    #[test]
    fn parser_keeps_at_most_n_questions() {
        let raw = "\
1. First question here?
- Second question here?
Found
[1] note.txt
Third question here?
none
";
        let q = parse_subquestions(raw, 2);
        assert_eq!(
            q,
            vec![
                "First question here?".to_string(),
                "Second question here?".to_string()
            ]
        );
        assert!(parse_subquestions(UNREACHABLE, 4).is_empty());
        assert!(parse_subquestions("I'm sorry to hear that.", 4).is_empty());
    }

    #[test]
    fn stub_hermes_splits_without_opening_a_socket() {
        let calls = AtomicUsize::new(0);
        let seen = Mutex::new(Vec::<(AskRole, String)>::new());
        let ask = |role: AskRole, user: &str| {
            let n = calls.fetch_add(1, Ordering::SeqCst);
            assert!(n < 4, "call budget guard let call {n} through");
            seen.lock().unwrap().push((role, user.to_string()));
            match role {
                AskRole::Plan => "\
Which note names the wash?
What is the hex code?
Who published the measurement?
Where was the note printed?
"
                .into(),
                AskRole::Helper => "\
Found
- The wash is amber. [1]
- A journal measured it. [9]
[1] wash-note.txt
[9] Invented Journal
"
                .into(),
                AskRole::Synthesize => "\
The wash is amber.
Found
- The wash is amber. [1]
[1] wash-note.txt
[9] Invented Journal
"
                .into(),
            }
        };
        let cfg = SplitConfig {
            helpers: 4,
            concurrency: 2,
            budget: 4,
        };
        let out = split_question("What colour is the wash?", &[wash()], cfg, &ask);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            4,
            "plan + two helpers + synthesis"
        );
        assert_eq!(out.calls, 4);
        assert_eq!(out.helpers.len(), 2);
        assert_eq!(out.helpers[0].question, "Which note names the wash?");
        assert_eq!(out.helpers[1].question, "What is the hex code?");
        let joined = seen
            .lock()
            .unwrap()
            .iter()
            .map(|(_, u)| u.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !joined.contains("Who published the measurement?"),
            "{joined}"
        );
        assert!(!joined.contains("Where was the note printed?"), "{joined}");
        assert_eq!(out.answer.standing, "Found");
        assert_eq!(out.answer.citations, vec![(1, "wash-note.txt".into())]);
        assert!(out
            .answer
            .citations
            .iter()
            .all(|(_, t)| !t.contains("Invented")));
        assert!(!out.answer.text.contains("Invented"), "{}", out.answer.text);
        let synth = seen
            .lock()
            .unwrap()
            .iter()
            .find(|(r, _)| *r == AskRole::Synthesize)
            .unwrap()
            .1
            .clone();
        assert!(synth.contains("wash-note.txt"), "{synth}");
        assert!(
            !synth.contains("Invented"),
            "synthesis must not be handed a source the session does not have: {synth}"
        );
        let roles: Vec<AskRole> = seen.lock().unwrap().iter().map(|(r, _)| *r).collect();
        assert_eq!(roles.iter().filter(|r| **r == AskRole::Plan).count(), 1);
        assert_eq!(roles.iter().filter(|r| **r == AskRole::Helper).count(), 2);
        assert_eq!(
            roles.iter().filter(|r| **r == AskRole::Synthesize).count(),
            1
        );
    }

    #[test]
    fn synthesis_drops_a_citation_the_session_did_not_provide() {
        let ask = |role: AskRole, _user: &str| match role {
            AskRole::Plan => "Which note names the wash?".into(),
            AskRole::Helper => "Found\n- The wash is amber. [1]\n[1] wash-note.txt\n".into(),
            AskRole::Synthesize => "\
Found
- The wash is amber. [1]
- A paper measured violet. [3]
[1] wash-note.txt
[3] Invented Journal 2024
"
            .into(),
        };
        let out = split_question(
            "What colour is the wash?",
            &[wash()],
            SplitConfig {
                helpers: 2,
                concurrency: 1,
                budget: 4,
            },
            &ask,
        );
        assert_eq!(
            out.calls, 3,
            "one sub-question still leaves the synthesis slot"
        );
        assert_eq!(out.answer.citations, vec![(1, "wash-note.txt".into())]);
        assert!(out.answer.standing == "Found", "{:?}", out.answer);
        assert!(!format!("{:?}", out.answer).contains("Invented"));
        assert!(out.answer.citations.iter().all(|(n, _)| *n != 3));
    }

    #[test]
    fn a_dead_planner_does_not_spend_the_rest_of_the_budget() {
        let calls = AtomicUsize::new(0);
        let ask = |role: AskRole, _user: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(role, AskRole::Plan);
            UNREACHABLE.to_string()
        };
        let out = split_question(
            "What colour is the wash?",
            &[wash()],
            SplitConfig::default(),
            &ask,
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(out.calls, 1);
        assert!(out.helpers.is_empty());
        assert_eq!(out.answer.standing, "Unreachable");
    }

    #[test]
    fn helpers_off_is_one_direct_ask() {
        let calls = AtomicUsize::new(0);
        let ask = |role: AskRole, user: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(role, AskRole::Helper);
            assert!(user.contains("wash-note.txt"), "{user}");
            "Inferred\n- The wash reads as amber.\n".into()
        };
        let out = split_question(
            "What colour is the wash?",
            &[wash()],
            SplitConfig {
                helpers: 0,
                concurrency: 4,
                budget: 8,
            },
            &ask,
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(out.helpers.is_empty());
        assert_eq!(out.answer.standing, "Inferred");
        assert!(out.answer.citations.is_empty());
    }

    fn hold(inflight: &AtomicUsize, max: &AtomicUsize, siblings: usize) {
        let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
        max.fetch_max(now, Ordering::SeqCst);
        let start = Instant::now();
        while inflight.load(Ordering::SeqCst) < siblings
            && start.elapsed() < Duration::from_millis(250)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        inflight.fetch_sub(1, Ordering::SeqCst);
    }

    #[test]
    fn two_helpers_run_together_and_one_does_not() {
        let inflight = AtomicUsize::new(0);
        let max = AtomicUsize::new(0);
        let ask = |role: AskRole, _user: &str| match role {
            AskRole::Plan => {
                "What is the first part of this?\nWhat is the second part of this?".into()
            }
            AskRole::Helper => {
                hold(&inflight, &max, 2);
                "Inferred\n- A part was noted.\n".into()
            }
            AskRole::Synthesize => "Inferred\n- The parts agree.\n".into(),
        };
        let wide = split_question(
            "How does the whole thing fit together?",
            &[],
            SplitConfig {
                helpers: 2,
                concurrency: 2,
                budget: 4,
            },
            &ask,
        );
        assert_eq!(
            max.load(Ordering::SeqCst),
            2,
            "concurrency 2 lets both helpers overlap"
        );
        assert_eq!(wide.helpers.len(), 2);
        assert_eq!(wide.calls, 4);

        let inflight = AtomicUsize::new(0);
        let max = AtomicUsize::new(0);
        let ask = |role: AskRole, _user: &str| match role {
            AskRole::Plan => {
                "What is the first part of this?\nWhat is the second part of this?".into()
            }
            AskRole::Helper => {
                hold(&inflight, &max, 2);
                "Inferred\n- A part was noted.\n".into()
            }
            AskRole::Synthesize => "Inferred\n- The parts agree.\n".into(),
        };
        let started = Instant::now();
        let narrow = split_question(
            "How does the whole thing fit together?",
            &[],
            SplitConfig {
                helpers: 2,
                concurrency: 1,
                budget: 4,
            },
            &ask,
        );
        assert_eq!(
            max.load(Ordering::SeqCst),
            1,
            "concurrency 1 keeps the second helper waiting"
        );
        assert!(
            started.elapsed() >= Duration::from_millis(200),
            "the serial helper held its slot"
        );
        assert_eq!(narrow.helpers.len(), 2);
        assert_eq!(narrow.calls, 4);
        assert_eq!(narrow.answer.standing, "Inferred");
    }

    #[test]
    fn budget_of_one_skips_the_planner() {
        let calls = AtomicUsize::new(0);
        let ask = |role: AskRole, _user: &str| {
            calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(role, AskRole::Helper);
            "Unknown\n- The note does not say.\n".into()
        };
        let out = split_question(
            "What colour is the wash?",
            &[wash()],
            SplitConfig {
                helpers: 4,
                concurrency: 4,
                budget: 1,
            },
            &ask,
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(out.calls, 1);
        assert!(out.helpers.is_empty());
        assert_eq!(out.answer.standing, "Unknown");
    }
}
