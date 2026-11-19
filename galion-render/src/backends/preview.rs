//! Pre-rendered Galion One voice preview.
//!
//! `GALION_PREVIEW_AUDIO_DIR` is a directory outside this repo. The web surface
//! serves only these names from it:
//!
//! * `voice-preview.opus` as `audio/ogg; codecs=opus`
//! * `voice-preview.mp3` as `audio/mpeg`
//! * `voice-preview.wav` as `audio/wav`
//! * `voice-preview.json` as `application/json; charset=utf-8`
//!
//! A missing directory or file is a 404 with `Cache-Control: no-store`. The page
//! then shows text captions and does not call the browser speech synthesizer.
//! Nothing here synthesizes speech or reads a voice model.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const ENV_DIR: &str = "GALION_PREVIEW_AUDIO_DIR";
/// A 20 s 48 kHz stereo 16-bit WAV is about 3.8 MiB. Larger than this is refused.
pub const MAX_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
pub const CACHE_CONTROL: &str = "public, max-age=3600";

struct Kind {
    file: &'static str,
    content_type: &'static str,
}

pub struct Reply {
    pub status: &'static str,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub content_length: usize,
    pub cache_control: &'static str,
    pub extra: String,
}

/// `None` when `path` is not one of the four preview names. The caller then
/// continues with the normal routes.
pub fn serve(
    dir: Option<&Path>,
    path: &str,
    head: bool,
    range: Option<&str>,
    if_none_match: Option<&str>,
) -> Option<Reply> {
    let kind = kind_for(path)?;
    Some(read_kind(dir, kind, head, range, if_none_match))
}

pub fn dir_from_raw(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn dir_from_env() -> Option<PathBuf> {
    std::env::var(ENV_DIR)
        .ok()
        .and_then(|raw| dir_from_raw(&raw))
}

fn kind_for(path: &str) -> Option<Kind> {
    let (file, content_type) = match path {
        "/preview/voice-preview.opus" => ("voice-preview.opus", "audio/ogg; codecs=opus"),
        "/preview/voice-preview.mp3" => ("voice-preview.mp3", "audio/mpeg"),
        "/preview/voice-preview.wav" => ("voice-preview.wav", "audio/wav"),
        "/preview/voice-preview.json" => ("voice-preview.json", "application/json; charset=utf-8"),
        _ => return None,
    };
    Some(Kind { file, content_type })
}

fn read_kind(
    dir: Option<&Path>,
    kind: Kind,
    head: bool,
    range: Option<&str>,
    if_none_match: Option<&str>,
) -> Reply {
    let Some(dir) = dir else {
        return missing();
    };
    let Ok(root) = fs::canonicalize(dir) else {
        return missing();
    };
    if !root.is_dir() {
        return missing();
    }
    let candidate = root.join(kind.file);
    let Ok(file) = fs::canonicalize(&candidate) else {
        return missing();
    };
    if !file.starts_with(&root) {
        return missing();
    }
    let Ok(meta) = fs::metadata(&file) else {
        return missing();
    };
    if !meta.is_file() {
        return missing();
    }
    let len = meta.len();
    if len > MAX_PREVIEW_BYTES {
        return Reply {
            status: "413 Payload Too Large",
            content_type: "text/plain",
            body: b"preview too large".to_vec(),
            content_length: b"preview too large".len(),
            cache_control: "no-store",
            extra: String::new(),
        };
    }
    let Ok(bytes) = fs::read(&file) else {
        return missing();
    };
    if bytes.len() as u64 != len || bytes.len() as u64 > MAX_PREVIEW_BYTES {
        return missing();
    }
    let etag = etag_for(&meta, bytes.len());
    let selection = select_range(bytes.len() as u64, range);
    match selection {
        RangeSel::Unsatisfiable => unsatisfiable(bytes.len(), &etag, head),
        RangeSel::Full if if_none_match.is_some_and(|h| etag_matches(h, &etag)) => {
            not_modified(&etag)
        }
        RangeSel::Full => {
            let length = bytes.len();
            Reply {
                status: "200 OK",
                content_type: kind.content_type,
                body: if head { Vec::new() } else { bytes },
                content_length: length,
                cache_control: CACHE_CONTROL,
                extra: headers_for(&etag, None),
            }
        }
        RangeSel::Part { start, end } => {
            let start_us = start as usize;
            let end_us = end as usize;
            let slice = bytes[start_us..=end_us].to_vec();
            let length = slice.len();
            let content_range = format!("bytes {start}-{end}/{}", bytes.len());
            Reply {
                status: "206 Partial Content",
                content_type: kind.content_type,
                body: if head { Vec::new() } else { slice },
                content_length: length,
                cache_control: CACHE_CONTROL,
                extra: headers_for(&etag, Some(&content_range)),
            }
        }
    }
}

fn missing() -> Reply {
    let body = b"not found".to_vec();
    Reply {
        status: "404 Not Found",
        content_type: "text/plain",
        content_length: body.len(),
        body,
        cache_control: "no-store",
        extra: String::new(),
    }
}

fn not_modified(etag: &str) -> Reply {
    Reply {
        status: "304 Not Modified",
        content_type: "text/plain",
        body: Vec::new(),
        content_length: 0,
        cache_control: CACHE_CONTROL,
        extra: headers_for(etag, None),
    }
}

fn unsatisfiable(len: usize, etag: &str, head: bool) -> Reply {
    let body = b"range not satisfiable".to_vec();
    let content_range = format!("bytes */{len}");
    Reply {
        status: "416 Range Not Satisfiable",
        content_type: "text/plain",
        content_length: body.len(),
        body: if head { Vec::new() } else { body },
        cache_control: "no-store",
        extra: headers_for(etag, Some(&content_range)),
    }
}

fn headers_for(etag: &str, content_range: Option<&str>) -> String {
    let mut extra = format!("Accept-Ranges: bytes\r\nETag: {etag}\r\n");
    if let Some(range) = content_range {
        extra.push_str("Content-Range: ");
        extra.push_str(range);
        extra.push_str("\r\n");
    }
    extra
}

fn etag_for(meta: &fs::Metadata, len: usize) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("\"{len}-{mtime}\"")
}

fn etag_matches(header: &str, etag: &str) -> bool {
    let header = header.trim();
    if header == "*" {
        return true;
    }
    header.split(',').any(|part| {
        let token = part.trim();
        token == etag || token.strip_prefix("W/").unwrap_or(token) == etag
    })
}

enum RangeSel {
    Full,
    Part { start: u64, end: u64 },
    Unsatisfiable,
}

/// One `bytes` range. An unknown unit or a multipart range serves the whole
/// file. A range that starts past the end is unsatisfiable. An end past the
/// last byte is clamped, per RFC 9110.
fn select_range(len: u64, header: Option<&str>) -> RangeSel {
    let Some(raw) = header.map(str::trim).filter(|s| !s.is_empty()) else {
        return RangeSel::Full;
    };
    let Some((unit, spec)) = raw.split_once('=') else {
        return RangeSel::Full;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") {
        return RangeSel::Full;
    }
    let spec = spec.trim();
    if spec.is_empty() {
        return RangeSel::Unsatisfiable;
    }
    if spec.contains(',') {
        return RangeSel::Full;
    }
    if let Some(suffix) = spec.strip_prefix('-') {
        let Ok(n) = suffix.trim().parse::<u64>() else {
            return RangeSel::Unsatisfiable;
        };
        if n == 0 || len == 0 {
            return RangeSel::Unsatisfiable;
        }
        let n = n.min(len);
        return RangeSel::Part {
            start: len - n,
            end: len - 1,
        };
    }
    let Some((start_s, end_s)) = spec.split_once('-') else {
        return RangeSel::Unsatisfiable;
    };
    let Ok(start) = start_s.trim().parse::<u64>() else {
        return RangeSel::Unsatisfiable;
    };
    if len == 0 || start >= len {
        return RangeSel::Unsatisfiable;
    }
    let end = if end_s.trim().is_empty() {
        len - 1
    } else {
        let Ok(n) = end_s.trim().parse::<u64>() else {
            return RangeSel::Unsatisfiable;
        };
        if n < start {
            return RangeSel::Unsatisfiable;
        }
        n.min(len - 1)
    };
    RangeSel::Part { start, end }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_dir_setting_is_unset() {
        assert!(dir_from_raw("").is_none());
        assert!(dir_from_raw("   ").is_none());
        assert_eq!(
            dir_from_raw(" /var/lib/galion/preview ").unwrap(),
            PathBuf::from("/var/lib/galion/preview")
        );
    }

    #[test]
    fn unknown_preview_names_are_not_this_route() {
        assert!(serve(None, "/preview/voice-preview.flac", false, None, None).is_none());
        assert!(serve(None, "/preview/../voice-preview.wav", false, None, None).is_none());
        assert!(serve(None, "/preview/voice-preview.wav/", false, None, None).is_none());
    }

    #[test]
    fn ranges_clamp_and_reject() {
        assert!(matches!(select_range(10, None), RangeSel::Full));
        assert!(matches!(
            select_range(10, Some("items=0-1")),
            RangeSel::Full
        ));
        assert!(matches!(
            select_range(10, Some("bytes=0-1,2-3")),
            RangeSel::Full
        ));
        match select_range(10, Some("bytes=0-3")) {
            RangeSel::Part { start: 0, end: 3 } => {}
            _ => panic!("closed range"),
        }
        match select_range(10, Some("bytes=8-")) {
            RangeSel::Part { start: 8, end: 9 } => {}
            _ => panic!("open range"),
        }
        match select_range(10, Some("bytes=-4")) {
            RangeSel::Part { start: 6, end: 9 } => {}
            _ => panic!("suffix"),
        }
        match select_range(10, Some("bytes=0-99")) {
            RangeSel::Part { start: 0, end: 9 } => {}
            _ => panic!("end clamp"),
        }
        assert!(matches!(
            select_range(10, Some("bytes=10-12")),
            RangeSel::Unsatisfiable
        ));
        assert!(matches!(
            select_range(10, Some("bytes=5-4")),
            RangeSel::Unsatisfiable
        ));
        assert!(matches!(
            select_range(10, Some("bytes=-0")),
            RangeSel::Unsatisfiable
        ));
        assert!(matches!(
            select_range(0, Some("bytes=0-0")),
            RangeSel::Unsatisfiable
        ));
    }
}
