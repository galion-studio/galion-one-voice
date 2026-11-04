// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.

use galion_symphony::eval::Status;
use galion_symphony::self_eval;

#[test]
fn infra_gates_pass_and_missing_inputs_are_blocked() {
    let report = self_eval();
    let fails: Vec<_> = report
        .gates
        .iter()
        .filter(|g| g.status == Status::Fail)
        .map(|g| format!("{}: {}", g.name, g.detail))
        .collect();
    assert!(fails.is_empty(), "{fails:?}\n{}", report.markdown());
    assert!(report.infra_pass());
    for name in [
        "gq-roundtrip",
        "clip-abi",
        "texture-keep",
        "clip-not-lane",
        "namespace",
        "foreign-refused",
        "clock",
        "heartbeat-pause",
        "presence",
        "empathy",
        "cpu-budgets",
        "pins-table",
        "output-clean",
    ] {
        let g = report
            .gates
            .iter()
            .find(|g| g.name == name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(g.status, Status::Pass, "{name}: {}", g.detail);
    }
    for name in [
        "landing-samantha-wav",
        "landing-jarvis-wav",
        "landing-clone-weights",
        "clone-generate",
    ] {
        let g = report.gates.iter().find(|g| g.name == name).unwrap();
        assert_ne!(g.status, Status::Fail, "{name}");
    }
}
