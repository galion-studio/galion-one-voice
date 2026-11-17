// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One feeling, updated one symphony tick at a time.
//!
//! The half-life keep, `0.5^(dt/half_life)`, is the same form as
//! `PersonaMood::tick` in `galion-emotion`. That function is not called.
//! It is a closed three-axis appraisal of a user, with persona gains.
//! This state is the speaker's own open vector.
//!
//! Order inside a tick:
//! 1. Blend the intent into the held goal (pins fill cited poles; an explicit
//!    axis replaces the pin on that axis; release sends the goal toward 0).
//! 2. Step energy and fatigue toward the frame, or hold them.
//! 3. Build the drive: `(1 - linger) * goal + linger * mood`, plus a coupling
//!    offset only when that coefficient is a number.
//! 4. Step the moment toward the drive (inertia).
//! 5. Step the mood toward the moment (the slow residue).

use std::collections::{BTreeMap, BTreeSet};

use crate::frame::IntentFrame;

use crate::config::{Config, Coupling};
use crate::feed::{assemble, AppliedPin, FeelingOut, LungsMoment, LungsRequest, Run, Snapshot};
use crate::inventory::{Inventory, Resolved, Track};
use crate::{FeelingError, CORE_AXES, DIRECTIONS, SCHEMA, TICK_MS};

#[derive(Clone, Debug)]
pub struct Feeling {
    cfg: Config,
    inv: Inventory,
    goal: BTreeMap<String, f64>,
    now: BTreeMap<String, f64>,
    mood: BTreeMap<String, f64>,
    energy: f64,
    fatigue: f64,
    environment: Option<String>,
    listener_distance_m: Option<f64>,
}

impl Feeling {
    pub fn rest(cfg: Config, inv: Inventory) -> Result<Self, FeelingError> {
        for name in cfg.pins.keys() {
            if !inv.is_category(name) {
                return Err(FeelingError(format!(
                    "pin {name} is not one of the {} inventory names",
                    inv.labels().len()
                )));
            }
        }
        let mut space = BTreeMap::new();
        for name in CORE_AXES {
            space.insert((*name).to_string(), 0.0);
        }
        Ok(Self {
            energy: cfg.energy_rest,
            fatigue: cfg.fatigue_rest,
            cfg,
            inv,
            goal: space.clone(),
            now: space.clone(),
            mood: space,
            environment: None,
            listener_distance_m: None,
        })
    }

    pub fn run(frames: &[IntentFrame], cfg: &Config, inv: &Inventory) -> Result<Run, FeelingError> {
        if frames.is_empty() {
            return Err(FeelingError("no intent frames".into()));
        }
        let mut feeling = Self::rest(cfg.clone(), inv.clone())?;
        let t0 = frames[0].t_ms;
        let mut outs = Vec::with_capacity(frames.len());
        let mut moments = Vec::with_capacity(frames.len());
        for frame in frames {
            let (out, moment) = feeling.step(frame, t0)?;
            outs.push(out);
            moments.push(moment);
        }
        let first = &moments[0];
        Ok(Run {
            schema: SCHEMA.to_string(),
            tick_ms: TICK_MS,
            contract: crate::frame::SCHEMA.to_string(),
            lungs: LungsRequest {
                category: first.category.clone(),
                arousal: first.arousal,
                energy: first.energy,
                fatigue: first.fatigue,
                trajectory: moments,
            },
            frames: outs,
            directions: DIRECTIONS.iter().map(|s| (*s).to_string()).collect(),
        })
    }

    fn step(
        &mut self,
        frame: &IntentFrame,
        t0_ms: u32,
    ) -> Result<(FeelingOut, LungsMoment), FeelingError> {
        if frame.t_ms % TICK_MS != 0 {
            return Err(FeelingError(format!(
                "t_ms {} is off the {} ms clock",
                frame.t_ms, TICK_MS
            )));
        }
        let blend = frame.blend.unwrap_or(self.cfg.default_blend);
        if !(0.0..=1.0).contains(&blend) {
            return Err(FeelingError("blend must be in [0, 1]".into()));
        }
        let (incoming, applied, mut open, track, citations, category_tie) = self.incoming(frame)?;
        self.mix_goal(&incoming, blend)?;
        let factor_half = self.cfg.factor_half_life_s;
        step_factor(factor_half, frame.energy, &mut self.energy, "energy")?;
        step_factor(factor_half, frame.fatigue, &mut self.fatigue, "fatigue")?;
        if let Some(env) = &frame.environment {
            self.environment = Some(env.clone());
        }
        if let Some(dist) = frame.listener_distance_m {
            if dist < 0.0 {
                return Err(FeelingError("listener_distance_m must be >= 0".into()));
            }
            self.listener_distance_m = Some(dist);
        }

        let mut drive_goal = self.goal.clone();
        let mut outer_applied = false;
        let mut inner_applied = false;
        // The last known distance stays in effect. A missing field is not a
        // reset. The shipped coefficient is unknown, so this adds nothing.
        if let Some(dist) = self.listener_distance_m {
            if push(&mut drive_goal, &self.cfg.distance_into, dist) {
                outer_applied = true;
            }
        }
        if push(
            &mut drive_goal,
            &self.cfg.energy_into,
            self.energy - self.cfg.energy_rest,
        ) {
            inner_applied = true;
        }
        if push(
            &mut drive_goal,
            &self.cfg.fatigue_into,
            self.fatigue - self.cfg.fatigue_rest,
        ) {
            inner_applied = true;
        }

        let dt = f64::from(TICK_MS) / 1000.0;
        let a_now = alpha(self.cfg.inertia_half_life_s, dt);
        let a_mood = alpha(self.cfg.mood_half_life_s, dt);
        let linger = self.cfg.linger;
        let names = self.axis_names();
        for name in &names {
            let goal = drive_goal.get(name).copied().unwrap_or(0.0);
            let mood = self.mood.get(name).copied().unwrap_or(0.0);
            let drive = clamp_axis((1.0 - linger) * goal + linger * mood);
            let cur = self.now.get(name).copied().unwrap_or(0.0);
            self.now
                .insert(name.clone(), clamp_axis(lerp(cur, drive, a_now)));
        }
        for name in &names {
            let target = self.now.get(name).copied().unwrap_or(0.0);
            let cur = self.mood.get(name).copied().unwrap_or(0.0);
            self.mood
                .insert(name.clone(), clamp_axis(lerp(cur, target, a_mood)));
        }
        open.sort();
        open.dedup();
        let snap = Snapshot {
            t_ms: frame.t_ms,
            t0_ms,
            track: track.as_str().to_string(),
            now: self.now.clone(),
            mood: self.mood.clone(),
            goal: self.goal.clone(),
            energy: self.energy,
            fatigue: self.fatigue,
            environment: self.environment.clone(),
            listener_distance_m: self.listener_distance_m,
            outer_applied,
            inner_applied,
            applied,
            open,
            citations,
            category_tie,
        };
        Ok(assemble(snap, &self.cfg.pins))
    }

    fn incoming(
        &self,
        frame: &IntentFrame,
    ) -> Result<
        (
            BTreeMap<String, f64>,
            Vec<AppliedPin>,
            Vec<String>,
            Track,
            Vec<String>,
            bool,
        ),
        FeelingError,
    > {
        if frame.release {
            let mut incoming = BTreeMap::new();
            for name in self.axis_names() {
                incoming.insert(name, 0.0);
            }
            let mut open = Vec::new();
            if !frame.axes.is_empty() || !frame.pins.is_empty() {
                open.push("release_ignores_targets".into());
            }
            let track = match &frame.track {
                Some(s) => Track::parse(s).map_err(FeelingError)?,
                None => Track::Prosody,
            };
            return Ok((incoming, Vec::new(), open, track, Vec::new(), false));
        }

        for ax in &frame.axes {
            check_axis(&ax.name, ax.value)?;
        }
        let mut num: BTreeMap<String, f64> = BTreeMap::new();
        let mut den: BTreeMap<String, f64> = BTreeMap::new();
        let mut applied = Vec::new();
        let mut open = Vec::new();
        let mut tracks = Vec::new();
        let mut cites = BTreeSet::new();
        for pin in &frame.pins {
            if pin.w == 0.0 {
                continue;
            }
            match self.inv.resolve(&pin.id).map_err(FeelingError)? {
                Resolved::Baseline { alias_from } => {
                    open.push(format!("baseline:{alias_from}"));
                }
                Resolved::Split {
                    alias_from,
                    to,
                    track,
                } => {
                    if let Some(t) = track {
                        tracks.push(t);
                    }
                    open.push(format!("split:{alias_from}->{}", to.join("+")));
                }
                Resolved::Unknown { id } => {
                    open.push(format!("not_a_pin:{id}"));
                }
                Resolved::One {
                    category,
                    track,
                    alias_from,
                } => {
                    if let Some(t) = track {
                        tracks.push(t);
                    }
                    let pole = self.cfg.pins.get(&category);
                    let placed = pole.map(|p| p.placed()).unwrap_or(false);
                    if let Some(pole) = pole {
                        cites.insert(pole.cite.clone());
                        for axis in CORE_AXES {
                            if let Some(sign) = pole.sign(axis) {
                                *num.entry((*axis).to_string()).or_insert(0.0) +=
                                    f64::from(sign) * pin.w;
                                *den.entry((*axis).to_string()).or_insert(0.0) += pin.w;
                            }
                        }
                    }
                    applied.push(AppliedPin {
                        category,
                        w: pin.w,
                        alias_from,
                        placed,
                    });
                }
            }
        }
        let mut incoming = BTreeMap::new();
        for (axis, n) in &num {
            let d = den[axis];
            if d > 0.0 {
                incoming.insert(axis.clone(), clamp_axis(n / d));
            }
        }
        for ax in &frame.axes {
            incoming.insert(ax.name.clone(), ax.value);
        }
        let track = if let Some(s) = &frame.track {
            Track::parse(s).map_err(FeelingError)?
        } else {
            tracks.sort();
            tracks.dedup();
            if tracks.len() > 1 {
                open.push("track_mixed".into());
                Track::Prosody
            } else {
                tracks.first().copied().unwrap_or(Track::Prosody)
            }
        };
        let category_tie = category_tie(&applied);
        if category_tie {
            open.push("category_tie".into());
        }
        Ok((
            incoming,
            applied,
            open,
            track,
            cites.into_iter().collect(),
            category_tie,
        ))
    }

    fn mix_goal(
        &mut self,
        incoming: &BTreeMap<String, f64>,
        blend: f64,
    ) -> Result<(), FeelingError> {
        for (name, new) in incoming {
            if !CORE_AXES.contains(&name.as_str()) && !self.goal.contains_key(name) {
                self.now.entry(name.clone()).or_insert(0.0);
                self.mood.entry(name.clone()).or_insert(0.0);
            }
            let prev = self.goal.get(name).copied().unwrap_or(0.0);
            self.goal
                .insert(name.clone(), clamp_axis(lerp(prev, *new, blend)));
        }
        if self.goal.len() > self.cfg.max_axes {
            return Err(FeelingError(format!(
                "axis count {} is above the cap {}",
                self.goal.len(),
                self.cfg.max_axes
            )));
        }
        Ok(())
    }

    fn axis_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for map in [&self.goal, &self.now, &self.mood] {
            names.extend(map.keys().cloned());
        }
        names.into_iter().collect()
    }
}

fn step_factor(
    half_life_s: f64,
    target: Option<f64>,
    slot: &mut f64,
    name: &str,
) -> Result<(), FeelingError> {
    let Some(target) = target else {
        return Ok(());
    };
    if !(0.0..=1.0).contains(&target) {
        return Err(FeelingError(format!("{name} must be in [0, 1]")));
    }
    let dt = f64::from(TICK_MS) / 1000.0;
    *slot = lerp(*slot, target, alpha(half_life_s, dt)).clamp(0.0, 1.0);
    Ok(())
}

fn category_tie(applied: &[AppliedPin]) -> bool {
    let mut best = -1.0;
    let mut winners = BTreeSet::new();
    for pin in applied {
        if pin.w > best + 1e-12 {
            best = pin.w;
            winners.clear();
            winners.insert(pin.category.clone());
        } else if (pin.w - best).abs() <= 1e-12 {
            winners.insert(pin.category.clone());
        }
    }
    winners.len() > 1
}

fn push(goal: &mut BTreeMap<String, f64>, coupling: &Coupling, amount: f64) -> bool {
    let Some(coef) = coupling.per_unit else {
        return false;
    };
    let prev = goal.get(&coupling.axis).copied().unwrap_or(0.0);
    goal.insert(coupling.axis.clone(), clamp_axis(prev + coef * amount));
    true
}

fn check_axis(name: &str, value: f64) -> Result<(), FeelingError> {
    if name.is_empty() {
        return Err(FeelingError("an axis name is empty".into()));
    }
    if !(-1.0..=1.0).contains(&value) {
        return Err(FeelingError(format!("axis {name} must be in [-1, 1]")));
    }
    Ok(())
}

/// `0.5^(dt/half_life)` kept, then the step is one minus that.
/// PersonaMood uses the keep on the way back to rest. Here the same keep is
/// the unmoved fraction of a chase toward a target.
pub fn alpha(half_life_s: f64, dt_s: f64) -> f64 {
    1.0 - 0.5_f64.powf(dt_s / half_life_s)
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
    from + (to - from) * t
}

fn clamp_axis(x: f64) -> f64 {
    x.clamp(-1.0, 1.0)
}
