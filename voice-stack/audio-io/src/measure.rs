// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Latency figures this crate can produce without inventing a device.
//!
//! [`measure_software`] is the file/loopback path: a 48 kHz period feeds the
//! 24 kHz ring into the 16 kHz framer. [`measure_live`] opens the real host
//! and records cpal's capture-to-callback delta when the host provides one.
//! A missing sound card is `LiveReport::error`, not a made-up microsecond.

use crate::capture::CaptureAssembler;
use crate::capture::CaptureQueue;
use crate::device::{host_report, open, Direction, SessionConfig};
use crate::loopback::Loopback;
use crate::playback::{Playback, Renderer};
use crate::{mono_ns, CAPTURE_FRAME_SAMPLES, CAPTURE_HZ, PLAYBACK_HZ};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct SoftwareReport {
    pub capture_hz: u32,
    pub playback_hz: u32,
    pub device_hz: u32,
    pub period_samples: usize,
    /// Index of the first hot sample in the loopback capture. 0 is the start of the first frame.
    pub impulse_capture_sample: usize,
    /// Virtual callback time when the frame that holds the impulse was sealed.
    pub impulse_frame_ready_ms: f64,
    pub capture_frames_in_one_second: usize,
    /// Wall time to box-resample and slice 1 s of 48 kHz audio into 16 kHz frames.
    pub capture_one_second_us: u128,
    /// Wall time to push 1 s of 24 kHz audio and render it at 48 kHz.
    pub playback_one_second_us: u128,
    /// Wall time to push one 80 ms heartbeat frame and render it at 48 kHz.
    pub heartbeat_frame_us: u128,
    /// Wall time from `barge_in` to the render that output silence.
    pub flush_honor_ns: u128,
    pub flush_discarded: u64,
    /// `capture_one_second_us` compared with one second of audio. Under 1 means faster than realtime.
    pub capture_realtime_factor: f64,
}

#[derive(Clone, Debug)]
pub struct LiveReport {
    pub host: String,
    pub available_hosts: Vec<String>,
    pub pulse_plugin: bool,
    pub device_count: usize,
    pub list_error: Option<String>,
    pub open_error: Option<String>,
    pub input_name: Option<String>,
    pub input_rate: Option<u32>,
    pub callbacks: u64,
    pub frames: u64,
    /// Host-reported capture-to-callback delays, microseconds. Empty when the device did not report any.
    pub capture_to_callback_us: Vec<u32>,
    pub capture_to_callback_median_us: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct FullReport {
    pub software: SoftwareReport,
    pub live: LiveReport,
}

pub fn measure_software() -> SoftwareReport {
    let mut loopback = Loopback::new(48_000, 480).expect("loopback rates");
    let mut burst = vec![0.0f32; PLAYBACK_HZ as usize];
    for sample in burst.iter_mut().take(480) {
        *sample = 0.8;
    }
    assert!(loopback.playback.try_push_all(&burst));
    let mut impulse_at = None;
    let mut ready_ns = 0u64;
    let mut captured = 0usize;
    for _ in 0..20 {
        for frame in loopback.tick() {
            if impulse_at.is_none() {
                if let Some(pos) = frame.pcm.iter().position(|s| *s > 8_000) {
                    impulse_at = Some(captured + pos);
                    ready_ns = frame.ready_ns;
                }
            }
            captured += CAPTURE_FRAME_SAMPLES;
        }
        if impulse_at.is_some() {
            break;
        }
    }

    let capture_one_second_us = time_capture_one_second();
    let playback_one_second_us = time_playback_one_second();
    let heartbeat_frame_us = time_heartbeat_frame();
    let (flush_honor_ns, flush_discarded) = time_flush();

    SoftwareReport {
        capture_hz: CAPTURE_HZ,
        playback_hz: PLAYBACK_HZ,
        device_hz: 48_000,
        period_samples: 480,
        impulse_capture_sample: impulse_at.unwrap_or(usize::MAX),
        impulse_frame_ready_ms: ready_ns as f64 / 1_000_000.0,
        capture_frames_in_one_second: frames_in_one_second(),
        capture_one_second_us,
        playback_one_second_us,
        heartbeat_frame_us,
        flush_honor_ns,
        flush_discarded,
        capture_realtime_factor: capture_one_second_us as f64 / 1_000_000.0,
    }
}

fn frames_in_one_second() -> usize {
    let queue = CaptureQueue::new(128);
    let mut asm = CaptureAssembler::new(48_000).expect("rate");
    let block = vec![0.1f32; 4_800];
    for i in 0..10 {
        asm.push_mono(&block, (i + 1) as u64 * 100_000_000, None, &queue);
    }
    queue.pushed() as usize
}

fn time_capture_one_second() -> u128 {
    let queue = CaptureQueue::new(128);
    let mut asm = CaptureAssembler::new(48_000).expect("rate");
    let block = vec![0.1f32; 48_000];
    let start = Instant::now();
    asm.push_mono(&block, 1_000_000_000, None, &queue);
    let us = start.elapsed().as_micros();
    assert_eq!(queue.pushed(), 50, "1 s of 48 kHz must be 50 frames");
    us
}

fn time_playback_one_second() -> u128 {
    let pb = Playback::new(65_536);
    let block = vec![0.2f32; PLAYBACK_HZ as usize];
    let start = Instant::now();
    assert!(pb.try_push_all(&block));
    let mut renderer = Renderer::new(pb, 48_000).expect("rate");
    let mut buf = vec![0.0; 4_800];
    for _ in 0..10 {
        let _ = renderer.render(&mut buf);
    }
    start.elapsed().as_micros()
}

fn time_heartbeat_frame() -> u128 {
    let pb = Playback::new(8_192);
    let frame = vec![0.2f32; crate::PLAYBACK_FRAME_SAMPLES];
    let start = Instant::now();
    pb.push_heartbeat_frame(&frame).expect("frame");
    let mut renderer = Renderer::new(pb, 48_000).expect("rate");
    let mut buf = vec![0.0; 3_840];
    let _ = renderer.render(&mut buf);
    start.elapsed().as_micros()
}

fn time_flush() -> (u128, u64) {
    let pb = Playback::new(8_192);
    pb.push(&vec![0.4f32; 4_000]);
    let mut renderer = Renderer::new(pb.clone(), PLAYBACK_HZ).expect("rate");
    let t0 = mono_ns();
    pb.barge_in();
    let mut buf = vec![0.0; 256];
    let result = renderer.render(&mut buf);
    let dt = mono_ns().saturating_sub(t0);
    assert!(result.flushed);
    assert!(buf.iter().all(|s| *s == 0.0));
    (u128::from(dt), pb.discarded())
}

/// Open the capture device for `listen` and collect host timestamps.
/// Returns quickly when no device can be opened.
pub fn measure_live(listen: Duration) -> LiveReport {
    let listed = host_report();
    let mut report = LiveReport {
        host: listed.host.to_string(),
        available_hosts: listed.available,
        pulse_plugin: listed.pulse_plugin,
        device_count: listed.devices.len(),
        list_error: listed.error,
        open_error: None,
        input_name: None,
        input_rate: None,
        callbacks: 0,
        frames: 0,
        capture_to_callback_us: Vec::new(),
        capture_to_callback_median_us: None,
    };
    let session = match open(&SessionConfig::from_env(Direction::Capture)) {
        Ok(session) => session,
        Err(err) => {
            report.open_error = Some(err.to_string());
            return report;
        }
    };
    report.input_name = session.info().input_name.clone();
    report.input_rate = session.info().input_rate;
    std::thread::sleep(listen);
    report.callbacks = session.capture().callbacks();
    let mut latencies = Vec::new();
    let mut frames = 0u64;
    let capture = session.capture();
    while let Some(frame) = capture.pop() {
        frames += 1;
        if frame.device_latency_known {
            latencies.push(frame.device_latency_us);
        }
    }
    report.frames = frames;
    report.capture_to_callback_median_us = median_u32(&latencies);
    report.capture_to_callback_us = latencies;
    if let Some(err) = session.last_error() {
        report.open_error = Some(err);
    }
    report
}

pub fn measure_all() -> FullReport {
    FullReport {
        software: measure_software(),
        live: measure_live(Duration::from_millis(200)),
    }
}

fn median_u32(values: &[u32]) -> Option<u32> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    Some(v[v.len() / 2])
}

pub fn software_json(report: &SoftwareReport) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"capture_hz\": {capture_hz},\n",
            "  \"playback_hz\": {playback_hz},\n",
            "  \"device_hz\": {device_hz},\n",
            "  \"period_samples\": {period_samples},\n",
            "  \"impulse_capture_sample\": {impulse},\n",
            "  \"impulse_frame_ready_ms\": {ready},\n",
            "  \"capture_frames_in_one_second\": {frames},\n",
            "  \"capture_one_second_us\": {cap_us},\n",
            "  \"playback_one_second_us\": {play_us},\n",
            "  \"heartbeat_frame_us\": {heart_us},\n",
            "  \"flush_honor_ns\": {flush_ns},\n",
            "  \"flush_discarded\": {discarded},\n",
            "  \"capture_realtime_factor\": {rtf}\n",
            "}}"
        ),
        capture_hz = report.capture_hz,
        playback_hz = report.playback_hz,
        device_hz = report.device_hz,
        period_samples = report.period_samples,
        impulse = report.impulse_capture_sample,
        ready = format_ms(report.impulse_frame_ready_ms),
        frames = report.capture_frames_in_one_second,
        cap_us = report.capture_one_second_us,
        play_us = report.playback_one_second_us,
        heart_us = report.heartbeat_frame_us,
        flush_ns = report.flush_honor_ns,
        discarded = report.flush_discarded,
        rtf = format_ms(report.capture_realtime_factor),
    )
}

pub fn live_json(report: &LiveReport) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"host\": {host},\n",
            "  \"available_hosts\": {hosts},\n",
            "  \"pulse_plugin\": {pulse},\n",
            "  \"device_count\": {count},\n",
            "  \"list_error\": {list_err},\n",
            "  \"open_error\": {open_err},\n",
            "  \"input_name\": {name},\n",
            "  \"input_rate\": {rate},\n",
            "  \"callbacks\": {callbacks},\n",
            "  \"frames\": {frames},\n",
            "  \"capture_to_callback_us\": {samples},\n",
            "  \"capture_to_callback_median_us\": {median}\n",
            "}}"
        ),
        host = json_str(&report.host),
        hosts = json_string_list(&report.available_hosts),
        pulse = report.pulse_plugin,
        count = report.device_count,
        list_err = json_opt(report.list_error.as_deref()),
        open_err = json_opt(report.open_error.as_deref()),
        name = json_opt(report.input_name.as_deref()),
        rate = json_opt_u32(report.input_rate),
        callbacks = report.callbacks,
        frames = report.frames,
        samples = json_u32_list(&report.capture_to_callback_us),
        median = json_opt_u32(report.capture_to_callback_median_us),
    )
}

pub fn full_json(report: &FullReport) -> String {
    format!(
        "{{\n  \"software\": {},\n  \"live\": {}\n}}",
        software_json(&report.software),
        live_json(&report.live)
    )
}

fn format_ms(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.4}")
    } else {
        "null".to_string()
    }
}

fn json_opt(value: Option<&str>) -> String {
    match value {
        Some(v) => json_str(v),
        None => "null".to_string(),
    }
}

fn json_opt_u32(value: Option<u32>) -> String {
    match value {
        Some(v) => v.to_string(),
        None => "null".to_string(),
    }
}

fn json_string_list(values: &[String]) -> String {
    let body = values
        .iter()
        .map(|v| json_str(v))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_u32_list(values: &[u32]) -> String {
    let body = values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{body}]")
}

fn json_str(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_path_is_inside_the_20_ms_frame_and_faster_than_realtime() {
        let report = measure_software();
        assert_eq!(report.capture_frames_in_one_second, 50);
        assert!(
            report.impulse_capture_sample < 32,
            "impulse at {}",
            report.impulse_capture_sample
        );
        assert!(
            (report.impulse_frame_ready_ms - 20.0).abs() < 0.1,
            "ready {}",
            report.impulse_frame_ready_ms
        );
        assert!(
            report.capture_one_second_us < 200_000,
            "{}",
            report.capture_one_second_us
        );
        assert!(
            report.playback_one_second_us < 200_000,
            "{}",
            report.playback_one_second_us
        );
        assert!(
            report.heartbeat_frame_us < 80_000,
            "{}",
            report.heartbeat_frame_us
        );
        assert!(
            report.flush_honor_ns < 50_000_000,
            "{}",
            report.flush_honor_ns
        );
        assert!(report.flush_discarded >= 4_000);
        assert!(report.capture_realtime_factor < 0.2);
        let host = crate::compiled_host();
        assert!(host == "alsa" || host == "wasapi" || host == "default");
    }
}
