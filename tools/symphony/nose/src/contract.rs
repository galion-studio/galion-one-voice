// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! TODO(S1): local stub of `galion-mne-contract`.
//!
//! Agent S1 owns that crate. This module is the minimum this layer reads so
//! a plan can be built and tested before the crate exists. When it lands,
//! delete this file and depend on the crate. Do not grow the stub into a
//! second contract.

use serde::Deserialize;

use crate::baseline::Speaker;
use crate::bursts::BurstAsk;
use crate::feeling::{self, OpenFeeling};
use crate::plan::{self, NasalSpan, Request, TimedFeeling};
use crate::read::{self, Reading};

pub const SCHEMA: &str = "galion-mne-intent-stub/0";
pub const TODO: &str = "TODO(S1): replace contract.rs with the galion-mne-contract crate";

#[derive(Clone, Debug, Deserialize)]
pub struct Intent {
    pub schema: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub feeling: Option<serde_json::Value>,
    #[serde(default)]
    pub trajectory: Vec<TrajectoryIn>,
    #[serde(default)]
    pub nose: Vec<NasalSpan>,
    #[serde(default)]
    pub bursts: Vec<BurstAsk>,
    #[serde(default)]
    pub place_from_feeling: bool,
    #[serde(default)]
    pub speaker: Option<Speaker>,
    /// Optional PR 191 reading. Parsed, not planned, unless `adopt_reading`.
    #[serde(default)]
    pub reading: Option<serde_json::Value>,
    #[serde(default)]
    pub adopt_reading: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TrajectoryIn {
    pub at_s: f64,
    pub feeling: serde_json::Value,
}

impl Intent {
    pub fn parse(text: &str) -> Result<Self, String> {
        let intent: Intent = serde_json::from_str(text).map_err(|e| format!("intent: {e}"))?;
        if intent.schema != SCHEMA {
            return Err(format!(
                "intent schema is {}, want {SCHEMA}. {TODO}",
                intent.schema
            ));
        }
        Ok(intent)
    }

    pub fn to_request(&self) -> Result<(Request, Option<Reading>), String> {
        let feeling = match &self.feeling {
            Some(v) => feeling::from_value(v)?,
            None => OpenFeeling::neutral(),
        };
        let mut trajectory = Vec::new();
        for step in &self.trajectory {
            trajectory.push(TimedFeeling {
                at_s: step.at_s,
                feeling: feeling::from_value(&step.feeling)?,
            });
        }
        let reading = match &self.reading {
            Some(v) => Some(read::parse_reading(v)?),
            None => None,
        };
        let mut bursts = self.bursts.clone();
        if self.adopt_reading {
            if let Some(r) = &reading {
                for ev in &r.events {
                    if crate::gestures::canonical(&ev.gesture).is_none() {
                        continue;
                    }
                    bursts.push(BurstAsk {
                        gesture: ev.gesture.clone(),
                        at_s: Some(ev.start_s),
                        place: None,
                        category: ev.category.clone(),
                        mouth: None,
                        end_s: Some(ev.end_s),
                    });
                }
            }
        }
        let speaker = self.speaker.clone().unwrap_or_default();
        let mut req = Request::new(&self.text, feeling, speaker);
        req.trajectory = trajectory;
        req.nose = self.nose.clone();
        req.bursts = bursts;
        req.place_from_feeling = self.place_from_feeling;
        Ok((req, reading))
    }
}

/// Plan from the stub. The reading is returned beside the plan and is not
/// written into the plan's burst list unless `adopt_reading` was set.
pub fn plan_intent(
    intent: &Intent,
    cfg: &crate::config::Config,
    inventory: &crate::inventory::Inventory,
) -> Result<(plan::Plan, Option<Reading>), String> {
    let (req, reading) = intent.to_request()?;
    let planned = plan::plan(&req, cfg, inventory)?;
    Ok((planned, reading))
}
