//! Emotion-match metric for galion-voicecheck.
//!
//! Scores how well a clone's 12 test lines reproduce emotion *moments* found in
//! the owner base recording. Moments are ~1.5–3 s windows classified with the
//! existing fp32 galion-emotion stack (SUPERB 4-class SER + MSP A/V/D + acoustic
//! proxies). No 9-class SER is on the box; categories beyond neu/hap/ang/sad are
//! either **proxy** (A/V/D + prosody) or **not_measurable**.
//!
//! Headline `emotion_match` (higher is better) — universal formula:
//!   category_blend = Σ strength_C·score_C / Σ strength_C
//!   emb_sim = mean_i max_j mean_centred_cosine(emb_i, emb_window_j)  (EmoCtrlTTS-Eval / Emo-DPO)
//!   emotion_match = 0.70·category_blend + 0.30·emb_sim
//!   score_C = 0.55·max_i match_i + 0.45·mean_i match_i
//! Legacy galion-emotion path still uses the 7-D (neu,hap,ang,sad,A,V,D) match.
//! Pitch/energy proxies are diagnostics only — not used in gate strengths for `universal`.
//!
//! Per-category leaderboard and the formula are emitted in the report JSON.

use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const DEFAULT_WINDOW_S: f64 = 2.5;
pub const DEFAULT_EMO_MIN: f64 = 0.83;
/// Minimum category strength on a base window to count as a "moment".
pub const DEFAULT_PRESENCE: f64 = 0.28;
const EPS: f64 = 1e-12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasureKind {
    DirectModelLabel,
    ProxyAvdProsody,
    NotMeasurable,
}

impl MeasureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            MeasureKind::DirectModelLabel => "direct model label",
            MeasureKind::ProxyAvdProsody => "proxy from A/V/D + prosody",
            MeasureKind::NotMeasurable => "not measurable",
        }
    }
}

#[derive(Clone, Debug)]
pub struct CategoryDef {
    pub id: &'static str,
    pub kind: MeasureKind,
    pub notes: &'static str,
}

/// Slavik's requested set + direct SER labels we can always report.
pub fn catalog() -> Vec<CategoryDef> {
    vec![
        CategoryDef { id: "joyful", kind: MeasureKind::DirectModelLabel, notes: "hap probability + positive valence (SUPERB hap is the direct class)" },
        CategoryDef { id: "happy", kind: MeasureKind::DirectModelLabel, notes: "alias of joyful; same hap+valence detector (deduped in blend)" },
        CategoryDef { id: "aggressive", kind: MeasureKind::DirectModelLabel, notes: "ang probability (SUPERB ang)" },
        CategoryDef { id: "yelling", kind: MeasureKind::ProxyAvdProsody, notes: "ang + high arousal + elevated F0/energy; not a model class" },
        CategoryDef { id: "fear", kind: MeasureKind::ProxyAvdProsody, notes: "low valence + high arousal + low dominance; NO fear class in SUPERB — proxy only" },
        CategoryDef { id: "logic", kind: MeasureKind::ProxyAvdProsody, notes: "high neu + low arousal + steady F0 (low f0_sd); calm/neutral proxy" },
        CategoryDef { id: "reason", kind: MeasureKind::ProxyAvdProsody, notes: "same detector as logic (calm neutral); kept as alias for the requested name" },
        CategoryDef { id: "love", kind: MeasureKind::ProxyAvdProsody, notes: "high valence + low-mid arousal + soft level; not a model class" },
        CategoryDef { id: "seductive", kind: MeasureKind::ProxyAvdProsody, notes: "high valence + low arousal + soft energy / higher shimmer; weak proxy, overlaps love" },
        CategoryDef { id: "sad", kind: MeasureKind::DirectModelLabel, notes: "sad probability (SUPERB sad)" },
        CategoryDef { id: "neutral", kind: MeasureKind::DirectModelLabel, notes: "neu probability (SUPERB neu)" },
        CategoryDef { id: "surprise", kind: MeasureKind::NotMeasurable, notes: "no surprise class in SUPERB/MSP; would need emotion2vec+ / 8-class SER" },
        CategoryDef { id: "disgust", kind: MeasureKind::NotMeasurable, notes: "no disgust class in SUPERB/MSP; would need emotion2vec+" },
    ]
}

/// Categories that participate in the blended gate (aliases collapsed).
pub fn blend_ids() -> &'static [&'static str] {
    &["joyful", "aggressive", "yelling", "fear", "logic", "love", "seductive", "sad", "neutral"]
}

#[derive(Clone, Debug, Default)]
pub struct EmoVec {
    pub neu: f64,
    pub hap: f64,
    pub ang: f64,
    pub sad: f64,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub f0_mean_hz: f64,
    pub f0_sd_st: f64,
    pub f0_range_st: f64,
    pub energy_sd_db: f64,
    pub rms_dbfs: f64,
    pub shimmer_local_pct: f64,
    pub start_s: f64,
    pub end_s: f64,
    pub id: String,
}

impl EmoVec {
    pub fn core7(&self) -> [f64; 7] {
        [self.neu, self.hap, self.ang, self.sad, self.arousal, self.valence, self.dominance]
    }

    pub fn from_galion_file(v: &Value, id: &str, start_s: f64, end_s: f64) -> Option<Self> {
        let ser = v.get("ser")?.get("probs")?;
        let avd = v.get("avd")?;
        let px = v.get("proxies").cloned().unwrap_or(Value::Null);
        let g = |o: &Value, k: &str| o.get(k).and_then(|x| x.as_f64()).unwrap_or(f64::NAN);
        Some(EmoVec {
            neu: g(ser, "neu"),
            hap: g(ser, "hap"),
            ang: g(ser, "ang"),
            sad: g(ser, "sad"),
            arousal: g(avd, "arousal"),
            valence: g(avd, "valence"),
            dominance: g(avd, "dominance"),
            f0_mean_hz: g(&px, "f0_mean_hz"),
            f0_sd_st: g(&px, "f0_sd_st"),
            f0_range_st: g(&px, "f0_range_st_p5_p95"),
            energy_sd_db: g(&px, "energy_sd_db"),
            rms_dbfs: g(&px, "rms_dbfs"),
            shimmer_local_pct: g(&px, "shimmer_local_pct"),
            start_s,
            end_s,
            id: id.to_string(),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "start_s": self.start_s, "end_s": self.end_s,
            "ser": {"neu": self.neu, "hap": self.hap, "ang": self.ang, "sad": self.sad},
            "avd": {"arousal": self.arousal, "valence": self.valence, "dominance": self.dominance},
            "proxies": {
                "f0_mean_hz": self.f0_mean_hz, "f0_sd_st": self.f0_sd_st, "f0_range_st": self.f0_range_st,
                "energy_sd_db": self.energy_sd_db, "rms_dbfs": self.rms_dbfs, "shimmer_local_pct": self.shimmer_local_pct
            }
        })
    }
}

fn clamp01(x: f64) -> f64 {
    if !x.is_finite() { return 0.0; }
    x.max(0.0).min(1.0)
}

fn soft_level(rms_dbfs: f64) -> f64 {
    // softer (more negative dBFS) → higher; map [-40,-10] → [1,0]
    if !rms_dbfs.is_finite() { return 0.5; }
    clamp01((-10.0 - rms_dbfs) / 30.0)
}

fn steady_f0(f0_sd_st: f64) -> f64 {
    // typical speech f0_sd ~1.5–4 st; lower → steadier
    if !f0_sd_st.is_finite() { return 0.5; }
    clamp01(1.0 - (f0_sd_st / 4.0))
}

fn high_f0(f0_mean_hz: f64, ref_mean: f64) -> f64 {
    if !(f0_mean_hz.is_finite() && ref_mean.is_finite()) || ref_mean <= 0.0 { return 0.5; }
    // > ~1.15× speaker mean → elevated
    clamp01((f0_mean_hz / ref_mean - 0.95) / 0.35)
}

/// Category presence/strength on one window in [0,1].
pub fn category_strength(cat: &str, w: &EmoVec, speaker_f0_mean: f64) -> f64 {
    let soft = soft_level(w.rms_dbfs);
    let steady = steady_f0(w.f0_sd_st);
    let hi_f0 = high_f0(w.f0_mean_hz, speaker_f0_mean);
    let hi_ar = clamp01(w.arousal);
    let hi_en = if w.energy_sd_db.is_finite() { clamp01(w.energy_sd_db / 12.0) } else { 0.5 };
    match cat {
        "joyful" | "happy" => clamp01(0.65 * w.hap + 0.35 * w.valence),
        "aggressive" => clamp01(w.ang),
        "yelling" => clamp01(0.40 * w.ang + 0.25 * hi_ar + 0.20 * hi_f0 + 0.15 * hi_en),
        "fear" => clamp01(0.35 * (1.0 - w.valence) + 0.35 * hi_ar + 0.30 * (1.0 - w.dominance)),
        "logic" | "reason" => clamp01(0.50 * w.neu + 0.25 * (1.0 - hi_ar) + 0.25 * steady),
        "love" => clamp01(0.40 * w.valence + 0.20 * w.hap + 0.20 * (1.0 - (w.arousal - 0.40).abs() / 0.40) + 0.20 * soft),
        "seductive" => {
            let shimmer = if w.shimmer_local_pct.is_finite() { clamp01(w.shimmer_local_pct / 12.0) } else { 0.4 };
            clamp01(0.35 * w.valence + 0.25 * (1.0 - hi_ar) + 0.25 * soft + 0.15 * shimmer)
        }
        "sad" => clamp01(w.sad),
        "neutral" => clamp01(w.neu),
        _ => 0.0,
    }
}

pub fn match_score(a: &EmoVec, b: &EmoVec) -> f64 {
    let va = a.core7();
    let vb = b.core7();
    let mut s = 0.0;
    for i in 0..7 {
        let d = va[i] - vb[i];
        s += d * d;
    }
    let d = (s / 7.0).sqrt();
    (1.0 - d).max(0.0)
}

#[derive(Clone, Debug)]
pub struct Moment {
    pub category: String,
    pub kind: MeasureKind,
    pub strength: f64,
    pub window: EmoVec,
}

#[derive(Clone, Debug)]
pub struct CategoryScore {
    pub category: String,
    pub kind: MeasureKind,
    pub measurable: bool,
    pub notes: String,
    pub moment: Option<Moment>,
    pub best_line: Option<String>,
    pub best_match: Option<f64>,
    pub mean_match: Option<f64>,
    pub score: Option<f64>,
    pub line_matches: BTreeMap<String, f64>,
    pub hap_vs_moment: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct EmotionMatchResult {
    pub emotion_match: Option<f64>,
    pub formula: String,
    pub window_s: f64,
    pub presence_floor: f64,
    pub moments: Vec<Moment>,
    pub leaderboard: Vec<CategoryScore>,
    pub not_measurable: Vec<Value>,
    pub ceiling: Option<f64>,
}

pub fn find_moments(windows: &[EmoVec], presence: f64) -> Vec<Moment> {
    if windows.is_empty() { return vec![]; }
    let f0m = {
        let v: Vec<f64> = windows.iter().map(|w| w.f0_mean_hz).filter(|x| x.is_finite()).collect();
        if v.is_empty() { 180.0 } else { v.iter().sum::<f64>() / v.len() as f64 }
    };
    let mut out = Vec::new();
    // Collapse aliases: happy→joyful, reason→logic for moment picking
    let cats = ["joyful", "aggressive", "yelling", "fear", "logic", "love", "seductive", "sad", "neutral"];
    for cat in cats {
        let mut best: Option<(f64, &EmoVec)> = None;
        for w in windows {
            let s = category_strength(cat, w, f0m);
            if s + EPS >= presence {
                if best.map(|(bs, _)| s > bs).unwrap_or(true) {
                    best = Some((s, w));
                }
            }
        }
        if let Some((s, w)) = best {
            let kind = catalog().into_iter().find(|c| c.id == cat).map(|c| c.kind).unwrap_or(MeasureKind::ProxyAvdProsody);
            out.push(Moment { category: cat.to_string(), kind, strength: s, window: w.clone() });
        }
    }
    out
}

pub fn score_against_moments(lines: &[(String, EmoVec)], moments: &[Moment], presence: f64) -> EmotionMatchResult {
    let formula = "emotion_match = Σ_C strength_C · (0.55·max_i match_i + 0.45·mean_i match_i) / Σ strength_C; \
match_i = max(0, 1 − RMS_7(v_line − v_moment)) on (neu,hap,ang,sad,A,V,D); \
strength_C = category detector on the best base window (≥ presence floor). \
joyful uses hap+valence (direct); aggressive=ang (direct); yelling/fear/logic/love/seductive are A/V/D+prosody proxies; \
surprise/disgust not measurable with SUPERB/MSP.".to_string();

    let mut leaderboard = Vec::new();
    let mut not_meas = Vec::new();
    for def in catalog() {
        if def.kind == MeasureKind::NotMeasurable {
            not_meas.push(json!({"category": def.id, "kind": def.kind.as_str(), "notes": def.notes}));
            continue;
        }
        if def.id == "happy" || def.id == "reason" {
            // aliases shown once via joyful/logic; still list for transparency
            leaderboard.push(CategoryScore {
                category: def.id.to_string(),
                kind: def.kind,
                measurable: true,
                notes: format!("{} (alias — see {})", def.notes, if def.id == "happy" { "joyful" } else { "logic" }),
                moment: None,
                best_line: None,
                best_match: None,
                mean_match: None,
                score: None,
                line_matches: BTreeMap::new(),
                hap_vs_moment: None,
            });
            continue;
        }
        let moment = moments.iter().find(|m| m.category == def.id).cloned();
        if let Some(m) = moment {
            let mut matches = BTreeMap::new();
            let mut vals = Vec::new();
            for (id, line) in lines {
                let sc = match_score(line, &m.window);
                matches.insert(id.clone(), sc);
                vals.push((id.clone(), sc));
            }
            vals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let best = vals.first().cloned();
            let mean = if vals.is_empty() { f64::NAN } else { vals.iter().map(|x| x.1).sum::<f64>() / vals.len() as f64 };
            let score = best.as_ref().map(|(_, b)| 0.55 * b + 0.45 * mean);
            let hap_vs = if m.window.hap > EPS {
                Some(lines.iter().map(|(_, l)| l.hap).sum::<f64>() / lines.len().max(1) as f64 / m.window.hap)
            } else { None };
            leaderboard.push(CategoryScore {
                category: def.id.to_string(),
                kind: def.kind,
                measurable: true,
                notes: def.notes.to_string(),
                moment: Some(m),
                best_line: best.as_ref().map(|b| b.0.clone()),
                best_match: best.map(|b| b.1),
                mean_match: if mean.is_finite() { Some(mean) } else { None },
                score,
                line_matches: matches,
                hap_vs_moment: hap_vs,
            });
        } else {
            leaderboard.push(CategoryScore {
                category: def.id.to_string(),
                kind: def.kind,
                measurable: true,
                notes: format!("{} — no moment ≥ {:.2} in base", def.notes, presence),
                moment: None,
                best_line: None,
                best_match: None,
                mean_match: None,
                score: None,
                line_matches: BTreeMap::new(),
                hap_vs_moment: None,
            });
        }
    }

    // Blended overall over detected (non-alias) moments
    let mut num = 0.0;
    let mut den = 0.0;
    for cs in &leaderboard {
        if let (Some(m), Some(sc)) = (&cs.moment, cs.score) {
            if blend_ids().contains(&cs.category.as_str()) && sc.is_finite() {
                let w = m.strength.max(EPS);
                num += w * sc;
                den += w;
            }
        }
    }
    let emotion_match = if den > EPS { Some(num / den) } else { None };

    // Sort leaderboard by score desc (None last)
    leaderboard.sort_by(|a, b| match (a.score, b.score) {
        (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => a.category.cmp(&b.category),
    });

    EmotionMatchResult {
        emotion_match,
        formula,
        window_s: DEFAULT_WINDOW_S,
        presence_floor: presence,
        moments: moments.to_vec(),
        leaderboard,
        not_measurable: not_meas,
        ceiling: None,
    }
}

/// Ceiling: score the happy/joyful moment window against the *other* base windows
/// (self-match of the recording's own diversity).
pub fn ceiling_vs_rest(moments: &[Moment], windows: &[EmoVec]) -> Option<f64> {
    let lines: Vec<(String, EmoVec)> = windows.iter().map(|w| (w.id.clone(), w.clone())).collect();
    let r = score_against_moments(&lines, moments, 0.0);
    r.emotion_match
}

pub fn result_to_json(r: &EmotionMatchResult) -> Value {
    json!({
        "emotion_match": r.emotion_match,
        "formula": r.formula,
        "window_s": r.window_s,
        "presence_floor": r.presence_floor,
        "ceiling_self": r.ceiling,
        "moments": r.moments.iter().map(|m| json!({
            "category": m.category, "kind": m.kind.as_str(), "strength": m.strength,
            "start_s": m.window.start_s, "end_s": m.window.end_s, "window": m.window.to_json()
        })).collect::<Vec<_>>(),
        "leaderboard": r.leaderboard.iter().map(|c| json!({
            "category": c.category, "kind": c.kind.as_str(), "measurable": c.measurable, "notes": c.notes,
            "score": c.score, "best_line": c.best_line, "best_match": c.best_match, "mean_match": c.mean_match,
            "hap_vs_moment": c.hap_vs_moment,
            "moment_start_s": c.moment.as_ref().map(|m| m.window.start_s),
            "moment_end_s": c.moment.as_ref().map(|m| m.window.end_s),
            "moment_strength": c.moment.as_ref().map(|m| m.strength),
            "line_matches": c.line_matches,
        })).collect::<Vec<_>>(),
        "not_measurable": r.not_measurable,
    })
}


/// Mean-centred cosine (EmoCtrlTTS-Eval / Emo-DPO): subtract per-vector mean, then cosine.
pub fn mean_centred_cosine(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let ma = a.iter().sum::<f64>() / a.len() as f64;
    let mb = b.iter().sum::<f64>() / b.len() as f64;
    let mut dot = 0.0; let mut na = 0.0; let mut nb = 0.0;
    for i in 0..a.len() {
        let x = a[i] - ma; let y = b[i] - mb;
        dot += x * y; na += x * x; nb += y * y;
    }
    if na < 1e-24 || nb < 1e-24 { return 0.0; }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

/// Universal blend: 0.70 category + 0.30 embedding similarity.
pub fn blend_universal(category_blend: f64, emb_sim: f64) -> f64 {
    0.70 * category_blend + 0.30 * emb_sim
}
/// Parse `START-END` (seconds) CLI window override for a single forced joyful window.
pub fn parse_window_spec(s: &str) -> Option<(f64, f64)> {
    let (a, b) = s.split_once('-')?;
    let start: f64 = a.trim().parse().ok()?;
    let end: f64 = b.trim().parse().ok()?;
    if end > start && start >= 0.0 { Some((start, end)) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(neu: f64, hap: f64, ang: f64, sad: f64, a: f64, v: f64, d: f64) -> EmoVec {
        EmoVec {
            neu, hap, ang, sad, arousal: a, valence: v, dominance: d,
            f0_mean_hz: 190.0, f0_sd_st: 2.5, f0_range_st: 8.0,
            energy_sd_db: 9.0, rms_dbfs: -21.0, shimmer_local_pct: 8.0,
            start_s: 0.0, end_s: 2.5, id: "w".into(),
        }
    }

    #[test]
    fn joyful_peaks_on_happy_window() {
        let happy = w(0.53, 0.36, 0.10, 0.01, 0.40, 0.64, 0.50);
        let angry = w(0.02, 0.01, 0.97, 0.00, 0.58, 0.34, 0.63);
        let neu = w(0.95, 0.03, 0.01, 0.01, 0.50, 0.47, 0.58);
        assert!(category_strength("joyful", &happy, 190.0) > category_strength("joyful", &angry, 190.0));
        assert!(category_strength("joyful", &happy, 190.0) > category_strength("joyful", &neu, 190.0));
        assert!(category_strength("aggressive", &angry, 190.0) > 0.9);
        assert!(category_strength("logic", &neu, 190.0) > category_strength("logic", &angry, 190.0));
    }

    #[test]
    fn match_identical_is_one() {
        let a = w(0.5, 0.3, 0.1, 0.1, 0.4, 0.6, 0.5);
        assert!((match_score(&a, &a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn match_far_is_lower() {
        let a = w(0.9, 0.05, 0.03, 0.02, 0.4, 0.5, 0.5);
        let b = w(0.05, 0.05, 0.9, 0.0, 0.7, 0.3, 0.7);
        assert!(match_score(&a, &b) < 0.7);
    }

    #[test]
    fn find_moments_and_blend() {
        let windows = vec![
            { let mut x = w(0.95, 0.03, 0.01, 0.01, 0.50, 0.47, 0.58); x.id = "n".into(); x.start_s = 0.0; x.end_s = 2.5; x },
            { let mut x = w(0.53, 0.36, 0.10, 0.01, 0.40, 0.64, 0.50); x.id = "h".into(); x.start_s = 10.0; x.end_s = 12.5; x },
            { let mut x = w(0.02, 0.01, 0.97, 0.00, 0.58, 0.34, 0.63); x.id = "a".into(); x.start_s = 15.0; x.end_s = 17.5; x },
        ];
        let moments = find_moments(&windows, 0.28);
        assert!(moments.iter().any(|m| m.category == "joyful"));
        assert!(moments.iter().any(|m| m.category == "aggressive"));
        let lines = vec![
            ("t01".into(), w(0.40, 0.35, 0.20, 0.05, 0.42, 0.60, 0.50)),
            ("t02".into(), w(0.70, 0.10, 0.15, 0.05, 0.45, 0.50, 0.55)),
        ];
        let r = score_against_moments(&lines, &moments, 0.28);
        assert!(r.emotion_match.unwrap() > 0.5);
        assert!(r.leaderboard.iter().any(|c| c.category == "joyful" && c.score.is_some()));
        assert!(!r.not_measurable.is_empty());
    }

    #[test]
    fn parse_window() {
        assert_eq!(parse_window_spec("9.947-12.434"), Some((9.947, 12.434)));
        assert_eq!(parse_window_spec("10-12"), Some((10.0, 12.0)));
        assert!(parse_window_spec("12-10").is_none());
    }

    #[test]
    fn surprise_disgust_not_measurable() {
        let cats: Vec<_> = catalog().into_iter().filter(|c| c.id == "surprise" || c.id == "disgust").collect();
        assert_eq!(cats.len(), 2);
        assert!(cats.iter().all(|c| c.kind == MeasureKind::NotMeasurable));
    }

    #[test]
    fn mean_centred_identical() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        assert!((mean_centred_cosine(&a, &a) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn mean_centred_orthogonalish() {
        let a = vec![1.0, -1.0, 1.0, -1.0];
        let b = vec![1.0, 1.0, -1.0, -1.0];
        let c = mean_centred_cosine(&a, &b);
        assert!(c.abs() < 0.05, "got {}", c);
    }

    #[test]
    fn blend_universal_weights() {
        assert!((blend_universal(1.0, 0.0) - 0.70).abs() < 1e-12);
        assert!((blend_universal(0.0, 1.0) - 0.30).abs() < 1e-12);
        assert!((blend_universal(0.8, 0.9) - (0.70*0.8 + 0.30*0.9)).abs() < 1e-12);
    }
}
