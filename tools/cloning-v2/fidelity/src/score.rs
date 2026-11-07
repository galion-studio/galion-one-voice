//! Offline metrics: WER, DSP embedding cosine, MOS proxy, persona emotion_match.
//!
//! ECAPA is recorded only when a caller passes a galion-ecapa (or equivalent) JSON.
//! The DSP embedding cosine is never stored in the ECAPA field.

use serde::Serialize;
use serde_json::Value;

use crate::analyze::{self, analyze};
use crate::audio::{self, TARGET_SR};
use crate::embed;
use crate::envelope;

pub const WER_GATE: f64 = 0.018;
pub const ECAPA_GATE: f64 = 0.7057;
/// Same gate as `galion_persona::GATE` in `scorers/persona/galion-persona`.
pub const EMOTION_GATE: f64 = 0.83;

pub fn normalize_words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mapped: String = lower
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c == '\'' || c == ' ' {
                c
            } else {
                ' '
            }
        })
        .collect();
    mapped.split_whitespace().map(|w| w.to_string()).collect()
}

pub fn word_error_rate(reference: &str, hypothesis: &str) -> Option<f64> {
    let reference_words = normalize_words(reference);
    if reference_words.is_empty() {
        return None;
    }
    let hypothesis_words = normalize_words(hypothesis);
    Some(edit_distance(&reference_words, &hypothesis_words) as f64 / reference_words.len() as f64)
}

fn edit_distance(reference: &[String], hypothesis: &[String]) -> usize {
    let (n, m) = (reference.len(), hypothesis.len());
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = usize::from(reference[i - 1] != hypothesis[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

pub fn wer_pass(wer: f64) -> bool {
    wer <= WER_GATE
}

/// Strict: the rollout line is ECAPA > 0.7057.
pub fn ecapa_pass(ecapa: f64) -> bool {
    ecapa > ECAPA_GATE
}

/// `emotion_match = 0.70 * category_blend + 0.30 * emb_sim`, the persona scorer formula.
pub fn emotion_match(category_blend: f64, emb_sim: f64) -> f64 {
    0.70 * category_blend + 0.30 * emb_sim
}

pub fn emotion_pass(value: f64) -> bool {
    value + 1e-12 >= EMOTION_GATE
}

/// MOS proxy in [1, 5]. Not SQUIM. `mos = 1 + 4 * clamp(0.40*snr_n + 0.35*voiced + 0.25*(1-flatness) - 0.30*clip_pen, 0, 1)`
/// with `snr_n = clamp((snr_db-5)/25, 0, 1)` and `clip_pen = clamp(20*clip_fraction, 0, 1)`.
pub fn mos_from_components(
    snr_db: f64,
    voiced_ratio: f64,
    flatness: f64,
    clip_fraction: f64,
) -> f64 {
    let snr_n = ((snr_db - 5.0) / 25.0).clamp(0.0, 1.0);
    let voiced = voiced_ratio.clamp(0.0, 1.0);
    let tonal = (1.0 - flatness.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let clip_pen = (clip_fraction * 20.0).clamp(0.0, 1.0);
    let raw = (0.40 * snr_n + 0.35 * voiced + 0.25 * tonal - 0.30 * clip_pen).clamp(0.0, 1.0);
    1.0 + 4.0 * raw
}

pub fn mos_proxy(samples: &[f32], sr: u32) -> f64 {
    let analysis = analyze(samples, sr);
    let voiced = if analysis.frames.is_empty() {
        0.0
    } else {
        analysis.frames.iter().filter(|frame| frame.voiced).count() as f64
            / analysis.frames.len() as f64
    };
    let snr = if analysis.stationary {
        if voiced >= 0.55 {
            28.0
        } else {
            0.0
        }
    } else {
        let rms: Vec<f32> = analysis.frames.iter().map(|frame| frame.rms).collect();
        let p90 = audio::percentile(&rms, 0.90);
        20.0 * ((p90 as f64 + 1e-8) / (analysis.noise_floor as f64 + 1e-8)).log10()
    };
    let flatness = analyze::spectral_flatness(&analyze::welch_power(samples, 1024));
    mos_from_components(snr, voiced, flatness, analyze::clip_fraction(samples))
}

pub fn emotion_from_persona_json(value: &Value) -> Option<(f64, &'static str)> {
    if let Some(score) = value.get("emotion_match").and_then(Value::as_f64) {
        return Some((score, "persona_json.emotion_match"));
    }
    if let (Some(blend), Some(emb)) = (
        value.get("category_blend").and_then(Value::as_f64),
        value.get("emb_sim").and_then(Value::as_f64),
    ) {
        return Some((
            emotion_match(blend, emb),
            "persona_json.category_blend+emb_sim",
        ));
    }
    if let Some(sets) = value.get("sets").and_then(Value::as_object) {
        let scores: Vec<f64> = sets
            .values()
            .filter_map(|entry| entry.get("emotion_match").and_then(Value::as_f64))
            .collect();
        if !scores.is_empty() {
            return Some((
                scores.iter().sum::<f64>() / scores.len() as f64,
                "persona_json.sets.mean",
            ));
        }
    }
    None
}

fn as_f32_vec(value: &Value) -> Option<Vec<f32>> {
    let array = value.as_array()?;
    let nums: Vec<f32> = array
        .iter()
        .filter_map(|v| v.as_f64().map(|x| x as f32))
        .collect();
    if nums.len() == array.len() && !nums.is_empty() {
        Some(nums)
    } else {
        None
    }
}

pub fn ecapa_from_json(value: &Value) -> Option<(f64, &'static str)> {
    if let Some(score) = value.get("ecapa").and_then(Value::as_f64) {
        return Some((score, "ecapa_json.ecapa"));
    }
    if let Some(score) = value.get("cosine").and_then(Value::as_f64) {
        return Some((score, "ecapa_json.cosine"));
    }
    if let (Some(reference), Some(hypothesis)) = (
        value.get("ref_embedding").and_then(as_f32_vec),
        value.get("hyp_embedding").and_then(as_f32_vec),
    ) {
        return embed::cosine(&reference, &hypothesis).map(|c| (c, "ecapa_json.embeddings"));
    }
    if let Some(scores) = value.get("scores").and_then(Value::as_object) {
        let nums: Vec<f64> = scores.values().filter_map(Value::as_f64).collect();
        if nums.len() == 1 {
            return Some((nums[0], "ecapa_json.scores"));
        }
        if !nums.is_empty() {
            return Some((
                nums.iter().sum::<f64>() / nums.len() as f64,
                "ecapa_json.scores.mean",
            ));
        }
    }
    if let Some(score) = value.get("mean").and_then(Value::as_f64) {
        return Some((score, "ecapa_json.mean"));
    }
    None
}

#[derive(Debug, Serialize)]
pub struct EvalReport {
    pub schema: &'static str,
    pub wer: Option<f64>,
    pub wer_percent: Option<f64>,
    pub wer_pass: Option<bool>,
    pub wer_source: String,
    pub ecapa: Option<f64>,
    pub ecapa_pass: Option<bool>,
    pub ecapa_source: String,
    pub embed_cosine: Option<f64>,
    pub embed_note: &'static str,
    pub mos_proxy: Option<f64>,
    pub mos_source: &'static str,
    pub emotion_match: Option<f64>,
    pub emotion_match_pass: Option<bool>,
    pub emotion_match_source: String,
    pub timbre_applied: bool,
    pub timbre_strength: f64,
    pub gates: GateSpec,
    pub rollout_claim: &'static str,
}

#[derive(Debug, Serialize)]
pub struct GateSpec {
    pub wer_max: f64,
    pub wer_max_percent: f64,
    pub ecapa_min_exclusive: f64,
    pub emotion_match_min: f64,
}

pub struct EvalInput<'a> {
    pub reference: &'a [f32],
    pub reference_sr: u32,
    pub hypothesis: &'a [f32],
    pub hypothesis_sr: u32,
    pub reference_text: Option<&'a str>,
    pub hypothesis_text: Option<&'a str>,
    pub ecapa_json: Option<&'a Value>,
    pub persona_json: Option<&'a Value>,
    pub apply_timbre: bool,
    pub timbre_strength: Option<f32>,
}

pub fn evaluate(input: EvalInput<'_>) -> EvalReport {
    let reference = audio::resample_linear(input.reference, input.reference_sr, TARGET_SR);
    let mut hypothesis = audio::resample_linear(input.hypothesis, input.hypothesis_sr, TARGET_SR);
    let strength = if input.apply_timbre {
        input.timbre_strength.unwrap_or(0.55).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if strength > 1e-6 {
        hypothesis = envelope::correct_envelope(&hypothesis, &reference, TARGET_SR, strength);
    }
    let embed_cosine = embed::cosine(
        &embed::embedding(&reference, TARGET_SR),
        &embed::embedding(&hypothesis, TARGET_SR),
    );
    let mos = if hypothesis.is_empty() {
        None
    } else {
        Some(mos_proxy(&hypothesis, TARGET_SR))
    };

    let (wer, wer_source) = match (input.reference_text, input.hypothesis_text) {
        (Some(reference_text), Some(hypothesis_text)) => {
            match word_error_rate(reference_text, hypothesis_text) {
                Some(value) => (Some(value), "transcripts".to_string()),
                None => (None, "empty_reference_transcript".to_string()),
            }
        }
        _ => (None, "not_provided".to_string()),
    };
    let (ecapa, ecapa_source) = match input.ecapa_json {
        Some(json) => match ecapa_from_json(json) {
            Some((value, source)) => (Some(value), source.to_string()),
            None => (None, "ecapa_json_unparsed".to_string()),
        },
        None => (None, "not_provided".to_string()),
    };
    let (emotion, emotion_source) = match input.persona_json {
        Some(json) => match emotion_from_persona_json(json) {
            Some((value, source)) => (Some(value), source.to_string()),
            None => (None, "persona_json_unparsed".to_string()),
        },
        None => (None, "not_provided".to_string()),
    };

    EvalReport {
        schema: "galion-fidelity-eval/1",
        wer_percent: wer.map(|w| w * 100.0),
        wer_pass: wer.map(wer_pass),
        wer,
        wer_source,
        ecapa_pass: ecapa.map(ecapa_pass),
        ecapa,
        ecapa_source,
        embed_cosine,
        embed_note: "DSP log-mel embedding cosine. Not ECAPA. ECAPA stays null until galion-ecapa JSON is passed.",
        mos_proxy: mos,
        mos_source: "dsp_proxy",
        emotion_match_pass: emotion.map(emotion_pass),
        emotion_match: emotion,
        emotion_match_source: emotion_source,
        timbre_applied: strength > 1e-6,
        timbre_strength: strength as f64,
        gates: GateSpec {
            wer_max: WER_GATE,
            wer_max_percent: 1.8,
            ecapa_min_exclusive: ECAPA_GATE,
            emotion_match_min: EMOTION_GATE,
        },
        rollout_claim: "reporting only; this harness does not roll out or deploy",
    }
}
