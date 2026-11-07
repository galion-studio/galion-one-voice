// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! Public core-map helpers on small in-memory CPU lists.
//! Does not pin this process and does not touch a device.

use galion_cores::{
    active_map_with, format_cpu_list, parse_cpu_list, placement_with, CoreMap, CoreRole, RolePlacement, CORE_MAP_MIN_CPUS,
    CPU_SET_BITS,
};
use std::process::Command;

#[test]
fn cpu_lists_round_trip_inside_the_bitset() {
    for list in ["0", "0-2,6", "7", "0-7", "1023"] {
        let parsed = parse_cpu_list(list).unwrap();
        assert!(parsed.iter().all(|&c| c < CPU_SET_BITS));
        assert_eq!(parse_cpu_list(&format_cpu_list(&parsed)).as_deref(), Some(parsed.as_slice()));
    }
    assert_eq!(parse_cpu_list("0-2,6"), Some(vec![0, 1, 2, 6]));
    assert_eq!(format_cpu_list(&[0, 1, 2, 6]), "0-2,6");
    assert_eq!(parse_cpu_list(""), None);
    assert_eq!(parse_cpu_list("3-1"), None);
    assert_eq!(parse_cpu_list("x"), None);
    assert_eq!(parse_cpu_list("1024"), None);
    assert_eq!(parse_cpu_list(&CPU_SET_BITS.to_string()), None);
}

#[test]
fn every_role_has_a_small_in_memory_placement() {
    assert_eq!(CORE_MAP_MIN_CPUS, 8);
    for map in [CoreMap::V2, CoreMap::V3] {
        for role in CoreRole::ALL {
            let p = placement_with(role, map, None, None, 8);
            assert_eq!(p.role, role);
            assert_eq!(p.map, map);
            assert!(p.pinned());
            assert_eq!(p.cpus, role.cpus_in(map));
            assert_eq!(p.threads, p.cpus.len() as u32);
            assert!(p.cpus.iter().all(|&c| c < 8));
            assert_eq!(p.cpu_list(), format_cpu_list(&p.cpus));
        }
    }

    assert_eq!(CoreRole::Bus.v3_cpus(), vec![0]);
    assert_eq!(CoreRole::Retrieval.v3_cpus(), vec![0]);
    assert_eq!(CoreRole::Asr.v3_cpus(), vec![1]);
    assert_eq!(CoreRole::Reasoning.v3_cpus(), vec![2, 3, 4, 5]);
    assert_eq!(CoreRole::PiperSamantha.v3_cpus(), vec![6]);
    assert_eq!(CoreRole::PiperJarvis.v3_cpus(), vec![7]);
    assert_eq!(CoreRole::PiperSamantha.v2_cpus(), vec![6]);
    assert_eq!(CoreRole::PiperJarvis.v2_cpus(), vec![6]);
    assert_eq!(CoreRole::Retrieval.v2_cpus(), vec![7]);

    assert_eq!(CoreRole::parse("vosk"), Some(CoreRole::Asr));
    assert_eq!(CoreRole::parse("llm"), Some(CoreRole::Reasoning));
    assert_eq!(CoreRole::parse("jarvis"), Some(CoreRole::PiperJarvis));
    assert_eq!(CoreRole::parse("piper"), Some(CoreRole::PiperSamantha));
    assert_eq!(CoreRole::parse("nope"), None);
    assert_eq!(CoreRole::piper_for_voice("voices/jarvis/voice.onnx"), CoreRole::PiperJarvis);
    assert_eq!(CoreRole::piper_for_voice("voices/samantha/voice.onnx"), CoreRole::PiperSamantha);
}

#[test]
fn map_switch_and_overrides_stay_in_memory() {
    assert_eq!(active_map_with(Some("v3"), false, 4), CoreMap::Off);
    assert_eq!(active_map_with(Some("v3"), true, 8), CoreMap::V3);
    assert_eq!(active_map_with(Some("v2"), false, 8), CoreMap::V2);
    assert_eq!(active_map_with(Some("off"), true, 8), CoreMap::Off);
    assert_eq!(active_map_with(None, true, 8), CoreMap::V3);
    assert_eq!(active_map_with(None, false, 8), CoreMap::Off);
    assert_eq!(active_map_with(Some("auto"), true, 8), CoreMap::V3);

    let clamped = placement_with(CoreRole::PiperSamantha, CoreMap::V3, Some("5-7"), Some("9"), 8);
    assert_eq!((clamped.cpu_list(), clamped.threads), ("5-7".to_string(), 3));
    let ignored = placement_with(CoreRole::Reasoning, CoreMap::V3, Some("4-11"), None, 8);
    assert_eq!(ignored.cpus, vec![2, 3, 4, 5]);
    let off = placement_with(CoreRole::Bus, CoreMap::Off, Some("0"), Some("2"), 8);
    assert_eq!(off, RolePlacement { role: CoreRole::Bus, map: CoreMap::Off, cpus: Vec::new(), threads: 0 });
    assert!(!off.pinned());
    assert_eq!(off.cpu_list(), "");
}

fn pin_bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_galion-pin"));
    cmd.env("GALION_CORE_MAP", "off");
    cmd.env_remove("GALION_REASONING_LOCAL");
    cmd.env_remove("GALION_LLM_LOCAL");
    cmd.env_remove("GALION_CORES_BUS");
    cmd.env_remove("GALION_CORES_REASONING");
    cmd.env_remove("GALION_THREADS_BUS");
    cmd.env_remove("GALION_THREADS_REASONING");
    cmd.env_remove("GALION_PIN_DEFAULT_THREADS");
    cmd
}

#[test]
fn galion_pin_execs_unpinned_when_the_map_is_off() {
    let missing = pin_bin().output().unwrap();
    assert_eq!(missing.status.code(), Some(2));

    let bad = pin_bin().arg("not-a-role").arg("--").arg("true").output().unwrap();
    assert_eq!(bad.status.code(), Some(2));

    let hello = pin_bin().args(["bus", "--", "printf", "%s", "ok"]).output().unwrap();
    assert!(hello.status.success(), "stdout {:?} stderr {:?}", hello.stdout, hello.stderr);
    assert_eq!(hello.stdout, b"ok");
    let err = String::from_utf8(hello.stderr).unwrap();
    assert!(err.contains("role=bus"), "{err}");
    assert!(err.contains("map=off"), "{err}");
    assert!(err.contains("cpus=inherit"), "{err}");
    assert!(err.contains("threads=4"), "{err}");

    let threads = pin_bin().args(["reasoning", "--", "printf", "%s", "{threads}"]).output().unwrap();
    assert!(threads.status.success(), "{:?}", String::from_utf8_lossy(&threads.stderr));
    assert_eq!(threads.stdout, b"4");

    let mut custom = pin_bin();
    custom.env("GALION_PIN_DEFAULT_THREADS", "2");
    let custom = custom.args(["retrieval", "--", "printf", "%s", "{threads}"]).output().unwrap();
    assert!(custom.status.success(), "{:?}", String::from_utf8_lossy(&custom.stderr));
    assert_eq!(custom.stdout, b"2");
    let err = String::from_utf8(custom.stderr).unwrap();
    assert!(err.contains("role=retrieval"), "{err}");
    assert!(err.contains("threads=2"), "{err}");
}
