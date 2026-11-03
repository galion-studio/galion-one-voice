// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Section A gate: the runtime path is a Galion clip, the tick does not allocate,
//! and a late frame stays inside the one-percent budget.

use galion_ml_voice::artifact::{self, ArtError, FramePull, FRAME_SAMPLES, TICK_BUDGET};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hint::black_box;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static ARMED: Cell<bool> = const { Cell::new(false) }; }

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.with(|a| a.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("galion-clip-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn wav_mono(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut o = Vec::new();
    o.extend_from_slice(b"RIFF");
    o.extend_from_slice(&(36 + data_len).to_le_bytes());
    o.extend_from_slice(b"WAVEfmt ");
    o.extend_from_slice(&16u32.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
    o.extend_from_slice(&24_000u32.to_le_bytes());
    o.extend_from_slice(&48_000u32.to_le_bytes());
    o.extend_from_slice(&2u16.to_le_bytes());
    o.extend_from_slice(&16u16.to_le_bytes());
    o.extend_from_slice(b"data");
    o.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        o.extend_from_slice(&s.to_le_bytes());
    }
    o
}

#[test]
fn cli_writes_the_galion_clip_and_refuses_chatterbox() {
    // Cargo 1.85 sets this from the binary filename, hyphens included.
    let bin = env!("CARGO_BIN_EXE_galion-voice-compile");
    let d = tmp("cli");
    let input = d.join("clone.wav");
    let samples: Vec<i16> = (0..400).map(|i| (i * 20) as i16).collect();
    std::fs::write(&input, wav_mono(&samples)).unwrap();
    let out = d.join("samantha.gq");
    let ok = Command::new(bin)
        .args(["--persona", "samantha", "--input"])
        .arg(&input)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "stderr {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let line = String::from_utf8_lossy(&ok.stdout);
    assert!(line.contains("compiled"), "{line}");
    assert!(line.contains("sha256="), "{line}");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(b"GALIONGQ"));
    assert!(!bytes.starts_with(b"RIFF"));
    let clip = artifact::open_runtime(&out).unwrap();
    assert_eq!(clip.persona, "samantha");
    assert_eq!(clip.sample_rate, 24_000);
    assert_eq!(clip.frame_ms, 80);

    let st = d.join("t3_nano_v1.safetensors");
    let json = br#"{"w":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
    let mut body = (json.len() as u64).to_le_bytes().to_vec();
    body.extend_from_slice(json);
    body.extend_from_slice(&[0, 0, 0, 0]);
    std::fs::write(&st, body).unwrap();
    let refused = Command::new(bin)
        .args(["--persona", "samantha", "--input"])
        .arg(&st)
        .arg("--out")
        .arg(d.join("nope.gq"))
        .output()
        .unwrap();
    assert_eq!(
        refused.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(!d.join("nope.gq").exists());
}

#[test]
fn fill_frame_allocates_nothing_and_meets_the_tick_budget() {
    let src: Vec<i16> = (0..(FRAME_SAMPLES * 3))
        .map(|i| (i as i16).wrapping_mul(3))
        .collect();
    let clip =
        artifact::parse(&artifact::compile_samples("jarvis", 24_000, 1, &src).unwrap()).unwrap();
    let mut out = [0f32; FRAME_SAMPLES];

    ARMED.with(|a| a.set(true));
    let probe = black_box(Box::new(1u64));
    ARMED.with(|a| a.set(false));
    drop(probe);
    assert!(
        ALLOCS.swap(0, Ordering::SeqCst) > 0,
        "counting allocator is live"
    );

    ARMED.with(|a| a.set(true));
    for i in 0..clip.n_frames {
        let pull = clip.fill_frame(i, false, &mut out);
        assert_eq!(pull, FramePull::Sound);
        black_box(out[0]);
    }
    assert_eq!(clip.fill_frame(0, true, &mut out), FramePull::Paused);
    assert_eq!(
        clip.fill_frame(clip.n_frames, false, &mut out),
        FramePull::Paused
    );
    ARMED.with(|a| a.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "fill_frame allocated");

    for _ in 0..32 {
        black_box(clip.fill_frame(0, false, &mut out));
    }
    let mut dts = Vec::with_capacity(2_000);
    for i in 0..2_000 {
        let late = i % 17 == 0;
        let start = Instant::now();
        let pull = clip.fill_frame((i as u32) % clip.n_frames, late, &mut out);
        black_box(pull);
        black_box(out[i % FRAME_SAMPLES]);
        dts.push(start.elapsed());
    }
    dts.sort();
    let p99 = dts[((dts.len() - 1) as f64 * 0.99).round() as usize];
    let max = *dts.last().unwrap();
    eprintln!(
        "section-a tick p99_ns={} max_ns={} budget_ns={}",
        p99.as_nanos(),
        max.as_nanos(),
        TICK_BUDGET.as_nanos()
    );
    assert!(p99 < TICK_BUDGET, "p99 {p99:?} exceeds {TICK_BUDGET:?}");
    assert!(
        max < TICK_BUDGET,
        "max {max:?} exceeds the 80 ms one-percent budget"
    );
}

#[test]
fn raw_chatterbox_pcm_is_not_opened_as_runtime() {
    let d = tmp("raw");
    let pcm = d.join("worker.s16le");
    std::fs::write(&pcm, 7i16.to_le_bytes()).unwrap();
    assert!(matches!(
        artifact::open_runtime(&pcm),
        Err(ArtError::Refused(_))
    ));
    let out = d.join("samantha.gq");
    artifact::compile_file(&pcm, &out, "samantha", 24_000, 1).unwrap();
    assert!(artifact::is_primary_runtime_path(&out));
    assert!(!artifact::is_primary_runtime_path(&pcm));
    let _ = artifact::open_runtime(&out).unwrap();
}
