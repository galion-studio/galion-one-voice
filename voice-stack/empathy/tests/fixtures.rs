// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Fixture files. A row that fails names the file and the line.

use std::fs;
use std::path::PathBuf;

use galion_sam_empathy::{
    detect_intent, Empathy, Intent, NameBook, PronounClass, TickIn, WantState,
};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn rows(name: &str) -> Vec<(usize, Vec<String>)> {
    fixture(name)
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            Some((
                i + 1,
                line.split('\t').map(|c| c.to_string()).collect::<Vec<_>>(),
            ))
        })
        .collect()
}

#[test]
fn intent_fixtures() {
    for (line, cols) in rows("intents.tsv") {
        assert_eq!(cols.len(), 2, "intents.tsv:{line} expected 2 columns");
        let expect = Intent::parse(&cols[0]).unwrap_or_else(|| panic!("intents.tsv:{line} intent"));
        let got = detect_intent(&cols[1]);
        assert_eq!(got, expect, "intents.tsv:{line} {:?}", cols[1]);
    }
}

#[test]
fn rewrite_fixtures() {
    for (line, cols) in rows("rewrite.tsv") {
        assert_eq!(cols.len(), 4, "rewrite.tsv:{line} expected 4 columns");
        let mut names = NameBook::new();
        if cols[0] != "none" {
            let class = match cols[0].as_str() {
                "he" => PronounClass::He,
                "she" => PronounClass::She,
                "they" => PronounClass::They,
                other => panic!("rewrite.tsv:{line} class {other}"),
            };
            names.note(&cols[1], class);
        }
        let got = galion_sam_empathy::rewrite_query(&cols[2], &names);
        assert_eq!(got, cols[3], "rewrite.tsv:{line}");
    }
}

#[test]
fn two_men_and_no_focus_on_her_do_not_invent_a_name() {
    let mut names = NameBook::new();
    names.note("Maciej", PronounClass::He);
    names.note("Jan", PronounClass::He);
    assert_eq!(
        galion_sam_empathy::rewrite_query("ask him", &names),
        "ask Jan"
    );
    names.note("Ada", PronounClass::She);
    assert_eq!(
        galion_sam_empathy::rewrite_query("ask him", &names),
        "ask him"
    );
    assert_eq!(
        galion_sam_empathy::rewrite_query("ask her", &names),
        "ask Ada"
    );
}

#[test]
fn want_fixtures() {
    let mut current = String::new();
    let mut layer = Empathy::new();
    for (line, cols) in rows("wants.tsv") {
        assert_eq!(cols.len(), 4, "wants.tsv:{line} expected 4 columns");
        if cols[0] != current {
            current = cols[0].clone();
            layer = Empathy::new();
        }
        let out = layer.tick(TickIn {
            inbound: None,
            asr: &cols[1],
            asr_final: true,
            invite_name: false,
            frame_ms: 80,
        });
        assert!(out.prosody.speaking.tag.is_none(), "wants.tsv:{line}");
        if cols[2] == "absent" {
            assert!(layer.wants().is_empty(), "wants.tsv:{line} opened a want");
            continue;
        }
        let state = WantState::parse(&cols[2]).unwrap_or_else(|| panic!("wants.tsv:{line} state"));
        let newest = layer
            .wants()
            .last()
            .unwrap_or_else(|| panic!("wants.tsv:{line} empty"));
        assert_eq!(newest.state, state, "wants.tsv:{line}");
        assert_eq!(newest.text, cols[3], "wants.tsv:{line}");
    }
}
