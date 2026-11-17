// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Local Open Mind seam. A missing store or a missing weights file is
//! BLOCKED. Stored text is returned unchanged. Nothing here is a thought trace.
use galion_openmind::{
    Fallback, MemoryWitness, MissingPiece, PathState, ReplyFrom, SeamBlock, TurnSeam, REMEMBER_ENV,
    SEAM_ENV, STORE_ENV, WEIGHTS_ENV,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static N: AtomicU64 = AtomicU64::new(0);
static ENV: Mutex<()> = Mutex::new(());

fn scratch(name: &str) -> PathBuf {
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("galion-seam-{}-{}-{name}", std::process::id(), n));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn weights(dir: &Path) -> PathBuf {
    let p = dir.join("student.gq");
    fs::write(&p, b"not-a-model").unwrap();
    p
}

struct EnvGuard {
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    fn set(pairs: &[(&str, Option<&str>)]) -> Self {
        let saved = pairs
            .iter()
            .map(|(k, v)| {
                let prev = std::env::var(k).ok();
                match v {
                    Some(value) => std::env::set_var(k, value),
                    None => std::env::remove_var(k),
                }
                ((*k).to_string(), prev)
            })
            .collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, prev) in self.saved.drain(..) {
            match prev {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

#[test]
fn missing_store_and_weights_are_blocked_and_not_a_memory() {
    let seam = TurnSeam::new(None, None, true);
    let before = Path::new("/opt/galion/openmind").exists();
    let decision = seam.consult(1, false);
    assert!(!decision.hermes_called);
    assert!(decision.thought_trace().is_none());
    assert_eq!(
        decision.path,
        PathState::Blocked {
            missing: MissingPiece::StoreAndWeights
        }
    );
    assert_eq!(decision.memory, MemoryWitness::Blocked { missing: "store" });
    assert_eq!(decision.fallback, Fallback::Canned);
    assert_eq!(decision.reply_from, ReplyFrom::Lane);
    let token = decision.stage_token();
    assert!(token.contains("seam=blocked:store+weights"));
    assert!(token.contains("memory=blocked:store"));
    assert!(token.contains("fallback=canned"));
    assert!(!token.contains("remember"));
    assert_eq!(
        seam.note_turn(1, "user", "the code is 42"),
        Err(SeamBlock::MissingStore)
    );
    assert_eq!(Path::new("/opt/galion/openmind").exists(), before);
}

#[test]
fn hermes_selected_while_blocked_is_unreachable_and_still_not_called() {
    let seam = TurnSeam::new(None, None, true);
    let decision = seam.consult(4, true);
    assert!(!decision.hermes_called);
    assert!(decision.thought_trace().is_none());
    assert_eq!(decision.fallback, Fallback::Unreachable);
    assert_eq!(decision.reply_from, ReplyFrom::Lane);
    assert!(decision.stage_token().contains("fallback=unreachable"));
}

#[test]
fn weights_file_without_a_store_does_not_speak() {
    let dir = scratch("weights");
    let w = weights(&dir);
    let seam = TurnSeam::new(None, Some(w.clone()), true);
    assert!(seam.weights_present());
    assert!(!seam.store_present());
    let decision = seam.consult(2, false);
    assert_eq!(
        decision.path,
        PathState::Blocked {
            missing: MissingPiece::Store
        }
    );
    assert_eq!(decision.reply_from, ReplyFrom::Lane);
    assert!(decision.thought_trace().is_none());
    let body = fs::read(&w).unwrap();
    assert_eq!(body, b"not-a-model");
    match &decision.reply_from {
        ReplyFrom::Lane => {}
        ReplyFrom::Stored { text, .. } => panic!("weights bytes became speech: {text}"),
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn later_turn_reads_the_exact_stored_line() {
    let dir = scratch("mem");
    let store = dir.join("turns.jsonl");
    fs::write(&store, b"").unwrap();
    let w = weights(&dir);
    let seam = TurnSeam::new(Some(store.clone()), Some(w), true);
    assert!(matches!(seam.path_state(), PathState::Present { .. }));
    seam.note_turn(1, "user", "the code is 42").unwrap();
    let first = seam.consult(1, false);
    assert_eq!(first.memory, MemoryWitness::Empty);
    assert_eq!(first.reply_from, ReplyFrom::Lane);
    assert!(first.thought_trace().is_none());
    let later = seam.consult(2, false);
    assert_eq!(
        later.memory,
        MemoryWitness::Hit {
            turn: 1,
            text: "the code is 42".into(),
        }
    );
    assert_eq!(
        later.reply_from,
        ReplyFrom::Stored {
            turn: 1,
            text: "the code is 42".into(),
        }
    );
    assert!(!later.hermes_called);
    assert!(later.thought_trace().is_none());
    assert!(!later.stage_token().contains("the code is 42"));
    assert!(later.stage_token().contains("memory=hit:1"));
    let on_disk = fs::read_to_string(&store).unwrap();
    assert_eq!(on_disk, "v1\t1\tuser\tthe code is 42\n");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn store_without_weights_still_witnesses_and_stays_blocked() {
    let dir = scratch("store-only");
    let seam = TurnSeam::new(Some(dir.clone()), None, true);
    assert_eq!(
        seam.path_state(),
        PathState::Blocked {
            missing: MissingPiece::Weights
        }
    );
    seam.note_turn(7, "user", "desk lamp is on").unwrap();
    let later = seam.consult(8, false);
    assert_eq!(
        later.path,
        PathState::Blocked {
            missing: MissingPiece::Weights
        }
    );
    assert_eq!(
        later.reply_from,
        ReplyFrom::Stored {
            turn: 7,
            text: "desk lamp is on".into(),
        }
    );
    assert_eq!(later.fallback, Fallback::Canned);
    let file = dir.join("turns.jsonl");
    assert!(file.is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn remember_denied_does_not_write_or_read() {
    let dir = scratch("deny");
    let store = dir.join("turns.jsonl");
    fs::write(&store, b"v1\t1\tuser\talready here\n").unwrap();
    let seam = TurnSeam::new(Some(store.clone()), None, false);
    assert_eq!(
        seam.note_turn(2, "user", "new fact"),
        Err(SeamBlock::RememberDenied)
    );
    assert_eq!(fs::read(&store).unwrap(), b"v1\t1\tuser\talready here\n");
    let decision = seam.consult(3, false);
    assert_eq!(
        decision.memory,
        MemoryWitness::Blocked {
            missing: "remember"
        }
    );
    assert_eq!(decision.reply_from, ReplyFrom::Lane);
    assert!(decision.thought_trace().is_none());
    let token = decision.stage_token();
    assert!(!token.contains("already here"));
    assert!(!token.contains("new fact"));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn malformed_line_is_skipped_not_repaired() {
    let dir = scratch("bad");
    let store = dir.join("turns.jsonl");
    fs::write(
        &store,
        b"this is not a memory\nv1\t3\tuser\tkept line\n\nv1\tnope\tuser\tdropped\n",
    )
    .unwrap();
    let seam = TurnSeam::new(Some(store), None, true);
    let hit = seam.recall_previous(9).unwrap();
    assert_eq!(hit, Some((3, "kept line".into())));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn too_long_blank_and_bad_role_do_not_write() {
    let dir = scratch("refuse");
    let store = dir.join("turns.jsonl");
    fs::write(&store, b"").unwrap();
    let seam = TurnSeam::new(Some(store.clone()), None, true);
    let long = "x".repeat(2_001);
    assert_eq!(seam.note_turn(1, "user", &long), Err(SeamBlock::TooLong));
    assert_eq!(seam.note_turn(1, "user", "   "), Err(SeamBlock::EmptyText));
    assert_eq!(seam.note_turn(1, "system", "no"), Err(SeamBlock::BadRole));
    assert_eq!(fs::read(&store).unwrap(), b"");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn consult_does_not_create_a_store() {
    let dir = scratch("consult");
    let missing = dir.join("absent.jsonl");
    let seam = TurnSeam::new(Some(missing.clone()), None, true);
    assert!(!seam.store_present());
    let _ = seam.consult(1, false);
    assert!(!missing.exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn deploy_tree_is_not_created_or_written() {
    let opt = Path::new("/opt/galion/openmind");
    let file = opt.join("turns.jsonl");
    let dir_before = opt.exists();
    let bytes_before = fs::read(&file).ok();
    let seam = TurnSeam::new(Some(opt.to_path_buf()), None, true);
    let err = seam.note_turn(1, "user", "do not write this").unwrap_err();
    if dir_before {
        assert!(matches!(
            err,
            SeamBlock::RefusedPath | SeamBlock::MissingStore
        ));
    } else {
        assert_eq!(err, SeamBlock::MissingStore);
        assert!(!opt.exists(), "seam created /opt/galion/openmind");
    }
    assert_eq!(fs::read(&file).ok(), bytes_before);
    let as_file = TurnSeam::new(Some(file.clone()), None, true);
    if !file.exists() {
        assert_eq!(
            as_file.note_turn(1, "user", "do not write this"),
            Err(SeamBlock::MissingStore)
        );
        assert!(!file.exists());
    } else {
        assert_eq!(
            as_file.note_turn(2, "user", "do not write this"),
            Err(SeamBlock::RefusedPath)
        );
        assert_eq!(fs::read(&file).ok(), bytes_before);
    }
}

#[test]
fn newline_is_flattened_so_one_record_stays_one_line() {
    let dir = scratch("flat");
    let seam = TurnSeam::new(Some(dir.clone()), None, true);
    seam.note_turn(1, "user", "line one\nline two").unwrap();
    let hit = seam.recall_previous(2).unwrap();
    assert_eq!(hit, Some((1, "line one line two".into())));
    let raw = fs::read_to_string(dir.join("turns.jsonl")).unwrap();
    assert_eq!(raw.lines().count(), 1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn from_env_ignores_missing_paths_and_empty_weights() {
    let _lock = ENV.lock().unwrap_or_else(|p| p.into_inner());
    let dir = scratch("env");
    let missing_store = dir.join("no-such-store");
    let empty = dir.join("empty.gq");
    fs::write(&empty, b"").unwrap();
    let real = weights(&dir);
    let _env = EnvGuard::set(&[
        (STORE_ENV, Some(missing_store.to_str().unwrap())),
        (WEIGHTS_ENV, Some(empty.to_str().unwrap())),
        (REMEMBER_ENV, None),
        (SEAM_ENV, Some("1")),
    ]);
    let seam = TurnSeam::from_env();
    assert!(!seam.store_present());
    assert!(!seam.weights_present());
    assert!(!seam.remember_granted());
    assert!(galion_openmind::seam_enabled_from_env());
    let blocked = seam.consult(1, false);
    assert!(matches!(
        blocked.path,
        PathState::Blocked {
            missing: MissingPiece::StoreAndWeights
        }
    ));
    drop(_env);
    let _env = EnvGuard::set(&[
        (STORE_ENV, Some(dir.to_str().unwrap())),
        (WEIGHTS_ENV, Some(real.to_str().unwrap())),
        (REMEMBER_ENV, Some("yes")),
        (SEAM_ENV, None),
    ]);
    let seam = TurnSeam::from_env();
    assert!(seam.store_present() && seam.weights_present() && seam.remember_granted());
    assert!(!galion_openmind::seam_enabled_from_env());
    assert!(TurnSeam::weights_from_env());
    drop(_env);
    let _ = fs::remove_dir_all(&dir);
}
