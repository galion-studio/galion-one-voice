//! Tiny argv parser: `--key value`, `--flag`, positionals.
use std::collections::HashMap;
pub struct Args { pub kv: HashMap<String, String>, pub flags: Vec<String>, pub pos: Vec<String> }
impl Args {
    pub fn parse(flag_names: &[&str]) -> Args {
        let mut kv = HashMap::new(); let mut flags = vec![]; let mut pos = vec![];
        let mut it = std::env::args().skip(1);
        while let Some(a) = it.next() {
            if let Some(k) = a.strip_prefix("--") {
                if flag_names.contains(&k) { flags.push(k.to_string()); }
                else if let Some((k, v)) = k.split_once('=') { kv.insert(k.to_string(), v.to_string()); }
                else { kv.insert(k.to_string(), it.next().unwrap_or_default()); }
            } else { pos.push(a); }
        }
        Args { kv, flags, pos }
    }
    pub fn get(&self, k: &str) -> Option<&str> { self.kv.get(k).map(|s| s.as_str()) }
    pub fn has(&self, k: &str) -> bool { self.flags.iter().any(|f| f == k) }
}
pub fn die(msg: &str) -> ! { eprintln!("{msg}"); std::process::exit(2) }
pub fn r(x: f64, d: i32) -> serde_json::Value {
    if !x.is_finite() { return serde_json::Value::Null; }
    let k = 10f64.powi(d); serde_json::json!((x * k).round() / k)
}
pub fn mean_sd(v: &[f64]) -> (f64, f64) {
    if v.is_empty() { return (f64::NAN, f64::NAN); }
    let m = v.iter().sum::<f64>() / v.len() as f64;
    // population sd (numpy default, ddof=0) = the convention of own-clone-analysis/scores
    let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
    (m, sd)
}
