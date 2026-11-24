// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Read the knowledge pack, query FTS5 in RAM, print hits.
//!
//! `remember` stays denied unless `--remember` is passed. The default does
//! not create `HORBM_MEMORY_DB` and does not publish hits into the cell.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use horbm_retrieval::{
    memory_db_path, resolve_knowledge_home, run_retrieval, FixtureRow, RunRequest, SessionConsent,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("horbm-retrieval: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut query: Option<String> = None;
    let mut knowledge: Option<PathBuf> = None;
    let mut remember = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                println!(
                    "horbm-retrieval --query TEXT [--knowledge DIR] [--remember]\n\
                     GALION_KNOWLEDGE_HOME or GALION_HORBM_KNOWLEDGE selects the card pack.\n\
                     HORBM_MEMORY_DB is not created when remember is denied."
                );
                return Ok(());
            }
            "--remember" => {
                remember = true;
                i += 1;
            }
            "--query" => {
                query = Some(need_value(&args, &mut i, "--query")?);
            }
            "--knowledge" => {
                let value = need_value(&args, &mut i, "--knowledge")?;
                knowledge = Some(PathBuf::from(value));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let query = query.ok_or("missing --query TEXT")?;
    let knowledge = match knowledge {
        Some(path) => {
            if !path.is_dir() {
                return Err(format!(
                    "knowledge directory is missing: {}",
                    path.display()
                ));
            }
            path
        }
        None => resolve_knowledge_home(&|key| env::var(key).ok())
            .map_err(|err| err.to_string())?
            .ok_or_else(|| "knowledge directory is not set".to_string())?,
    };
    let consent = if remember {
        SessionConsent::granted_remember()
    } else {
        SessionConsent::denied()
    };
    let outcome = run_retrieval(RunRequest {
        knowledge_dir: knowledge,
        memory_db: memory_db_path(&|key| env::var(key).ok()),
        consent,
        stable_prefix: query.clone(),
        extra_rows: Vec::<FixtureRow>::new(),
    })
    .map_err(|err| err.to_string())?;

    println!(
        "remember: {}",
        if consent.remember() { "grant" } else { "deny" }
    );
    println!(
        "cell: {}",
        if outcome.cell.hits.is_empty() {
            "empty"
        } else {
            "hits"
        }
    );
    println!("cell_generation: {}", outcome.cell.generation);
    println!("knowledge: {}", outcome.report.path.display());
    println!("cards: {}", outcome.report.cards);
    println!("manifest: {}", outcome.report.manifest);
    println!("query: {query}");
    println!("query_hits: {}", outcome.hits.len());
    for (n, hit) in outcome.hits.iter().enumerate() {
        let clause = hit.clause.replace(['\n', '\r'], " ");
        println!(
            "{} card={} score={:.6} source={} clause={clause}",
            n + 1,
            hit.card_id,
            hit.score,
            hit.source
        );
    }
    println!("memory_db: {}", outcome.memory_db.as_str());
    println!("revisions: {}", outcome.revisions.as_str());
    println!("disk_written: no");
    Ok(())
}

fn need_value(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    *i += 1;
    let value = args
        .get(*i)
        .cloned()
        .ok_or_else(|| format!("missing value for {flag}"))?;
    *i += 1;
    Ok(value)
}
