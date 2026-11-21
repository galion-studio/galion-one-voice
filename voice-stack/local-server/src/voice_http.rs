// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! HTTP voice path for the Galion One page.
//!
//! `POST /v2/say` is a typed turn. `POST /v1/turn` is one finished mic take.
//! `GET /v1/dot` is the mark state the page already polls. Speaker audio is
//! kept in memory for `GET /v1/reply/:id` and is not written to disk.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{json, Value};

use galion_sam_asr::{read_wav_bytes, wav_to_16k_mono};

use crate::bind::origin_allowed;
use crate::pipeline::TurnProduct;
use crate::protocol::error_json;
use crate::server::AppState;

#[derive(Debug)]
pub struct Stage {
    pub seq: u64,
    pub mic_level: f32,
    pub reply_id: u64,
    pub wav: Vec<u8>,
    pub ms: u32,
    pub started: Instant,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            seq: 0,
            mic_level: 0.0,
            reply_id: 0,
            wav: Vec::new(),
            ms: 0,
            started: Instant::now(),
        }
    }
}

pub type SharedStage = Arc<Mutex<Stage>>;

pub fn new_stage() -> SharedStage {
    Arc::new(Mutex::new(Stage::default()))
}

fn origin_ok(headers: &HeaderMap, app: &AppState) -> bool {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    origin_allowed(origin, app.bound)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (
        status,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body.to_string(),
    )
        .into_response()
}

fn product_error(product: &TurnProduct) -> Option<String> {
    for event in &product.events {
        let Ok(value) = serde_json::from_str::<Value>(event) else {
            continue;
        };
        if value.get("type").and_then(|t| t.as_str()) == Some("error") {
            return value
                .get("error")
                .and_then(|e| e.as_str())
                .map(str::to_string);
        }
    }
    None
}

fn publish(stage: &mut Stage, product: &TurnProduct) {
    stage.seq = stage.seq.saturating_add(1);
    if product.pcm.is_empty() {
        return;
    }
    stage.reply_id = stage.reply_id.saturating_add(1);
    stage.ms = if product.rate == 0 {
        0
    } else {
        ((product.pcm.len() as u64 * 1000) / u64::from(product.rate)) as u32
    };
    stage.wav = wav_bytes(&product.pcm, product.rate);
}

fn wav_bytes(pcm: &[i16], rate: u32) -> Vec<u8> {
    let data_len = pcm.len() * 2;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate.saturating_mul(2)).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for sample in pcm {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

pub async fn dot_state(State(app): State<AppState>) -> Response {
    let (phase, level, info) = {
        let Ok(pipe) = app.http_pipe.lock() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        (pipe.phase().to_string(), pipe.level(), pipe.info())
    };
    let Ok(mut stage) = app.stage.lock() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    stage.seq = stage.seq.saturating_add(1);
    let mode = if phase != "idle" {
        phase
    } else if stage.mic_level > 0.02 {
        "listening".into()
    } else {
        "idle".into()
    };
    let radius = match mode.as_str() {
        "listening" => 0.135,
        "speaking" => 0.105,
        "thinking" => 0.11,
        _ => 0.1,
    };
    let mut body = json!({
        "radius": radius,
        "t_ms": stage.seq,
        "mode": mode,
        "level": level.max(stage.mic_level),
        "persona": "samantha",
        "approval": false,
        "vad": {"backend": info.vad_backend, "ready": info.vad_ready},
    });
    if !stage.wav.is_empty() {
        body["reply"] = json!({
            "id": stage.reply_id.to_string(),
            "url": format!("/v1/reply/{}", stage.reply_id),
            "ms": stage.ms,
        });
    }
    let _ = info;
    json_response(StatusCode::OK, body)
}

pub async fn mic_level(State(app): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if !origin_ok(&headers, &app) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let rms = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("rms").and_then(|n| n.as_f64()))
        .unwrap_or(0.0) as f32;
    if let Ok(mut stage) = app.stage.lock() {
        stage.mic_level = rms.clamp(0.0, 1.0);
        stage.seq = stage.seq.saturating_add(1);
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn say_text(State(app): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if !origin_ok(&headers, &app) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let text = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_string))
        .unwrap_or_default();
    if text.trim().is_empty() {
        return json_response(StatusCode::BAD_REQUEST, json!({"error": "empty-text"}));
    }
    let pipe = app.http_pipe.clone();
    let stage = app.stage.clone();
    let product = tokio::task::spawn_blocking(move || {
        let Ok(mut guard) = pipe.lock() else {
            return TurnProduct::events(vec![error_json("turn-lock")]);
        };
        let product = guard.push_text(&text, 0);
        if let Ok(mut slot) = stage.lock() {
            publish(&mut slot, &product);
        }
        product
    })
    .await
    .unwrap_or_else(|_| TurnProduct::events(vec![error_json("turn-task")]));
    if let Some(code) = product_error(&product) {
        return json_response(StatusCode::SERVICE_UNAVAILABLE, json!({"error": code}));
    }
    let silence = product.pcm.is_empty();
    json_response(
        StatusCode::OK,
        json!({
            "reply": product.reply,
            "persona": "samantha",
            "mood": "calm",
            "silence": silence,
        }),
    )
}

pub async fn voice_turn(State(app): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if !origin_ok(&headers, &app) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let pcm = match read_wav_bytes(&body).and_then(|wav| wav_to_16k_mono(&wav)) {
        Ok(pcm) => pcm,
        Err(_) => return json_response(StatusCode::BAD_REQUEST, json!({"error": "bad-wav"})),
    };
    let pipe = app.http_pipe.clone();
    let stage = app.stage.clone();
    let product = tokio::task::spawn_blocking(move || {
        let Ok(mut guard) = pipe.lock() else {
            return TurnProduct::events(vec![error_json("turn-lock")]);
        };
        let product = guard.commit_pcm(&pcm, 16_000, 0);
        if let Ok(mut slot) = stage.lock() {
            publish(&mut slot, &product);
        }
        product
    })
    .await
    .unwrap_or_else(|_| TurnProduct::events(vec![error_json("turn-task")]));
    if let Some(code) = product_error(&product) {
        return json_response(
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error": code, "speech": true}),
        );
    }
    json_response(
        StatusCode::OK,
        json!({"ok": true, "speech": !product.reply.is_empty()}),
    )
}

pub async fn reply_audio(State(app): State<AppState>, Path(id): Path<String>) -> Response {
    let Ok(stage) = app.stage.lock() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if stage.wav.is_empty() || id != stage.reply_id.to_string() {
        return StatusCode::NOT_FOUND.into_response();
    }
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "audio/wav"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        stage.wav.clone(),
    )
        .into_response()
}

pub fn html_file(path: &std::path::Path) -> Response {
    match std::fs::read(path) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
