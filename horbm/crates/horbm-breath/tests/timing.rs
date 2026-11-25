// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Wall time of the physiology tick. The 80 ms clock is the budget.

use horbm_breath::{measure_inhale, measure_ticks, FRAME_MS};

#[test]
fn the_tick_and_the_inhale_stay_under_one_frame() {
    let tick = measure_ticks();
    let inhale = measure_inhale();
    let budget_ns = u64::from(FRAME_MS) * 1_000_000;
    eprintln!(
        "breath tick p50_us={:.3} p99_us={:.3} p50_ns={} p99_ns={} batches={} batch={}",
        tick.p50_us(),
        tick.p99_us(),
        tick.p50_ns,
        tick.p99_ns,
        tick.batches,
        tick.batch
    );
    eprintln!(
        "breath inhale p50_us={:.3} p99_us={:.3} p50_ns={} p99_ns={} batches={} batch={}",
        inhale.p50_us(),
        inhale.p99_us(),
        inhale.p50_ns,
        inhale.p99_ns,
        inhale.batches,
        inhale.batch
    );
    assert!(tick.p50_ns > 0, "tick p50 was zero");
    assert!(tick.p50_ns <= tick.p99_ns);
    assert!(tick.p99_ns < budget_ns, "tick p99 {} ns", tick.p99_ns);
    assert!(inhale.p50_ns > 0, "inhale p50 was zero");
    assert!(inhale.p50_ns <= inhale.p99_ns);
    assert!(inhale.p99_ns < budget_ns, "inhale p99 {} ns", inhale.p99_ns);
}
