// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! After the scan is open, a query into the worker's hit buffer does not
//! allocate. The published cell still builds strings. That happens outside
//! this call.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};

use horbm::MEMORY_HIT_CAP;
use horbm_brain::{
    embed_diary_into, encode_query_into, DiaryArena, DiaryRow, DiaryScan, ScanHit, StoredKeys,
    StoredPack, DIARY_DIM,
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
        System.dealloc(ptr, layout);
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

fn opened() -> DiaryScan {
    let mut values = [0i8; DIARY_DIM];
    let embed = embed_diary_into("the blue bowl is in the cupboard", &mut values).unwrap();
    let diary = DiaryArena::from_rows([DiaryRow {
        item_id: "bowl",
        values: &values,
        scale: embed.scale,
        norm: embed.norm,
        allow: true,
    }])
    .unwrap();
    let mut lane = [0i8; 8];
    encode_query_into("bowl", &mut lane);
    let pack = StoredPack {
        k: 8,
        keys: lane.to_vec(),
        clause_id: vec![1],
        allow: vec![true],
        source: vec![1],
        item_id: vec!["bowl".to_string()],
        scale: vec![1],
    };
    let keys = StoredKeys::from_bytes(&pack.encode().unwrap()).unwrap();
    DiaryScan::open(keys, diary, true, MEMORY_HIT_CAP)
}

#[test]
fn a_query_does_not_allocate_after_setup() {
    let mut scan = opened();
    let mut hit_buf = [ScanHit::default(); MEMORY_HIT_CAP];
    assert_eq!(hit_buf.len(), MEMORY_HIT_CAP);

    ARMED.with(|armed| armed.set(true));
    let probe = Vec::<u8>::with_capacity(8);
    black_box(probe);
    ARMED.with(|armed| armed.set(false));
    assert!(
        ALLOCS.load(Ordering::SeqCst) > 0,
        "the counting allocator did not see a heap allocation"
    );

    let warm = scan.scan_selected_into("blue bowl", &mut hit_buf);
    assert!(warm > 0, "warmup produced no hit");
    ALLOCS.store(0, Ordering::SeqCst);
    ARMED.with(|armed| armed.set(true));
    for _ in 0..100 {
        for text in [
            "blue bowl",
            "where is the blue bowl",
            "",
            "zzznomatchtoken",
            "id-β",
        ] {
            let n = scan.scan_selected_into(text, &mut hit_buf);
            black_box(n);
            for hit in &hit_buf[..n] {
                black_box(scan.item_id(*hit));
                black_box((hit.pool, hit.row, hit.score.to_bits(), hit.native.to_bits()));
            }
        }
    }
    scan.set_remember(false);
    let denied = scan.scan_selected_into("blue bowl", &mut hit_buf);
    black_box(denied);
    scan.set_remember(true);
    let empty = scan.scan_selected_into("blue bowl", &mut []);
    black_box(empty);
    ARMED.with(|armed| armed.set(false));
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "a query allocated after setup"
    );
}
