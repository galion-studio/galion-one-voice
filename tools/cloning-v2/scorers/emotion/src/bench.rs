//! Thin benchmark glue: label mapping + standard SER metrics (accuracy, UAR,
//! per-class recall, confusion matrix) over a `path<TAB>label` list.
//! No new scoring: predictions come from the existing ONNX SER path.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub fn read_manifest(path: &str) -> Result<Vec<(String, String)>> {
    let txt = std::fs::read_to_string(path).with_context(|| format!("read manifest {path}"))?;
    let mut out = Vec::new();
    for (i, line) in txt.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((p, l)) = line.split_once('\t') else { bail!("{path}:{}: expected `path<TAB>label`", i + 1) };
        out.push((p.to_string(), l.trim().to_lowercase()));
    }
    if out.is_empty() {
        bail!("manifest {path} has no entries");
    }
    Ok(out)
}

/// `model_label=true_label[|true_label...],...` -> model label -> true labels.
pub fn parse_label_map(spec: &str, model_labels: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut map = BTreeMap::new();
    for pair in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((m, t)) = pair.split_once('=') else { bail!("label map entry {pair:?} needs `model=true`") };
        let m = m.trim().to_string();
        if !model_labels.contains(&m) {
            bail!("label map names {m:?}, which is not a model label ({})", model_labels.join(","));
        }
        map.insert(m, t.split('|').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect());
    }
    Ok(map)
}

/// A prediction is correct when the model's top label maps to the true label.
#[allow(clippy::too_many_arguments)]
pub fn report(
    name: &str,
    model: &str,
    device: &str,
    threads: Option<usize>,
    model_labels: &[String],
    map: &BTreeMap<String, Vec<String>>,
    preds: &[(String, String)],
    load_ms: f64,
    wall_s: f64,
) -> Value {
    let covered: BTreeSet<&str> = map.values().flatten().map(String::as_str).collect();
    let truths: BTreeSet<&str> = preds.iter().map(|(t, _)| t.as_str()).collect();
    let mut confusion: BTreeMap<&str, BTreeMap<&str, u32>> = BTreeMap::new();
    let (mut n_in, mut correct) = (0u32, 0u32);
    let mut per: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for (t, p) in preds {
        *confusion.entry(t.as_str()).or_default().entry(p.as_str()).or_default() += 1;
        if covered.contains(t.as_str()) {
            let ok = map.get(p).is_some_and(|v| v.iter().any(|x| x == t));
            n_in += 1;
            correct += ok as u32;
            let e = per.entry(t.as_str()).or_default();
            e.0 += 1;
            e.1 += ok as u32;
        }
    }
    let recall: BTreeMap<&str, f64> = per.iter().map(|(k, (n, c))| (*k, *c as f64 / *n as f64)).collect();
    let uar = if recall.is_empty() { None } else { Some(recall.values().sum::<f64>() / recall.len() as f64) };
    let not_covered: Vec<&str> = truths.iter().filter(|t| !covered.contains(**t)).copied().collect();
    json!({
        "schema": "galion-emotion-bench/1",
        "name": name,
        "model": model,
        "model_labels": model_labels,
        "label_map": map,
        "device": device,
        "threads": threads,
        "n_files": preds.len(),
        "n_scored": n_in,
        "accuracy": if n_in > 0 { Some(correct as f64 / n_in as f64) } else { None },
        "uar": uar,
        "recall": recall,
        "not_covered": not_covered,
        "confusion": confusion,
        "timing": {"model_load_ms": load_ms, "wall_s": wall_s, "files_per_s": preds.len() as f64 / wall_s.max(1e-9)},
    })
}

/// One markdown table row: | scorer | kind | device | files | accuracy | UAR | not covered | wall s |
pub fn md_row(r: &Value) -> String {
    let f = |v: &Value| v.as_f64().map(|x| format!("{x:.3}")).unwrap_or_else(|| "-".into());
    let nc: Vec<&str> = r["not_covered"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
    format!(
        "| {} | ort | {} | {} | {} | {} | {} | {:.1} |",
        r["name"].as_str().unwrap_or(""),
        r["device"].as_str().unwrap_or(""),
        r["n_files"],
        f(&r["accuracy"]),
        f(&r["uar"]),
        if nc.is_empty() { "-".to_string() } else { nc.join(" ") },
        r["timing"]["wall_s"].as_f64().unwrap_or(0.0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Vec<String> {
        ["neu", "hap", "ang", "sad"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn map_rejects_unknown_model_label() {
        assert!(parse_label_map("xyz=happy", &labels()).is_err());
    }

    #[test]
    fn metrics_and_not_covered() {
        let map = parse_label_map("neu=neutral|calm,hap=happy,ang=angry,sad=sad", &labels()).unwrap();
        let preds: Vec<(String, String)> = [
            ("neutral", "neu"), ("neutral", "sad"), ("happy", "hap"), ("happy", "hap"),
            ("angry", "neu"), ("sad", "sad"), ("disgusted", "ang"), ("calm", "neu"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let r = report("t", "m", "cpu (1 threads)", Some(1), &labels(), &map, &preds, 0.0, 1.0);
        assert_eq!(r["n_scored"], 7);
        assert!((r["accuracy"].as_f64().unwrap() - 5.0 / 7.0).abs() < 1e-9);
        // recalls: neutral .5, happy 1, angry 0, sad 1, calm 1 -> UAR 0.7
        assert!((r["uar"].as_f64().unwrap() - 0.7).abs() < 1e-9);
        assert_eq!(r["not_covered"], json!(["disgusted"]));
    }
}
