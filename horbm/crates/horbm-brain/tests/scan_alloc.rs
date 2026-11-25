// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A query does not allocate after the scan is open.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm_brain::{
    DiaryArena, DiaryRow, DiaryScan, ScanHit, SimdLevel, StoredKeys, StoredPack, DIARY_DIM,
    SCAN_CAP,
};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|armed| armed.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn arena() -> DiaryScan {
    let n = 8usize;
    let k = 32usize;
    let mut keys = vec![0i8; n * k];
    for (i, lane) in keys.iter_mut().enumerate() {
        *lane = (i as i8).wrapping_add(1);
    }
    let pack = StoredPack {
        k,
        keys,
        clause_id: (0..n as u64).collect(),
        allow: (0..n).map(|i| i != 0).collect(),
        source: vec![1; n],
        item_id: (0..n).map(|i| format!("k{i}")).collect(),
        scale: vec![1, -3, 2, 0, 5, 1, -1, 4],
    };
    let stored = StoredKeys::from_bytes(&pack.encode().expect("keys")).expect("stored");
    let mut owned = Vec::new();
    for (id, text, allow) in [
        ("deny", "alpha token shared", false),
        ("near", "alpha token shared", true),
        ("far", "piper synthesizes a short clause", true),
        ("mid", "alpha token of a different claim", true),
    ] {
        let mut values = vec![0i8; DIARY_DIM];
        let embed = horbm_brain::embed_diary_into(text, &mut values).expect("embed");
        owned.push((id.to_string(), values, embed.scale, embed.norm, allow));
    }
    let diary =
        DiaryArena::from_rows(
            owned
                .iter()
                .map(|(id, values, scale, norm, allow)| DiaryRow {
                    item_id: id,
                    values,
                    scale: *scale,
                    norm: *norm,
                    allow: *allow,
                }),
        )
        .expect("diary");
    DiaryScan::open(stored, diary, true, SCAN_CAP)
}

#[test]
fn a_query_does_not_allocate_after_setup() {
    ARMED.with(|armed| armed.set(true));
    let probe = vec![1u8];
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counter must see a Vec"
    );
    drop(probe);

    let _ = DiaryScan::active_kernel();
    let mut scan = arena();
    let mut out = [ScanHit::default(); SCAN_CAP];
    let warm = scan.scan_selected_into("where is the kettle", &mut out);
    assert!(warm > 0);
    let scalar = {
        let n = scan.scan_scalar_into("where is the kettle", &mut out);
        out[..n].to_vec()
    };

    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for text in [
        "where is the kettle",
        "alpha token shared",
        "...",
        "",
        "id-β",
    ] {
        let n = scan.scan_selected_into(text, &mut out);
        black_box(n);
        for hit in &out[..n] {
            black_box(scan.item_id(*hit));
            black_box((hit.pool, hit.row, hit.score.to_bits(), hit.native.to_bits()));
        }
        let scalar_n = scan.scan_scalar_into(text, &mut out);
        black_box(scalar_n);
        let _ = scan.scan_at(SimdLevel::Avx2, text, &mut out);
        let _ = scan.scan_at(SimdLevel::Avx512, text, &mut out);
        let _ = scan.scan_at(SimdLevel::Neon, text, &mut out);
    }
    scan.set_remember(false);
    let denied = scan.scan_selected_into("alpha token shared", &mut out);
    black_box(denied);
    scan.set_remember(true);
    let empty = scan.scan_selected_into("alpha token shared", &mut []);
    black_box(empty);
    ARMED.with(|armed| armed.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "a query allocated");

    let mut check = [ScanHit::default(); SCAN_CAP];
    let n = scan.scan_scalar_into("where is the kettle", &mut check);
    assert_eq!(n, scalar.len());
    for (got, expect) in check[..n].iter().zip(&scalar) {
        assert_eq!(got.pool, expect.pool);
        assert_eq!(got.row, expect.row);
        assert_eq!(got.score.to_bits(), expect.score.to_bits());
        assert_eq!(got.native.to_bits(), expect.native.to_bits());
    }
}
