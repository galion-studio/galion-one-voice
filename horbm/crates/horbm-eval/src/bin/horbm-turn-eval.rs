// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Print the Samantha turn-taking report. The clock is virtual.
//!
//! Exit 0 when every live gate holds. A stub stage is not a failure.

fn main() {
    let report = horbm_eval::run_stand_in();
    println!("{}", report.text());
    if !report.passed() {
        std::process::exit(1);
    }
}
