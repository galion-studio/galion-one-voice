// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 3 gate: a mapped fixed fixture, prefaulted at open, bit-exact
//! against the scalar int8 dot. No timings.

use galion_tensor::features::SimdLevel;
use galion_tensor::gq::{sha256, to_hex, Prefault};
use galion_tensor::simd::{dot_i8_level, dot_i8_scalar};
use horbm_keys::{
    encode_query_i8, fixed_fixture, shaped_pack, ArenaError, KeyArena, KeyPack, StringKeyArena,
    FIXTURE_SEED, HEADER_LEN, KEY_DIM, MAGIC,
};
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("horbm-keys-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn naive(keys: &[i8], k: usize, query: &[i8]) -> Vec<i32> {
    keys.chunks_exact(k)
        .map(|row| {
            let acc: i64 = row
                .iter()
                .zip(query)
                .map(|(&a, &b)| a as i64 * b as i64)
                .sum();
            assert!(acc >= i32::MIN as i64 && acc <= i32::MAX as i64);
            acc as i32
        })
        .collect()
}

fn open_pack(name: &str, pack: &KeyPack, pf: Prefault) -> (PathBuf, KeyArena) {
    let d = dir(name);
    let path = d.join("keys.hkb");
    pack.write(&path).unwrap();
    let arena = KeyArena::open(&path, pf).unwrap();
    (d, arena)
}

#[test]
fn fixed_fixture_simd_matches_scalar_after_prefault() {
    let (pack, query) = fixed_fixture();
    assert_eq!(pack.k, KEY_DIM);
    assert_eq!(pack.n(), 8);
    assert!(!pack.allow[0] && pack.allow[1]);
    let bytes = pack.encode().unwrap();
    assert_eq!(&bytes[0..4], &MAGIC);
    // Locked hash of the seed. A drift in the fixture fails here before the dots.
    assert_eq!(
        to_hex(&sha256(&bytes)),
        "a16465e5fc826fb14e00fc0e0590a5020d8081fabfac866b595bd656848e93ee"
    );

    let (d, arena) = open_pack("gate", &pack, Prefault::Populate);
    assert_eq!(arena.len(), 8);
    assert_eq!(arena.key_dim(), KEY_DIM);
    assert!(arena.keys_aligned_64());
    #[cfg(unix)]
    assert!(
        !arena.keys_copied(),
        "unix mmap of a 64-byte-aligned key region stays in the map"
    );
    assert_eq!(arena.clause_id(3).unwrap(), 0x1003);
    assert_eq!(arena.source(3).unwrap(), 0xA003);
    assert!(!arena.allows(0).unwrap());
    assert!(arena.allows(7).unwrap());

    let mut scalar = vec![0i32; arena.len()];
    let mut selected = vec![0i32; arena.len()];
    // The query is an unaligned subslice, the same case the tensor kernels already cover.
    let mut padded = vec![0i8; query.len() + 3];
    padded[1..1 + query.len()].copy_from_slice(&query);
    let query = &padded[1..1 + KEY_DIM];
    arena.dots_scalar_into(query, &mut scalar).unwrap();
    arena.dots_selected_into(query, &mut selected).unwrap();
    let oracle = naive(&pack.keys, KEY_DIM, query);
    assert_eq!(scalar, oracle);
    assert_eq!(selected, oracle);
    for level in [
        SimdLevel::Scalar,
        SimdLevel::Avx2,
        SimdLevel::Avx512,
        SimdLevel::Neon,
    ] {
        let mut got = vec![0i32; arena.len()];
        arena.dots_into(query, level, &mut got).unwrap();
        assert_eq!(
            got,
            oracle,
            "{level:?} kernel {}",
            dot_i8_level(level).name()
        );
        assert_eq!(dot_i8_scalar(&pack.keys[..KEY_DIM], query), oracle[0]);
    }

    let mut allowed = vec![0i32; 7];
    let n = arena
        .dots_allowed_into(query, SimdLevel::Scalar, &mut allowed)
        .unwrap();
    assert_eq!(n, 7);
    assert_eq!(allowed, oracle[1..]);
    let mut wide = vec![7i32; 8];
    let n = arena
        .dots_allowed_into(query, KeyArena::active_kernel(), &mut wide)
        .unwrap();
    assert_eq!(n, 7);
    assert_eq!(&wide[..7], &oracle[1..]);
    assert_eq!(wide[7], 7, "the spare slot is not written");

    for pf in [
        Prefault::None,
        Prefault::WillNeed,
        Prefault::Touch,
        Prefault::Populate,
    ] {
        let again = KeyArena::open(d.join("keys.hkb"), pf).unwrap();
        let mut got = vec![0i32; again.len()];
        again.dots_scalar_into(query, &mut got).unwrap();
        assert_eq!(got, oracle);
    }
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn other_widths_including_the_256_wide_key_stay_bit_exact() {
    for (n, k, seed) in [
        (0usize, KEY_DIM, 1u64),
        (1, 1, 2),
        (5, 7, 3),
        (4, 15, 4),
        (3, 33, 5),
        (4, 256, 9),
        (2, 65, 11),
    ] {
        let (pack, query) = shaped_pack(n, k, seed);
        let (_d, arena) = open_pack(&format!("w-{k}-{n}"), &pack, Prefault::Touch);
        assert!(arena.keys_aligned_64());
        let mut scalar = vec![0i32; n];
        let mut simd = vec![0i32; n];
        arena.dots_scalar_into(&query, &mut scalar).unwrap();
        arena.dots_selected_into(&query, &mut simd).unwrap();
        assert_eq!(simd, scalar);
        assert_eq!(scalar, naive(&pack.keys, k, &query));
        if n > 0 {
            let mut one = vec![0i32; n];
            let written = arena
                .dots_allowed_into(&query, SimdLevel::Avx512, &mut one)
                .unwrap();
            assert_eq!(written, n);
            assert_eq!(one, scalar);
        }
    }
}

#[test]
fn kernel_is_the_galion_tensor_detect() {
    let (pack, _) = shaped_pack(0, KEY_DIM, FIXTURE_SEED);
    let (_d, _arena) = open_pack("kern", &pack, Prefault::None);
    let used = KeyArena::active_kernel();
    let selected = galion_tensor::features::selected();
    assert!(used <= selected);
    assert!(used.compiled() && galion_tensor::features::cpu_supports(used));
    if !cfg!(any(
        feature = "simd-avx2",
        feature = "simd-avx512",
        feature = "simd-neon"
    )) {
        assert_eq!(used, SimdLevel::Scalar);
    }
    #[cfg(all(feature = "simd-avx2", any(target_arch = "x86", target_arch = "x86_64")))]
    if galion_tensor::features::cpu_supports(SimdLevel::Avx2) {
        assert!(
            used == SimdLevel::Avx2 || used == SimdLevel::Avx512,
            "compiled AVX on an AVX2 CPU must leave the scalar oracle, got {used:?}"
        );
    }
}

#[test]
fn rejects_a_short_file_a_bad_magic_and_a_bad_query() {
    let d = dir("bad");
    let path = d.join("keys.hkb");
    std::fs::write(&path, b"nope").unwrap();
    assert!(matches!(
        KeyArena::open(&path, Prefault::None),
        Err(ArenaError::Truncated { .. })
    ));
    let (pack, query) = shaped_pack(3, 7, 99);
    let mut bytes = pack.encode().unwrap();
    bytes[0] = b'X';
    std::fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        KeyArena::open(&path, Prefault::Populate),
        Err(ArenaError::BadMagic)
    ));
    bytes[0] = MAGIC[0];
    bytes[4] = 9;
    bytes[5] = 0;
    std::fs::write(&path, &bytes).unwrap();
    assert!(matches!(
        KeyArena::open(&path, Prefault::None),
        Err(ArenaError::UnsupportedVersion(9))
    ));
    let bytes = pack.encode().unwrap();
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    assert!(matches!(
        KeyArena::open(&path, Prefault::Touch),
        Err(ArenaError::BadLayout)
    ));
    // Unused high bits in the last consent byte (3 rows -> 5 spare bits).
    let mut dirty = bytes.clone();
    let lay = horbm_keys::layout(3, 7).unwrap();
    dirty[lay.consent_off] |= 0b1000_0000;
    std::fs::write(&path, &dirty).unwrap();
    assert!(matches!(
        KeyArena::open(&path, Prefault::None),
        Err(ArenaError::BadConsent)
    ));

    let (_d, arena) = open_pack("q", &pack, Prefault::WillNeed);
    let mut out = vec![0i32; 3];
    assert!(matches!(
        arena.dots_scalar_into(&query[..query.len() - 1], &mut out),
        Err(ArenaError::QueryLen { .. })
    ));
    assert!(matches!(
        arena.dots_into(&query, SimdLevel::Scalar, &mut [0i32; 2]),
        Err(ArenaError::OutLen { .. })
    ));
    let mut small = [0i32; 1];
    assert!(matches!(
        arena.dots_allowed_into(&query, SimdLevel::Scalar, &mut small),
        Err(ArenaError::OutLen { .. })
    ));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn all_denied_writes_no_score() {
    let (mut pack, query) = shaped_pack(4, KEY_DIM, 42);
    pack.allow.fill(false);
    let (_d, arena) = open_pack("deny", &pack, Prefault::Populate);
    let mut out = [9i32; 4];
    let n = arena
        .dots_allowed_into(&query, SimdLevel::Scalar, &mut out)
        .unwrap();
    assert_eq!(n, 0);
    assert_eq!(out, [9i32; 4]);
    let mut every = [0i32; 4];
    arena.dots_scalar_into(&query, &mut every).unwrap();
    assert_eq!(every, naive(&pack.keys, KEY_DIM, &query).as_slice());
    assert_eq!(
        arena.clause_id(4).unwrap_err(),
        ArenaError::LenMismatch {
            field: "row",
            got: 4,
            expect: 4
        }
    );
}

#[test]
fn empty_header_is_a_valid_arena() {
    let pack = KeyPack {
        k: KEY_DIM,
        keys: vec![],
        clause_id: vec![],
        allow: vec![],
        source: vec![],
    };
    let bytes = pack.encode().unwrap();
    assert_eq!(bytes.len(), HEADER_LEN);
    let (d, arena) = open_pack("empty", &pack, Prefault::Populate);
    assert!(arena.is_empty());
    assert!(arena.keys_aligned_64());
    let mut out = [];
    arena.dots_selected_into(&[0i8; KEY_DIM], &mut out).unwrap();
    std::fs::remove_dir_all(d).unwrap();
}

/// Phase integration gate. The string scorer's i32 is the scalar dot of the
/// encoded query, and the selected kernel matches that scalar. A denied row
/// and an unknown id are `None`.
#[test]
fn string_item_scorer_is_bit_exact_with_scalar_on_the_fixture() {
    let (pack, _) = fixed_fixture();
    let ids: Vec<String> = (0..pack.n()).map(|i| format!("item-{i}")).collect();
    let (d, arena) = open_pack("score", &pack, Prefault::Populate);
    let keyed = StringKeyArena::bind(arena, ids.iter().map(String::as_str)).unwrap();
    let text = "fixture-query";
    let encoded = encode_query_i8(text, pack.k);
    let oracle = naive(&pack.keys, pack.k, &encoded);
    for (i, expected) in oracle.iter().enumerate() {
        let id = format!("item-{i}");
        let scalar = keyed.score_scalar_text(&id, text);
        let selected = keyed.score_selected_text(&id, text);
        if pack.allow[i] {
            assert_eq!(scalar, Some(*expected), "row {i}");
            assert_eq!(selected, scalar, "row {i}");
        } else {
            assert_eq!(scalar, None, "denied row {i} is not a score");
            assert_eq!(selected, None);
        }
    }
    assert_eq!(keyed.score_scalar_text("missing-item", text), None);
    assert_eq!(keyed.score_selected_text("missing-item", text), None);
    let short = ids[..ids.len() - 1].iter().map(String::as_str);
    let (short_dir, again) = open_pack("score-short", &pack, Prefault::None);
    match StringKeyArena::bind(again, short) {
        Err(ArenaError::LenMismatch {
            field: "item_id", ..
        }) => {}
        Err(err) => panic!("short id list: {err}"),
        Ok(_) => panic!("short id list was accepted"),
    }
    std::fs::remove_dir_all(short_dir).unwrap();
    std::fs::remove_dir_all(d).unwrap();
}
