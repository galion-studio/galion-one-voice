// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The object the voice pipeline can merge.
//!
//! Draft PR 194 leaves `nose` as a stub section (`status`, `source`,
//! `features`, `todo`). `nose` here uses those four fields. Ticks and the
//! burst list sit beside it so a stub deserializer that ignores unknown
//! keys can still read the section.

use serde_json::{json, Value};

use crate::plan::Plan;

pub fn profile_patch(plan: &Plan) -> Value {
    let n = plan.ticks.len().max(1) as f64;
    let velum = plan.ticks.iter().map(|t| t.velum_open).sum::<f64>() / n;
    let murmur = plan.ticks.iter().map(|t| t.murmur).sum::<f64>() / n;
    let a1 = plan.ticks.iter().map(|t| t.a1_p0_db_config).sum::<f64>() / n;
    json!({
        "schema": plan.schema,
        "nose": {
            "status": "planned",
            "source": "galion-nose",
            "features": {
                "velum_open_mean": velum,
                "murmur_mean": murmur,
                "a1_p0_db_config_mean": a1,
            },
            "todo": plan.gaps.join(" "),
        },
        "nose_ticks": plan.ticks,
        "bursts": {
            "status": if plan.bursts.is_empty() { "empty" } else { "planned" },
            "track": "burst",
            "events": plan.bursts,
        },
    })
}
