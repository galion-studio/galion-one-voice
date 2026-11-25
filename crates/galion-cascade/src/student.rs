// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! S0 student brain probe. CPU file read through [`galion_tensor::GqFile`].
//!
//! No download, no network, no third-party teacher, no weight write.
//! This build has no token generator. Opening a `.gq` is not speaking.
//! A content turn that used to restate the caller via [`crate::reflect::reply`]
//! now says the student is unavailable. The spoken line never copies the
//! caller's sentence and is not labelled `galion-reflect` or `student-gq`.
//!
//! Path: `GALION_STUDENT_GQ` when set, otherwise `weights/student/s0.gq`
//! (gitignored with the rest of `weights/`).

use std::path::{Path, PathBuf};

use galion_tensor::{GqFile, Prefault};

use crate::{CascadeError, Emotion, Llm, Transcript};

/// Label on `/v2/say` `"llm"` when a verified S0 file can be spoken.
/// This build never emits it.
pub const LABEL_READY: &str = "galion-student-s0";
/// Label on `/v2/say` `"llm"` while the student cannot speak.
pub const LABEL_BLOCKED: &str = "galion-student-blocked";
/// Hard-loop source once a generator actually produces the turn.
/// The probe does not return this. A file on disk is not this source.
pub const SOURCE_READY: &str = "student-gq";
/// Hard-loop source for the unavailable line.
pub const SOURCE_BLOCKED: &str = "student-blocked";

/// There is no S0 decode/speak step in this crate. Leave this false.
/// A future generator replaces the not-runnable return in [`probe_path`].
/// It must not call [`crate::reflect::reply`].
pub const INFERENCE_IMPLEMENTED: bool = false;

pub const LINE_ABSENT: &str = "Student brain unavailable. The S0 student .gq is not on this host.";
pub const LINE_BAD: &str = "Student brain unavailable. The S0 student .gq did not open.";
pub const LINE_NOT_RUNNABLE: &str =
    "Student brain unavailable. The S0 .gq opened and cannot speak.";

/// Default weight path, relative to the process working directory.
pub fn default_weight_path() -> PathBuf {
    PathBuf::from("weights/student/s0.gq")
}

/// `GALION_STUDENT_GQ` or [`default_weight_path`]. Does not create the file.
pub fn weight_path_from_env() -> PathBuf {
    match std::env::var("GALION_STUDENT_GQ") {
        Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw.trim()),
        _ => default_weight_path(),
    }
}

/// Persona-floor opt-in. Same env values that used to select `galion-reflect`,
/// plus `student`. The phone lane does not use this function. Its env default
/// is the loopback Hermes gateway; `GALION_LLM=canned` is the rule table.
pub fn selected_from_env() -> bool {
    matches!(
        std::env::var("GALION_LLM")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("canned") | Some("inhouse") | Some("in-house") | Some("student") | Some("reflect")
    )
}

/// Result of one local probe. `ready` is true only when a generator exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub ready: bool,
    pub label: &'static str,
    pub source: &'static str,
    pub reason: &'static str,
    pub detail: String,
    pub path: String,
    pub tensors: usize,
    pub line: &'static str,
}

fn blocked(
    path: &Path,
    reason: &'static str,
    detail: String,
    line: &'static str,
    tensors: usize,
) -> Probe {
    Probe {
        ready: false,
        label: LABEL_BLOCKED,
        source: SOURCE_BLOCKED,
        reason,
        detail,
        path: path.display().to_string(),
        tensors,
        line,
    }
}

/// Open `path` with `GqFile` only when it is a file. Missing and empty files
/// stay unloaded. A file that parses is still not spoken.
pub fn probe_path(path: &Path) -> Probe {
    if !path.exists() {
        return blocked(
            path,
            "weights-absent",
            "weights-absent".to_string(),
            LINE_ABSENT,
            0,
        );
    }
    if !path.is_file() {
        return blocked(path, "not-a-file", "not-a-file".to_string(), LINE_BAD, 0);
    }
    let file = match GqFile::open(path, Prefault::None) {
        Ok(file) => file,
        Err(err) => {
            return blocked(path, "bad-gq", format!("bad-gq: {err}"), LINE_BAD, 0);
        }
    };
    if let Err(err) = file.verify_all() {
        return blocked(
            path,
            "bad-gq",
            format!("checksum: {err}"),
            LINE_BAD,
            file.len(),
        );
    }
    let tensors = file.len();
    // Parsed and checksummed. This build still cannot decode or speak.
    let _ = INFERENCE_IMPLEMENTED;
    blocked(
        path,
        "not-runnable",
        format!("inference-not-implemented tensors={tensors}"),
        LINE_NOT_RUNNABLE,
        tensors,
    )
}

/// Probe [`weight_path_from_env`].
pub fn probe_env() -> Probe {
    probe_path(&weight_path_from_env())
}

/// Spoken answer for one content turn at `path`. `user` is the caller text.
/// It is not copied into the line. There is no model answer in this build.
pub fn answer_at(path: &Path, user: &str) -> (Probe, String) {
    let _ = user;
    let probe = probe_path(path);
    let line = probe.line.to_string();
    (probe, line)
}

/// [`answer_at`] for [`weight_path_from_env`].
pub fn answer(user: &str) -> (Probe, String) {
    answer_at(&weight_path_from_env(), user)
}

/// Phone opt-in (`GALION_LLM=student`). Not the phone default.
#[derive(Debug, Default, Clone)]
pub struct StudentLlm;

impl Llm for StudentLlm {
    fn name(&self) -> &'static str {
        LABEL_BLOCKED
    }

    fn reply(&mut self, t: &Transcript, _e: Emotion) -> Result<String, CascadeError> {
        if t.text.trim().is_empty() {
            return Err(CascadeError::Empty);
        }
        Ok(answer(&t.text).1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use galion_tensor::gq::{self, GqDType, HEADER_LEN, MAGIC, VERSION};

    fn lock_off(cond: bool) -> bool {
        cond
    }

    fn reflect_of(user: &str) -> String {
        crate::reflect::reply("samantha", user)
    }

    /// Minimal `.gq` v1, one f32 scalar. Test fixture only. Not a student.
    fn synthetic_gq() -> Vec<u8> {
        let name = "s0.probe";
        let bytes = 1.0f32.to_le_bytes();
        let align: u32 = 64;
        let mut table = Vec::new();
        table.extend_from_slice(&(name.len() as u16).to_le_bytes());
        table.extend_from_slice(name.as_bytes());
        table.push(GqDType::F32 as u8);
        table.push(1u8);
        table.extend_from_slice(&0u16.to_le_bytes());
        table.extend_from_slice(&1u64.to_le_bytes());
        table.extend_from_slice(&0.0f32.to_le_bytes());
        table.extend_from_slice(&0u64.to_le_bytes());
        table.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        table.extend_from_slice(&gq::sha256(&bytes));
        let table_end = HEADER_LEN as u64 + table.len() as u64;
        let data_off = table_end.div_ceil(align as u64) * align as u64;
        let mut file = Vec::new();
        file.extend_from_slice(&MAGIC);
        file.extend_from_slice(&VERSION.to_le_bytes());
        file.extend_from_slice(&0u16.to_le_bytes());
        file.extend_from_slice(&align.to_le_bytes());
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&(HEADER_LEN as u64).to_le_bytes());
        file.extend_from_slice(&(table.len() as u64).to_le_bytes());
        file.extend_from_slice(&data_off.to_le_bytes());
        file.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        file.extend_from_slice(&[0u8; 8]);
        assert_eq!(file.len(), HEADER_LEN);
        file.extend_from_slice(&table);
        file.resize(data_off as usize, 0);
        file.extend_from_slice(&bytes);
        file
    }

    #[test]
    fn missing_file_is_blocked_and_does_not_reflect() {
        let path = std::env::temp_dir().join(format!(
            "galion-s0-missing-{}-{}.gq",
            std::process::id(),
            "no-such"
        ));
        let _ = std::fs::remove_file(&path);
        let user = "My mother died this morning.";
        let probe = probe_path(&path);
        assert!(!probe.ready);
        assert_eq!(probe.reason, "weights-absent");
        assert_eq!(probe.label, LABEL_BLOCKED);
        assert_eq!(probe.source, SOURCE_BLOCKED);
        assert_ne!(probe.label, "galion-reflect");
        assert_ne!(probe.source, SOURCE_READY);
        assert_eq!(probe.line, LINE_ABSENT);
        assert_eq!(probe.tensors, 0);
        assert!(!probe.line.contains("mother"));
        assert_ne!(probe.line, reflect_of(user));
        assert!(lock_off(!INFERENCE_IMPLEMENTED));
        assert_ne!(LABEL_READY, LABEL_BLOCKED);
    }

    #[test]
    fn empty_file_and_bad_magic_do_not_count_as_loaded() {
        let dir = std::env::temp_dir().join(format!("galion-s0-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let empty = dir.join("empty.gq");
        let junk = dir.join("junk.gq");
        std::fs::write(&empty, b"").unwrap();
        std::fs::write(&junk, b"not-a-galion-container").unwrap();
        for path in [&empty, &junk] {
            let probe = probe_path(path);
            assert!(!probe.ready, "{probe:?}");
            assert_eq!(probe.reason, "bad-gq");
            assert_eq!(probe.line, LINE_BAD);
            assert_eq!(probe.label, LABEL_BLOCKED);
            assert_eq!(probe.tensors, 0);
        }
        let as_dir = dir.join("not-a-file.gq");
        std::fs::create_dir_all(&as_dir).unwrap();
        let probe = probe_path(&as_dir);
        assert_eq!(probe.reason, "not-a-file");
        assert!(!probe.ready);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn opened_gq_cannot_speak_and_does_not_reflect() {
        let dir = std::env::temp_dir().join(format!("galion-s0-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s0.gq");
        std::fs::write(&path, synthetic_gq()).unwrap();
        let user = "My mother died this morning.";
        let probe = probe_path(&path);
        assert!(!probe.ready);
        assert_eq!(probe.reason, "not-runnable");
        assert_eq!(probe.tensors, 1);
        assert_eq!(probe.line, LINE_NOT_RUNNABLE);
        assert!(probe.detail.contains("inference-not-implemented"));
        assert_eq!(probe.label, LABEL_BLOCKED);
        assert_ne!(probe.source, "student-gq");
        let (again, line) = answer_at(&path, user);
        assert_eq!(again.line, LINE_NOT_RUNNABLE);
        assert_eq!(line, LINE_NOT_RUNNABLE);
        assert!(!line.contains("mother"));
        assert!(!line.contains("you said"));
        assert_ne!(line, reflect_of(user));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn student_llm_name_is_blocked_and_the_line_does_not_echo() {
        let mut llm = StudentLlm;
        let user = "My mother died this morning.";
        let spoken = llm
            .reply(
                &Transcript {
                    text: user.into(),
                    lang: crate::Lang::En,
                },
                Emotion::Sad,
            )
            .unwrap();
        assert!(spoken.starts_with("Student brain unavailable."));
        assert!(!spoken.contains("mother"));
        assert!(!spoken.contains("you said"));
        assert_ne!(spoken, reflect_of(user));
        assert_eq!(llm.name(), LABEL_BLOCKED);
        assert!(matches!(
            llm.reply(
                &Transcript {
                    text: "   ".into(),
                    lang: crate::Lang::En,
                },
                Emotion::Neutral,
            ),
            Err(CascadeError::Empty)
        ));
    }
}
