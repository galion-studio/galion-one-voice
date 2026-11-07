// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Default features of `galion-ml-voice` must not enable CUDA.
//!
//! Pi synthesis stays on the CPU. This test reads this crate's `Cargo.toml`
//! and walks the `default` feature list, including same-crate features it
//! turns on. A missing `default` key is Cargo's empty set.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[test]
fn default_features_do_not_enable_cuda() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let table = feature_table(&text).unwrap_or_else(|| {
        panic!("no [features] table in {}", path.display());
    });
    assert!(
        !table.is_empty(),
        "[features] in {} parsed as empty",
        path.display()
    );
    let enabled = default_features_enabled(&text);
    let cuda: Vec<_> = enabled
        .iter()
        .filter(|name| enables_cuda(name))
        .cloned()
        .collect();
    println!("manifest={}", path.display());
    println!("feature-keys={:?}", table.keys().collect::<Vec<_>>());
    println!("default-closure={enabled:?}");
    assert!(
        cuda.is_empty(),
        "default features enable cuda: {cuda:?}; closure={enabled:?}"
    );
}

#[test]
fn missing_default_key_is_an_empty_list() {
    let src = "[features]\nspeech = [\"dep:galion-heartbeat\"]\n";
    assert!(default_features_enabled(src).is_empty());
}

#[test]
fn explicit_cuda_in_default_is_detected() {
    let src = "[features]\ndefault = [\"cuda\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn cuda_name_match_is_case_insensitive() {
    let src = "[features]\ndefault = [\"CUDA\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn transitive_cuda_feature_is_detected() {
    let src = "[features]\ndefault = [\"gpu\"]\ngpu = [\"cuda\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn dependency_cuda_feature_is_detected() {
    let src = "[features]\ndefault = [\"ort/cuda\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn weak_dependency_cuda_feature_is_detected() {
    let src = "[features]\ndefault = [\"ort?/cuda\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn optional_dependency_named_cuda_is_detected() {
    let src = "[features]\ndefault = [\"dep:cuda\"]\n";
    assert!(closure_enables_cuda(src));
}

#[test]
fn comment_and_other_table_do_not_count() {
    let src = r#"
[package]
name = "x"
description = "mentions cuda in prose, not a feature"

[features]
# cuda stays off the default list
speech = ["dep:galion-heartbeat", "dep:galion-consent"]

[[example]]
name = "cuda_probe"
required-features = ["speech"]
"#;
    let on = default_features_enabled(src);
    assert!(on.is_empty(), "{on:?}");
    assert!(!closure_enables_cuda(src));
}

#[test]
fn non_cuda_default_stays_clean() {
    let src = r#"
[features]
default = [
  "speech",
]
speech = ["dep:galion-heartbeat", "dep:galion-consent", "dep:galion-cores"]
"#;
    let on = default_features_enabled(src);
    assert!(on.contains("speech"));
    assert!(on.contains("dep:galion-heartbeat"));
    assert!(!closure_enables_cuda(src), "{on:?}");
}

fn closure_enables_cuda(manifest: &str) -> bool {
    default_features_enabled(manifest)
        .iter()
        .any(|name| enables_cuda(name))
}

fn default_features_enabled(manifest: &str) -> BTreeSet<String> {
    let Some(table) = feature_table(manifest) else {
        return BTreeSet::new();
    };
    let Some(roots) = table.get("default") else {
        return BTreeSet::new();
    };
    let mut enabled = BTreeSet::new();
    let mut stack = roots.clone();
    while let Some(item) = stack.pop() {
        if !enabled.insert(item.clone()) {
            continue;
        }
        if let Some(name) = same_crate_feature(&item) {
            if let Some(deps) = table.get(name) {
                stack.extend(deps.iter().cloned());
            }
        }
    }
    enabled
}

fn same_crate_feature(item: &str) -> Option<&str> {
    if item.starts_with("dep:") || item.contains('/') {
        None
    } else {
        Some(item)
    }
}

fn enables_cuda(token: &str) -> bool {
    let rest = token.trim().strip_prefix("dep:").unwrap_or(token.trim());
    if let Some((pkg, feat)) = rest.split_once('/') {
        let pkg = pkg.strip_suffix('?').unwrap_or(pkg);
        return is_cuda(pkg) || is_cuda(feat);
    }
    is_cuda(rest.strip_suffix('?').unwrap_or(rest))
}

fn is_cuda(name: &str) -> bool {
    name.eq_ignore_ascii_case("cuda")
}

fn feature_table(manifest: &str) -> Option<BTreeMap<String, Vec<String>>> {
    let body = features_body(manifest)?;
    Some(parse_assignments(&body))
}

fn features_body(src: &str) -> Option<String> {
    let mut body = String::new();
    let mut in_table = false;
    let mut found = false;
    let mut depth = 0i32;
    let mut in_str: Option<char> = None;
    let mut escape = false;

    for line in src.split_inclusive('\n') {
        let logical = line.trim_end_matches(['\n', '\r']);
        let at_line_start = depth == 0 && in_str.is_none();
        if at_line_start && is_table_header(logical) {
            if is_features_header(logical) {
                in_table = true;
                found = true;
                continue;
            } else if in_table {
                break;
            }
        }
        if in_table {
            body.push_str(logical);
            body.push('\n');
            update_line_state(logical, &mut depth, &mut in_str, &mut escape);
        }
    }
    found.then_some(body)
}

fn is_table_header(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && !t.starts_with('#')
        && !t.contains('=')
        && t.starts_with('[')
        && t.ends_with(']')
}

fn is_features_header(line: &str) -> bool {
    let t = line.trim();
    if t.starts_with("[[") || !t.starts_with('[') || !t.ends_with(']') {
        return false;
    }
    t.trim_start_matches('[').trim_end_matches(']').trim() == "features"
}

fn update_line_state(line: &str, depth: &mut i32, in_str: &mut Option<char>, escape: &mut bool) {
    for c in line.chars() {
        if let Some(q) = *in_str {
            if *escape {
                *escape = false;
                continue;
            }
            if q == '"' && c == '\\' {
                *escape = true;
                continue;
            }
            if c == q {
                *in_str = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => *in_str = Some(c),
            '#' => break,
            '[' => *depth += 1,
            ']' => *depth -= 1,
            _ => {}
        }
    }
}

fn parse_assignments(body: &str) -> BTreeMap<String, Vec<String>> {
    let b = body.as_bytes();
    let mut map = BTreeMap::new();
    let mut i = 0;
    while i < b.len() {
        i = skip_ws_and_comments(b, i);
        if i >= b.len() {
            break;
        }
        let (key, next) = parse_key(b, i);
        i = next;
        if key.is_empty() {
            i += 1;
            continue;
        }
        i = skip_ws_and_comments(b, i);
        if i >= b.len() || b[i] != b'=' {
            break;
        }
        i += 1;
        i = skip_ws_and_comments(b, i);
        if i < b.len() && b[i] == b'[' {
            let (vals, next) = parse_array(b, i);
            map.insert(key, vals);
            i = next;
        } else if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
            let (val, next) = parse_string(b, i);
            map.insert(key, vec![val]);
            i = next;
        } else {
            let start = i;
            while i < b.len() && b[i] != b'\n' && b[i] != b'#' {
                i += 1;
            }
            let val = std::str::from_utf8(&b[start..i])
                .unwrap_or("")
                .trim()
                .to_string();
            if !val.is_empty() {
                map.insert(key, vec![val]);
            }
        }
    }
    map
}

fn skip_ws_and_comments(b: &[u8], mut i: usize) -> usize {
    loop {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        return i;
    }
}

fn parse_key(b: &[u8], i: usize) -> (String, usize) {
    if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
        return parse_string(b, i);
    }
    let start = i;
    let mut i = i;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.' {
            i += 1;
        } else {
            break;
        }
    }
    let key = std::str::from_utf8(&b[start..i]).unwrap_or("").to_string();
    (key, i)
}

fn parse_array(b: &[u8], mut i: usize) -> (Vec<String>, usize) {
    if i < b.len() && b[i] == b'[' {
        i += 1;
    }
    let mut vals = Vec::new();
    loop {
        i = skip_ws_and_comments(b, i);
        if i >= b.len() || b[i] == b']' {
            if i < b.len() && b[i] == b']' {
                i += 1;
            }
            break;
        }
        if b[i] == b',' {
            i += 1;
            continue;
        }
        if b[i] == b'"' || b[i] == b'\'' {
            let (value, next) = parse_string(b, i);
            vals.push(value);
            i = next;
            continue;
        }
        i += 1;
    }
    (vals, i)
}

fn parse_string(b: &[u8], i: usize) -> (String, usize) {
    let quote = b[i];
    let mut i = i + 1;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if quote == b'"' && c == b'\\' {
            i += 1;
            if i >= b.len() {
                break;
            }
            let decoded = match b[i] {
                b'n' => b'\n',
                b't' => b'\t',
                b'r' => b'\r',
                b'\\' => b'\\',
                b'"' => b'"',
                other => other,
            };
            out.push(decoded);
            i += 1;
            continue;
        }
        if c == quote {
            i += 1;
            break;
        }
        out.push(c);
        i += 1;
    }
    let value = String::from_utf8(out)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
    (value, i)
}
