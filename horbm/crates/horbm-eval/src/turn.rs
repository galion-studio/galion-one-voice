// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Full-Duplex-Bench style scores on the virtual clock.
//!
//! Pause takeover, turn-taking latency, and backchannel shape follow
//! Lin et al., arXiv:2503.04721. Stop time and resume-versus-yield follow
//! Full-Duplex-Bench v1.5, arXiv:2507.23159. Overlap ratio is the share of
//! Samantha's audible frames that are also user-voiced. The papers time
//! overlap with stop latency; this ratio is the two-band count on the same
//! timeline. False barge-ins are her own echo, which the canceller has to
//! remove before the ear.
//!
//! Latency gates sit one frame (80 ms) off the measured p50. Rate gates sit
//! 0.05 past the measured p95. A stub stage has no number and no gate.

use horbm_duplex::{Floor, LoopEvent, FRAME_MS};

use crate::drive::Trial;
use crate::percentile::percentile;
use crate::scenario::{Kind, Phase};

/// Measured turn-taking latency is 160 ms. One frame of slack on each side.
const TURN_P50_MIN_MS: f64 = 80.0;
const TURN_P95_MAX_MS: f64 = 240.0;
/// Measured stop time is 160 ms. Same one-frame slack.
const STOP_P50_MIN_MS: f64 = 80.0;
const STOP_P95_MAX_MS: f64 = 240.0;
/// Measured overlap p95 is 0.500. The gate is that value plus 0.05.
const OVERLAP_P95_MAX: f64 = 0.550;

/// Frozen once a stub stage is live and has been measured. `None` means the
/// gate is not set, so a live stage fails closed instead of inventing a limit.
const PAUSE_TOR_P95_MAX: Option<f64> = None;
const BACKCHANNEL_RATE_P95_MAX: Option<f64> = None;
const BACKCHANNEL_OFFSET_P95_MAX: Option<f64> = None;
/// Measured false barge-in p95 is 0 on both echo trials, with the canceller
/// on inside `Duplex::tick`. The gate is that value. Any barge fails.
const FALSE_BARGE_P95_MAX: Option<f64> = Some(0.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Stub,
    Live,
}

impl StageState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stub => "stub",
            Self::Live => "live",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub name: &'static str,
    pub n: usize,
    pub p50: Option<f64>,
    pub p95: Option<f64>,
    pub digits: usize,
    pub gate: Gate,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Gate {
    Off,
    Pass { limit: String },
    Fail { limit: String },
}

impl Gate {
    fn word(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Pass { .. } => "on",
            Self::Fail { .. } => "fail",
        }
    }

    fn ok(&self) -> bool {
        !matches!(self, Self::Fail { .. })
    }
}

pub struct Report {
    pub speech: &'static str,
    pub scenarios: usize,
    pub samples: usize,
    pub end_of_turn: StageState,
    pub backchannel: StageState,
    pub echo: StageState,
    pub end_of_turn_wired: bool,
    pub backchannel_wired: bool,
    pub echo_wired: bool,
    pub eot_probe_took_floor: bool,
    pub echo_probe_ran: usize,
    pub echo_probe_opened: usize,
    pub echo_probe_barged: usize,
    pub metrics: Vec<Metric>,
    pub misses: Vec<String>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.misses.is_empty() && self.metrics.iter().all(|metric| metric.gate.ok())
    }

    #[must_use]
    pub fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "turn-eval s43 clock=virtual persona=samantha speech={} scenarios={} samples={} cite=arxiv:2503.04721,arxiv:2507.23159\n",
            self.speech, self.scenarios, self.samples
        ));
        out.push_str(&format!(
            "stage end_of_turn={} wired={} pr=295 gate={}\n",
            self.end_of_turn.as_str(),
            bit(self.end_of_turn_wired),
            gate_word(self.end_of_turn, pause_gate_is_on(&self.metrics)),
        ));
        out.push_str(&format!(
            "stage backchannel={} wired={} pr=301 gate={}\n",
            self.backchannel.as_str(),
            bit(self.backchannel_wired),
            if self.backchannel == StageState::Stub {
                "off"
            } else {
                "on"
            }
        ));
        out.push_str(&format!(
            "stage echo_cancel={} wired={} pr=292 gate={}\n",
            self.echo.as_str(),
            bit(self.echo_wired),
            if self.echo == StageState::Stub {
                "off"
            } else {
                "on"
            }
        ));
        for metric in &self.metrics {
            out.push_str(&format_metric(metric));
            out.push('\n');
        }
        out.push_str(&format!(
            "probe end_of_turn took_floor={} echo_opened={}/{} echo_barged={}/{}\n",
            bit(self.eot_probe_took_floor),
            self.echo_probe_opened,
            self.echo_probe_ran,
            self.echo_probe_barged,
            self.echo_probe_ran,
        ));
        if !self.misses.is_empty() {
            out.push_str("miss");
            for miss in &self.misses {
                out.push(' ');
                out.push_str(miss);
            }
            out.push('\n');
        }
        out.push_str(if self.passed() {
            "result=pass\n"
        } else {
            "result=fail\n"
        });
        out
    }
}

pub fn score(trials: &[Trial], speech: &'static str) -> Report {
    let mut misses = Vec::new();
    let eot_probe_took_floor = trials
        .iter()
        .find(|trial| trial.kind == Kind::Probe)
        .map(took_floor)
        .unwrap_or(true);
    if !trials.iter().any(|trial| trial.kind == Kind::Probe) {
        misses.push("probe-eot".to_string());
    }
    let nod_count = trials.iter().map(nod_events).sum::<usize>();
    let end_of_turn = if eot_probe_took_floor {
        StageState::Stub
    } else {
        StageState::Live
    };
    let backchannel = if nod_count > 0 || cfg!(duplex_backchannel) {
        StageState::Live
    } else {
        StageState::Stub
    };
    let echo_wired = trials.iter().any(|trial| trial.echo_live) || cfg!(duplex_aec);
    let echo = if trials.iter().any(|trial| trial.echo_live) {
        StageState::Live
    } else {
        StageState::Stub
    };

    let pause = values(trials, Kind::Pause, took_floor_bit);
    let turns = optional(trials, Kind::Turn, turn_latency_ms);
    let stops = optional(trials, Kind::Barge, stop_ms);
    let resumes = resume_bits(trials);
    let overlaps = trials.iter().filter_map(overlap_ratio).collect::<Vec<_>>();
    let barges = values_echo(trials);

    if turns.iter().any(Option::is_none) {
        misses.push("turn-latency-missing".to_string());
    }
    if stops.iter().any(Option::is_none) {
        misses.push("stop-missing".to_string());
    }
    let echo_trials: Vec<&Trial> = trials
        .iter()
        .filter(|trial| matches!(trial.kind, Kind::EchoTone | Kind::EchoNoise))
        .collect();
    if echo_trials.iter().any(|trial| !trial.opened) {
        misses.push("echo-not-opened".to_string());
    }
    for trial in trials {
        if let Some(id) = resume_miss(trial) {
            misses.push(id.to_string());
        }
    }

    let turn_vals: Vec<f64> = turns.iter().copied().flatten().collect();
    let stop_vals: Vec<f64> = stops.iter().copied().flatten().collect();

    let mut metrics = Vec::new();
    metrics.push(rate_metric(
        "pause_handling_tor",
        &pause,
        if end_of_turn == StageState::Live {
            optional_max(PAUSE_TOR_P95_MAX, &pause)
        } else {
            Gate::Off
        },
    ));
    metrics.push(latency_metric(
        "turn_taking_latency_ms",
        &turn_vals,
        band_gate(&turn_vals, TURN_P50_MIN_MS, TURN_P95_MAX_MS),
    ));
    metrics.push(backchannel_rate(trials, backchannel));
    metrics.push(backchannel_offset(trials, backchannel));
    metrics.push(latency_metric(
        "interruption_stop_ms",
        &stop_vals,
        band_gate(&stop_vals, STOP_P50_MIN_MS, STOP_P95_MAX_MS),
    ));
    metrics.push(resume_metric(&resumes));
    metrics.push(rate_metric(
        "overlap_ratio",
        &overlaps,
        max_gate(&overlaps, OVERLAP_P95_MAX),
    ));
    metrics.push(false_barge(&barges, echo));

    Report {
        speech,
        scenarios: trials.len(),
        samples: trials.iter().map(|trial| trial.samples).sum(),
        end_of_turn,
        backchannel,
        echo,
        end_of_turn_wired: cfg!(duplex_eot),
        backchannel_wired: cfg!(duplex_backchannel),
        echo_wired,
        eot_probe_took_floor,
        echo_probe_ran: echo_trials.len(),
        echo_probe_opened: echo_trials.iter().filter(|trial| trial.opened).count(),
        echo_probe_barged: echo_trials
            .iter()
            .filter(|trial| false_barged(trial))
            .count(),
        metrics,
        misses,
    }
}

fn backchannel_rate(trials: &[Trial], stage: StageState) -> Metric {
    if stage == StageState::Stub {
        let n = trials
            .iter()
            .filter(|trial| trial.kind == Kind::Listen)
            .count();
        return stub_metric("backchannel_rate_per_s", n);
    }
    let events = trials.iter().map(nod_events).sum::<usize>() as f64;
    let seconds = listen_seconds(trials);
    let rate = if seconds > 0.0 { events / seconds } else { 0.0 };
    let samples = vec![rate];
    rate_metric(
        "backchannel_rate_per_s",
        &samples,
        optional_max(BACKCHANNEL_RATE_P95_MAX, &samples),
    )
}

fn backchannel_offset(trials: &[Trial], stage: StageState) -> Metric {
    if stage == StageState::Stub {
        return stub_metric("backchannel_offset_ms", 0);
    }
    let offsets = nod_offsets(trials);
    latency_metric(
        "backchannel_offset_ms",
        &offsets,
        optional_max(BACKCHANNEL_OFFSET_P95_MAX, &offsets),
    )
}

fn false_barge(samples: &[f64], stage: StageState) -> Metric {
    if stage == StageState::Stub {
        return stub_metric("false_barge_in", samples.len());
    }
    rate_metric(
        "false_barge_in",
        samples,
        optional_max(FALSE_BARGE_P95_MAX, samples),
    )
}

fn stub_metric(name: &'static str, n: usize) -> Metric {
    Metric {
        name,
        n,
        p50: None,
        p95: None,
        digits: 3,
        gate: Gate::Off,
    }
}

fn resume_metric(samples: &[f64]) -> Metric {
    let gate = if samples.is_empty() {
        Gate::Fail {
            limit: "floor=1".to_string(),
        }
    } else if samples.iter().all(|sample| *sample == 1.0) {
        Gate::Pass {
            limit: "floor=1".to_string(),
        }
    } else {
        Gate::Fail {
            limit: "floor=1".to_string(),
        }
    };
    Metric {
        name: "interruption_resume",
        n: samples.len(),
        p50: percentile(samples, 50.0),
        p95: percentile(samples, 95.0),
        digits: 3,
        gate,
    }
}

fn rate_metric(name: &'static str, samples: &[f64], gate: Gate) -> Metric {
    Metric {
        name,
        n: samples.len(),
        p50: percentile(samples, 50.0),
        p95: percentile(samples, 95.0),
        digits: 3,
        gate,
    }
}

fn latency_metric(name: &'static str, samples: &[f64], gate: Gate) -> Metric {
    Metric {
        name,
        n: samples.len(),
        p50: percentile(samples, 50.0),
        p95: percentile(samples, 95.0),
        digits: 0,
        gate,
    }
}

fn band_gate(samples: &[f64], min_p50: f64, max_p95: f64) -> Gate {
    let Some(p50) = percentile(samples, 50.0) else {
        return Gate::Fail {
            limit: format!("p50>={min_p50:.0} p95<={max_p95:.0}"),
        };
    };
    let Some(p95) = percentile(samples, 95.0) else {
        return Gate::Fail {
            limit: format!("p50>={min_p50:.0} p95<={max_p95:.0}"),
        };
    };
    let limit = format!("p50>={min_p50:.0} p95<={max_p95:.0}");
    if p50 + 1.0e-6 >= min_p50 && p95 <= max_p95 + 1.0e-6 {
        Gate::Pass { limit }
    } else {
        Gate::Fail { limit }
    }
}

fn max_gate(samples: &[f64], max_p95: f64) -> Gate {
    let Some(p95) = percentile(samples, 95.0) else {
        return Gate::Fail {
            limit: format!("p95<={max_p95:.3}"),
        };
    };
    let limit = format!("p95<={max_p95:.3}");
    if p95 <= max_p95 + 1.0e-9 {
        Gate::Pass { limit }
    } else {
        Gate::Fail { limit }
    }
}

fn optional_max(limit: Option<f64>, samples: &[f64]) -> Gate {
    let Some(p95) = percentile(samples, 95.0) else {
        return Gate::Fail {
            limit: "unset".to_string(),
        };
    };
    match limit {
        Some(max) => {
            let text = format!("p95<={max:.3}");
            if p95 <= max + 1.0e-9 {
                Gate::Pass { limit: text }
            } else {
                Gate::Fail { limit: text }
            }
        }
        None => Gate::Fail {
            limit: format!("unset measured_p95={p95:.3}"),
        },
    }
}

fn values(trials: &[Trial], kind: Kind, bit: fn(&Trial) -> f64) -> Vec<f64> {
    trials
        .iter()
        .filter(|trial| trial.kind == kind)
        .map(bit)
        .collect()
}

fn values_echo(trials: &[Trial]) -> Vec<f64> {
    trials
        .iter()
        .filter(|trial| matches!(trial.kind, Kind::EchoTone | Kind::EchoNoise))
        .map(|trial| if false_barged(trial) { 1.0 } else { 0.0 })
        .collect()
}

fn optional(trials: &[Trial], kind: Kind, pick: fn(&Trial) -> Option<f64>) -> Vec<Option<f64>> {
    trials
        .iter()
        .filter(|trial| trial.kind == kind)
        .map(pick)
        .collect()
}

fn resume_bits(trials: &[Trial]) -> Vec<f64> {
    trials
        .iter()
        .filter(|trial| {
            matches!(
                trial.kind,
                Kind::Barge | Kind::Ack | Kind::ShortHold | Kind::Tap
            )
        })
        .map(|trial| if resume_ok(trial) { 1.0 } else { 0.0 })
        .collect()
}

fn resume_miss(trial: &Trial) -> Option<&'static str> {
    let scored = matches!(
        trial.kind,
        Kind::Barge | Kind::Ack | Kind::ShortHold | Kind::Tap
    );
    if scored && !resume_ok(trial) {
        Some(trial.id)
    } else {
        None
    }
}

fn resume_ok(trial: &Trial) -> bool {
    match trial.kind {
        Kind::Barge => yielded(trial),
        Kind::Ack => ack_held(trial),
        Kind::ShortHold => short_hold(trial),
        Kind::Tap => tap_held(trial),
        _ => true,
    }
}

fn took_floor(trial: &Trial) -> bool {
    trial
        .obs
        .iter()
        .any(|obs| obs.me && matches!(obs.phase, Phase::IntraPause | Phase::Continuation))
}

fn took_floor_bit(trial: &Trial) -> f64 {
    if took_floor(trial) {
        1.0
    } else {
        0.0
    }
}

fn turn_latency_ms(trial: &Trial) -> Option<f64> {
    let end = trial
        .obs
        .iter()
        .rev()
        .find(|obs| obs.phase == Phase::User && obs.you)?;
    let end_ms = end.t_ms.saturating_add(u64::from(FRAME_MS));
    let first = trial.obs.iter().find(|obs| obs.me)?;
    Some(first.t_ms.saturating_sub(end_ms) as f64)
}

fn stop_ms(trial: &Trial) -> Option<f64> {
    let onset = trial
        .obs
        .iter()
        .find(|obs| obs.phase == Phase::Barge && obs.you)?;
    let stop = trial
        .obs
        .iter()
        .find(|obs| obs.event == LoopEvent::BargeIn)?;
    Some(stop.t_ms.saturating_sub(onset.t_ms) as f64)
}

fn yielded(trial: &Trial) -> bool {
    let Some(pos) = trial
        .obs
        .iter()
        .position(|obs| obs.event == LoopEvent::BargeIn)
    else {
        return false;
    };
    trial.obs[pos..].iter().all(|obs| !obs.me)
}

fn ack_held(trial: &Trial) -> bool {
    let mut heard = false;
    let mut last_me = false;
    for obs in &trial.obs {
        if obs.phase != Phase::Listen {
            continue;
        }
        if obs.event == LoopEvent::BargeIn {
            return false;
        }
        heard = true;
        last_me = obs.me;
    }
    if !heard || !last_me {
        return false;
    }
    trial
        .obs
        .iter()
        .filter(|obs| obs.phase == Phase::Tail)
        .all(|obs| obs.event != LoopEvent::BargeIn)
}

fn short_hold(trial: &Trial) -> bool {
    let quiet = trial.obs.iter().all(|obs| {
        let window = matches!(obs.phase, Phase::IntraPause | Phase::Continuation);
        !window || !obs.me
    });
    let resumed = trial
        .obs
        .iter()
        .any(|obs| obs.phase == Phase::Continuation && obs.event == LoopEvent::TurnResumed);
    quiet && resumed
}

fn tap_held(trial: &Trial) -> bool {
    let no_barge = trial.obs.iter().all(|obs| obs.event != LoopEvent::BargeIn);
    let still = trial
        .obs
        .iter()
        .any(|obs| obs.phase == Phase::Tail && obs.me);
    no_barge && still
}

fn overlap_ratio(trial: &Trial) -> Option<f64> {
    if matches!(trial.kind, Kind::EchoTone | Kind::EchoNoise) {
        return None;
    }
    let me = trial.obs.iter().filter(|obs| obs.me).count();
    if me == 0 {
        return None;
    }
    let both = trial.obs.iter().filter(|obs| obs.me && obs.you).count();
    Some(both as f64 / me as f64)
}

fn false_barged(trial: &Trial) -> bool {
    trial
        .obs
        .iter()
        .any(|obs| obs.phase == Phase::EchoScore && obs.event == LoopEvent::BargeIn)
}

fn nod_events(trial: &Trial) -> usize {
    if matches!(trial.kind, Kind::EchoTone | Kind::EchoNoise) {
        return 0;
    }
    let mut events = 0usize;
    let mut run = 0u32;
    let mut open = false;
    for obs in &trial.obs {
        if nod_frame(obs) {
            run = run.saturating_add(1);
            open = true;
        } else if open {
            if (1..=12).contains(&run) {
                events = events.saturating_add(1);
            }
            run = 0;
            open = false;
        }
    }
    if open && (1..=12).contains(&run) {
        events = events.saturating_add(1);
    }
    events
}

fn nod_frame(obs: &crate::drive::Obs) -> bool {
    obs.me && obs.floor == Floor::Listening
}

fn nod_offsets(trials: &[Trial]) -> Vec<f64> {
    let mut offsets = Vec::new();
    for trial in trials {
        let mut last_user_end: Option<u64> = None;
        let mut run = 0u32;
        let mut run_start = 0u64;
        let mut open = false;
        for obs in &trial.obs {
            if obs.phase == Phase::User && obs.you {
                last_user_end = Some(obs.t_ms.saturating_add(u64::from(FRAME_MS)));
            }
            if nod_frame(obs) {
                if !open {
                    run_start = obs.t_ms;
                }
                run = run.saturating_add(1);
                open = true;
            } else if open {
                if (1..=12).contains(&run) {
                    if let Some(end) = last_user_end {
                        offsets.push(run_start.saturating_sub(end) as f64);
                    }
                }
                run = 0;
                open = false;
            }
        }
    }
    offsets
}

fn listen_seconds(trials: &[Trial]) -> f64 {
    let frames = trials
        .iter()
        .filter(|trial| trial.kind == Kind::Listen)
        .map(|trial| trial.obs.len())
        .sum::<usize>();
    frames as f64 * f64::from(FRAME_MS) / 1_000.0
}

fn format_metric(metric: &Metric) -> String {
    let p50 = show(metric.p50, metric.digits);
    let p95 = show(metric.p95, metric.digits);
    let limit = match &metric.gate {
        Gate::Off => String::new(),
        Gate::Pass { limit } | Gate::Fail { limit } => format!(" limit={limit}"),
    };
    format!(
        "metric {} n={} p50={} p95={} gate={}{}",
        metric.name,
        metric.n,
        p50,
        p95,
        metric.gate.word(),
        limit
    )
}

fn show(value: Option<f64>, digits: usize) -> String {
    match value {
        Some(number) if digits == 0 => format!("{number:.0}"),
        Some(number) => format!("{number:.3}"),
        None => "stub".to_string(),
    }
}

fn bit(flag: bool) -> &'static str {
    if flag {
        "true"
    } else {
        "false"
    }
}

fn pause_gate_is_on(metrics: &[Metric]) -> bool {
    metrics
        .iter()
        .find(|metric| metric.name == "pause_handling_tor")
        .is_some_and(|metric| !matches!(metric.gate, Gate::Off))
}

fn gate_word(stage: StageState, on: bool) -> &'static str {
    if stage == StageState::Stub || !on {
        "off"
    } else {
        "on"
    }
}
