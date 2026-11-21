// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Device listing and cpal streams.
//!
//! Windows uses WASAPI shared mode (cpal's WASAPI host). Linux uses ALSA.
//! PulseAudio and PipeWire show up as ALSA devices when their ALSA plugin
//! is installed (`default`, `pulse`, `pipewire`). cpal does not have a
//! Pulse host. [`pulse_plugin_present`] only reports that a plugin file
//! exists on disk. It does not open the plugin.
//!
//! Capture is downmixed to mono and box-resampled to 16 kHz. Playback is
//! 24 kHz in the ring and linearly resampled to the device rate. The
//! callback reuses its scratch buffers. A period larger than 4096 frames
//! is rendered in chunks so the scratch buffer does not grow.

use crate::bus::HeartbeatIn;
use crate::capture::{CaptureAssembler, CaptureQueue};
use crate::playback::{Playback, Renderer};
use crate::resample::LinearResampler;
use crate::{AudioError, CAPTURE_HZ, PLAYBACK_HZ};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    SampleFormat, SampleRate, StreamConfig, SupportedStreamConfig, SupportedStreamConfigRange,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Capture,
    Playback,
    Duplex,
}

#[derive(Clone, Debug)]
pub struct SessionConfig {
    pub input_name: Option<String>,
    pub output_name: Option<String>,
    pub direction: Direction,
}

impl SessionConfig {
    pub fn from_env(direction: Direction) -> Self {
        Self {
            input_name: env_name("GALION_AUDIO_IN"),
            output_name: env_name("GALION_AUDIO_OUT"),
            direction,
        }
    }
}

fn env_name(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[derive(Clone, Debug)]
pub struct ListedDevice {
    pub name: String,
    pub input: bool,
    pub output: bool,
    pub default_input: bool,
    pub default_output: bool,
    /// ALSA name that points at Pulse or PipeWire (`pulse`, `pipewire`, …).
    pub pulse_name: bool,
    pub input_rate: Option<u32>,
    pub output_rate: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct HostReport {
    pub host: &'static str,
    pub available: Vec<String>,
    /// An ALSA config file whose name contains "pulse" or "pipewire" is on disk.
    pub pulse_plugin: bool,
    pub devices: Vec<ListedDevice>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub host: &'static str,
    pub input_name: Option<String>,
    pub output_name: Option<String>,
    pub input_rate: Option<u32>,
    pub output_rate: Option<u32>,
    pub input_channels: Option<u16>,
    pub output_channels: Option<u16>,
    pub input_format: Option<String>,
    pub output_format: Option<String>,
}

pub struct AudioSession {
    input: Option<cpal::Stream>,
    output: Option<cpal::Stream>,
    playback: Playback,
    capture: CaptureQueue,
    heartbeat: HeartbeatIn,
    info: SessionInfo,
    errors: Arc<Mutex<Option<String>>>,
}

impl AudioSession {
    pub fn playback(&self) -> Playback {
        self.playback.clone()
    }

    pub fn capture(&self) -> CaptureQueue {
        self.capture.clone()
    }

    /// 24 kHz frames for `AudioIn::fill`. The capture callback fills this.
    pub fn heartbeat(&self) -> HeartbeatIn {
        self.heartbeat.clone()
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }

    pub fn last_error(&self) -> Option<String> {
        self.errors
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Which directions have a live cpal stream. The stream objects themselves
    /// stay in the session so dropping it stops the device.
    pub fn streams_open(&self) -> (bool, bool) {
        (self.input.is_some(), self.output.is_some())
    }
}

pub fn compiled_host() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "wasapi"
    }
    #[cfg(target_os = "linux")]
    {
        "alsa"
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        "default"
    }
}

pub fn available_host_names() -> Vec<String> {
    cpal::available_hosts()
        .iter()
        .map(|id| id.name().to_string())
        .collect()
}

pub fn pulse_plugin_present() -> bool {
    const DIRS: &[&str] = &[
        "/usr/share/alsa/alsa.conf.d",
        "/etc/alsa/conf.d",
        "/usr/share/alsa",
    ];
    for dir in DIRS {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.contains("pulse") || name.contains("pipewire") {
                return true;
            }
        }
    }
    false
}

pub fn host_report() -> HostReport {
    let mut report = HostReport {
        host: compiled_host(),
        available: available_host_names(),
        pulse_plugin: pulse_plugin_present(),
        devices: Vec::new(),
        error: None,
    };
    let host = match platform_host() {
        Ok(host) => host,
        Err(err) => {
            report.error = Some(err.to_string());
            return report;
        }
    };
    let default_in = host.default_input_device().and_then(|d| d.name().ok());
    let default_out = host.default_output_device().and_then(|d| d.name().ok());
    match host.devices() {
        Ok(devices) => {
            for device in devices {
                let name = device.name().unwrap_or_else(|_| "unnamed".to_string());
                let (input, input_rate) = direction_rate(&device, false);
                let (output, output_rate) = direction_rate(&device, true);
                if !input && !output {
                    continue;
                }
                report.devices.push(ListedDevice {
                    default_input: default_in.as_deref() == Some(name.as_str()),
                    default_output: default_out.as_deref() == Some(name.as_str()),
                    pulse_name: is_pulse_name(&name),
                    input,
                    output,
                    input_rate,
                    output_rate,
                    name,
                });
            }
        }
        Err(err) => report.error = Some(err.to_string()),
    }
    report
}

pub fn open(config: &SessionConfig) -> Result<AudioSession, AudioError> {
    let host = platform_host()?;
    let playback = Playback::default();
    let capture = CaptureQueue::new(32);
    let heartbeat = HeartbeatIn::new("mic");
    let errors = Arc::new(Mutex::new(None));
    let mut info = SessionInfo {
        host: compiled_host(),
        input_name: None,
        output_name: None,
        input_rate: None,
        output_rate: None,
        input_channels: None,
        output_channels: None,
        input_format: None,
        output_format: None,
    };
    let input = if matches!(config.direction, Direction::Capture | Direction::Duplex) {
        let opened = open_input(
            &host,
            config.input_name.as_deref(),
            capture.clone(),
            heartbeat.clone(),
            Arc::clone(&errors),
        )?;
        info.input_name = Some(opened.name);
        info.input_rate = Some(opened.rate);
        info.input_channels = Some(opened.channels);
        info.input_format = Some(opened.format);
        Some(opened.stream)
    } else {
        None
    };
    let output = if matches!(config.direction, Direction::Playback | Direction::Duplex) {
        let opened = open_output(
            &host,
            config.output_name.as_deref(),
            playback.clone(),
            Arc::clone(&errors),
        )?;
        info.output_name = Some(opened.name);
        info.output_rate = Some(opened.rate);
        info.output_channels = Some(opened.channels);
        info.output_format = Some(opened.format);
        Some(opened.stream)
    } else {
        None
    };
    Ok(AudioSession {
        input,
        output,
        playback,
        capture,
        heartbeat,
        info,
        errors,
    })
}

struct OpenedStream {
    stream: cpal::Stream,
    name: String,
    rate: u32,
    channels: u16,
    format: String,
}

fn open_input(
    host: &cpal::Host,
    want: Option<&str>,
    queue: CaptureQueue,
    heartbeat: HeartbeatIn,
    errors: Arc<Mutex<Option<String>>>,
) -> Result<OpenedStream, AudioError> {
    let device = find_device(host, want, false)?;
    let name = device.name().unwrap_or_else(|_| "input".to_string());
    let supported = pick_config(&device, false, CAPTURE_HZ)?;
    let format = supported.sample_format();
    let format_name = format.to_string();
    let config: StreamConfig = supported.into();
    let channels = config.channels;
    let rate = config.sample_rate.0;
    let stream = match format {
        SampleFormat::F32 => {
            build_input::<f32>(&device, &config, rate, channels, queue, heartbeat, errors)?
        }
        SampleFormat::I16 => {
            build_input::<i16>(&device, &config, rate, channels, queue, heartbeat, errors)?
        }
        SampleFormat::U16 => {
            build_input::<u16>(&device, &config, rate, channels, queue, heartbeat, errors)?
        }
        other => {
            return Err(AudioError::Device(format!(
                "{name}: unsupported input format {other}"
            )))
        }
    };
    stream
        .play()
        .map_err(|err| AudioError::Device(err.to_string()))?;
    Ok(OpenedStream {
        stream,
        name,
        rate,
        channels,
        format: format_name,
    })
}

fn open_output(
    host: &cpal::Host,
    want: Option<&str>,
    playback: Playback,
    errors: Arc<Mutex<Option<String>>>,
) -> Result<OpenedStream, AudioError> {
    let device = find_device(host, want, true)?;
    let name = device.name().unwrap_or_else(|_| "output".to_string());
    let supported = pick_config(&device, true, PLAYBACK_HZ)?;
    let format = supported.sample_format();
    let format_name = format.to_string();
    let config: StreamConfig = supported.into();
    let channels = config.channels;
    let rate = config.sample_rate.0;
    let stream = match format {
        SampleFormat::F32 => {
            build_output::<f32>(&device, &config, rate, channels, playback, errors)?
        }
        SampleFormat::I16 => {
            build_output::<i16>(&device, &config, rate, channels, playback, errors)?
        }
        SampleFormat::U16 => {
            build_output::<u16>(&device, &config, rate, channels, playback, errors)?
        }
        other => {
            return Err(AudioError::Device(format!(
                "{name}: unsupported output format {other}"
            )))
        }
    };
    stream
        .play()
        .map_err(|err| AudioError::Device(err.to_string()))?;
    Ok(OpenedStream {
        stream,
        name,
        rate,
        channels,
        format: format_name,
    })
}

fn build_input<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    rate: u32,
    channels: u16,
    queue: CaptureQueue,
    heartbeat: HeartbeatIn,
    errors: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, AudioError>
where
    T: SampleWord + cpal::SizedSample,
{
    let mut assembler = CaptureAssembler::new(rate)?;
    let mut to_bus = LinearResampler::new(rate, PLAYBACK_HZ)?;
    let channels = channels.max(1) as usize;
    let mut scratch = Vec::with_capacity(8_192);
    let mut bus_buf = vec![0.0f32; 8_192];
    let origin = Instant::now();
    let err = error_callback(errors);
    device
        .build_input_stream(
            config,
            move |data: &[T], info| {
                queue.note_callback();
                let now_ns = origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
                let latency = device_latency_us(info);
                for_chunks(data, channels, &mut scratch, |mono| {
                    assembler.push_mono(mono, now_ns, latency, &queue);
                    to_bus.push(mono);
                    loop {
                        let n = to_bus.pull(&mut bus_buf);
                        if n == 0 {
                            break;
                        }
                        heartbeat.push(&bus_buf[..n]);
                    }
                });
            },
            err,
            None,
        )
        .map_err(|err| AudioError::Device(err.to_string()))
}

fn build_output<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    rate: u32,
    channels: u16,
    playback: Playback,
    errors: Arc<Mutex<Option<String>>>,
) -> Result<cpal::Stream, AudioError>
where
    T: SampleWord + cpal::SizedSample,
{
    let mut renderer = Renderer::new(playback, rate)?;
    let channels = channels.max(1) as usize;
    let mut mono = Vec::with_capacity(4_096);
    let err = error_callback(errors);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _info| {
                let frames = data.len() / channels;
                let mut off = 0;
                while off < frames {
                    let n = (frames - off).min(4_096);
                    mono.clear();
                    mono.resize(n, 0.0);
                    let _ = renderer.render(&mut mono);
                    for (i, sample) in mono.iter().enumerate() {
                        let base = (off + i) * channels;
                        for ch in 0..channels {
                            if let Some(dst) = data.get_mut(base + ch) {
                                *dst = T::to_device(*sample);
                            }
                        }
                    }
                    off += n;
                }
            },
            err,
            None,
        )
        .map_err(|err| AudioError::Device(err.to_string()))
}

fn device_latency_us(info: &cpal::InputCallbackInfo) -> Option<u32> {
    let ts = info.timestamp();
    ts.callback.duration_since(&ts.capture).map(|d| {
        let us = d.as_micros();
        us.min(u128::from(u32::MAX)) as u32
    })
}

fn error_callback(
    slot: Arc<Mutex<Option<String>>>,
) -> impl FnMut(cpal::StreamError) + Send + 'static {
    move |err| {
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(err.to_string());
        }
    }
}

trait SampleWord: Copy + Send + 'static {
    fn as_f32(self) -> f32;
    fn to_device(sample: f32) -> Self;
}

impl SampleWord for f32 {
    fn as_f32(self) -> f32 {
        self
    }
    fn to_device(sample: f32) -> Self {
        if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }
}

impl SampleWord for i16 {
    fn as_f32(self) -> f32 {
        self as f32 / 32_768.0
    }
    fn to_device(sample: f32) -> Self {
        crate::f32_to_i16(sample)
    }
}

impl SampleWord for u16 {
    fn as_f32(self) -> f32 {
        (self as f32 - 32_768.0) / 32_768.0
    }
    fn to_device(sample: f32) -> Self {
        let s = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        (s * 32_767.0 + 32_768.0).round() as u16
    }
}

fn for_chunks<T: SampleWord>(
    input: &[T],
    channels: usize,
    scratch: &mut Vec<f32>,
    mut on_mono: impl FnMut(&[f32]),
) {
    if channels == 0 || input.is_empty() {
        return;
    }
    let frames = input.len() / channels;
    let chunk = scratch.capacity().max(1);
    let mut off = 0;
    while off < frames {
        let n = (frames - off).min(chunk);
        scratch.clear();
        for frame in input[off * channels..(off + n) * channels].chunks_exact(channels) {
            let sum: f32 = frame.iter().copied().map(SampleWord::as_f32).sum();
            scratch.push(sum / channels as f32);
        }
        on_mono(scratch);
        off += n;
    }
}

fn pick_config(
    device: &cpal::Device,
    output: bool,
    want_rate: u32,
) -> Result<SupportedStreamConfig, AudioError> {
    let ranges = if output {
        device
            .supported_output_configs()
            .map_err(|err| AudioError::Device(err.to_string()))?
            .collect::<Vec<_>>()
    } else {
        device
            .supported_input_configs()
            .map_err(|err| AudioError::Device(err.to_string()))?
            .collect::<Vec<_>>()
    };
    if let Some(cfg) = choose_range(&ranges, want_rate) {
        return Ok(cfg);
    }
    let fallback = if output {
        device.default_output_config()
    } else {
        device.default_input_config()
    };
    fallback.map_err(|err| AudioError::Device(err.to_string()))
}

fn choose_range(
    ranges: &[SupportedStreamConfigRange],
    want_rate: u32,
) -> Option<SupportedStreamConfig> {
    let mut best: Option<(u8, usize)> = None;
    for (index, range) in ranges.iter().enumerate() {
        let fmt_rank = match range.sample_format() {
            SampleFormat::F32 => 0,
            SampleFormat::I16 => 1,
            SampleFormat::U16 => 2,
            _ => continue,
        };
        let ch_rank = if range.channels() == 1 { 0 } else { 1 };
        let min = range.min_sample_rate().0;
        let max = range.max_sample_rate().0;
        let rate_rank: u8 = if want_rate >= min && want_rate <= max {
            0
        } else {
            1
        };
        let rank = rate_rank * 8 + fmt_rank * 2 + ch_rank;
        if best.map(|(prev, _)| rank < prev).unwrap_or(true) {
            best = Some((rank, index));
        }
    }
    let (_, index) = best?;
    let range = ranges[index];
    let min = range.min_sample_rate().0;
    let max = range.max_sample_rate().0;
    let rate = if want_rate >= min && want_rate <= max {
        want_rate
    } else if want_rate < min {
        min
    } else {
        max
    };
    Some(range.with_sample_rate(SampleRate(rate)))
}

fn direction_rate(device: &cpal::Device, output: bool) -> (bool, Option<u32>) {
    let result = if output {
        device.default_output_config()
    } else {
        device.default_input_config()
    };
    match result {
        Ok(cfg) => (true, Some(cfg.sample_rate().0)),
        Err(_) => (false, None),
    }
}

fn find_device(
    host: &cpal::Host,
    want: Option<&str>,
    output: bool,
) -> Result<cpal::Device, AudioError> {
    if let Some(want) = want {
        let needle = want.to_ascii_lowercase();
        let devices = host
            .devices()
            .map_err(|err| AudioError::Device(err.to_string()))?;
        for device in devices {
            let name = device.name().unwrap_or_default();
            if !name.to_ascii_lowercase().contains(&needle) {
                continue;
            }
            let (ok, _) = direction_rate(&device, output);
            if ok {
                return Ok(device);
            }
        }
        return Err(AudioError::Device(format!(
            "no {} device matching {want}",
            if output { "output" } else { "input" }
        )));
    }
    let device = if output {
        host.default_output_device()
    } else {
        host.default_input_device()
    };
    device.ok_or_else(|| {
        AudioError::Device(format!(
            "no default {} device",
            if output { "output" } else { "input" }
        ))
    })
}

fn platform_host() -> Result<cpal::Host, AudioError> {
    #[cfg(target_os = "windows")]
    {
        cpal::host_from_id(cpal::HostId::Wasapi).map_err(|err| AudioError::Host(err.to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        cpal::host_from_id(cpal::HostId::Alsa).map_err(|err| AudioError::Host(err.to_string()))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Ok(cpal::default_host())
    }
}

fn is_pulse_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("pulse") || n.contains("pipewire")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_build_lists_its_host_backend() {
        let names = available_host_names();
        #[cfg(target_os = "linux")]
        assert!(
            names.iter().any(|n| n.eq_ignore_ascii_case("alsa")),
            "hosts {names:?}"
        );
        #[cfg(target_os = "windows")]
        assert!(
            names.iter().any(|n| n.eq_ignore_ascii_case("wasapi")),
            "hosts {names:?}"
        );
        assert_eq!(
            compiled_host(),
            if cfg!(target_os = "linux") {
                "alsa"
            } else if cfg!(target_os = "windows") {
                "wasapi"
            } else {
                "default"
            }
        );
    }

    #[test]
    fn host_report_does_not_panic_without_a_sound_card() {
        let report = host_report();
        assert_eq!(report.host, compiled_host());
        let _ = report.pulse_plugin;
        let _ = report.error;
        let _ = report.devices.len();
    }
}
