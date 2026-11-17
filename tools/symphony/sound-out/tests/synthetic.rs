// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Synthetic signals only. No voice audio is read or written. No weights
//! are downloaded. The Chatterbox process is the protocol stub.

use galion_sound_out::contract::MomentFixture;
use galion_sound_out::environment::{synthetic_burst, Environment, Room};
use galion_sound_out::piper::{stand_in_samples, PiperBackend};
use galion_sound_out::request::CloneRequest;
use galion_sound_out::session::Session;
use galion_sound_out::{
    choose, render, Burst, ChatterboxBackend, ClonerBackend, DeviceKind, MoshiStub, SAMPLE_RATE,
    SAMPLES_PER_FRAME,
};

fn rms(x: &[f64]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let mean = x.iter().map(|s| s * s).sum::<f64>() / x.len() as f64;
    mean.sqrt()
}

#[test]
fn cpu_is_the_default_and_an_explicit_cpu_wins() {
    let none = choose(None, false, false);
    assert_eq!(none.used, DeviceKind::Cpu);
    assert_eq!(none.cbx_ep, "cpu");
    assert!(!none.gpu_present);

    let forced = choose(Some("cpu"), true, false);
    assert_eq!(forced.requested, DeviceKind::Cpu);
    assert_eq!(forced.used, DeviceKind::Cpu);

    let gpu = choose(None, true, false);
    assert_eq!(gpu.used, DeviceKind::Cuda);
    assert_eq!(gpu.cbx_ep, "cuda");

    let rocm = choose(None, false, true);
    assert_eq!(rocm.requested, DeviceKind::Rocm);
    assert_eq!(rocm.used, DeviceKind::Cpu);
    assert_eq!(rocm.cbx_ep, "cpu");
}

#[test]
fn worker_line_carries_exaggeration_cfg_temperature_pitch_energy_and_pause() {
    let mut req = CloneRequest::from_text("Hello there.");
    req.exaggeration = 0.4;
    req.cfg_weight = 0.2;
    req.temperature = 0.7;
    req.prosody.pitch_st = 1.5;
    req.prosody.energy = 0.8;
    req.prosody.pre_pause_ms = 80;
    req.prosody.rate = 0.9;
    let line = req.worker_line();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!((v["exaggeration"].as_f64().unwrap() - 0.4).abs() < 1e-9);
    assert!((v["cfg_weight"].as_f64().unwrap() - 0.2).abs() < 1e-9);
    assert!((v["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-9);
    assert!((v["prosody"]["pitch_st"].as_f64().unwrap() - 1.5).abs() < 1e-9);
    assert!((v["prosody"]["energy"].as_f64().unwrap() - 0.8).abs() < 1e-9);
    assert_eq!(v["prosody"]["pre_pause_ms"].as_u64().unwrap(), 80);
    assert!((v["prosody"]["rate"].as_f64().unwrap() - 0.9).abs() < 1e-9);
    assert_eq!(v["device"], "cpu");
    assert_eq!(v["denoise"], false);
}

#[test]
fn moment_fixture_is_the_local_contract_stand_in() {
    let raw = r#"{
        "text": "Hello there.",
        "category": "embarrassment",
        "arousal": 0.2,
        "energy": 0.4,
        "fatigue": 0.0,
        "environment": {"room": "large", "noise_db": 20, "listener_distance_m": 2.0},
        "controls": {
            "exaggeration": 0.3,
            "cfg": 0.1,
            "temperature": 0.9,
            "prosody": {"pitch_st": -1.0, "energy": 0.7, "pre_pause_ms": 40, "rate": 1.1}
        }
    }"#;
    let moment: MomentFixture = serde_json::from_str(raw).unwrap();
    assert_eq!(moment.category, "embarrassment");
    assert_eq!(moment.environment.room, Room::Large);
    let req = moment.clone_request();
    assert!((req.cfg_weight - 0.1).abs() < 1e-9);
    assert!((req.exaggeration - 0.3).abs() < 1e-9);
    assert_eq!(req.prosody.pre_pause_ms, 40);
}

#[test]
fn piper_stand_in_is_realtime_and_streams_frames() {
    let mut backend = PiperBackend::stand_in();
    let caps = backend.capabilities();
    assert!(caps.realtime_on_cpu);
    assert!(caps.streaming);
    assert!(caps.stub);
    assert!(!caps.weights_used);

    let mut req = CloneRequest::from_text("one two three four five");
    req.prosody.rate = 0.8;
    req.prosody.pre_pause_ms = 80;
    let mut chunks = Vec::new();
    let meta = backend
        .speak_stream(&req, &mut |chunk| {
            chunks.push(chunk.samples.len());
            Ok(())
        })
        .unwrap();
    assert!(chunks.len() >= 2, "expected more than one 80 ms frame, got {}", chunks.len());
    assert_eq!(chunks[0], SAMPLES_PER_FRAME);
    assert!(meta.realtime_on_cpu);
    assert!(!meta.weights_used);

    let fast = stand_in_samples(&CloneRequest::from_text("one two three four"));
    let mut slow = CloneRequest::from_text("one two three four");
    slow.prosody.rate = 0.8;
    let slow = stand_in_samples(&slow);
    assert!(slow.len() > fast.len());

    let mut quiet = CloneRequest::from_text("one two");
    quiet.prosody.energy = 0.6;
    let loud = stand_in_samples(&CloneRequest::from_text("one two"));
    let quiet = stand_in_samples(&quiet);
    assert!(rms(&quiet) < rms(&loud));
}

#[test]
fn moshi_stub_streams_and_loads_no_weights() {
    let mut backend = MoshiStub;
    let caps = backend.capabilities();
    assert!(caps.stub);
    assert!(!caps.weights_used);
    assert!(!caps.realtime_on_cpu);
    assert!(caps.streaming);
    let req = CloneRequest::from_text("a short reply for the stub");
    let mut n = 0u32;
    let meta = backend
        .speak_stream(&req, &mut |chunk| {
            n += 1;
            assert!(chunk.samples.len() <= SAMPLES_PER_FRAME);
            Ok(())
        })
        .unwrap();
    assert!(n >= 1);
    assert!(meta.stub);
    assert!(!meta.weights_used);
}

#[test]
fn chatterbox_stub_reads_energy_pause_pitch_and_the_model_knobs() {
    let mut backend = ChatterboxBackend::stub();
    let mut plain = CloneRequest::from_text("Hello there.");
    plain.prosody.energy = 1.0;
    let (base, meta) = backend.speak(&plain).unwrap();
    let echo = meta.controls_echo.expect("controls line");
    assert!(echo.contains("exaggeration=0.0000"));
    assert!(echo.contains("cfg_weight=0.0000"));
    assert!(echo.contains("temperature=0.8000"));
    assert!(echo.contains("energy=1.0000"));
    assert!(echo.contains("pre_pause_ms=0"));
    assert!(!meta.weights_used);

    let mut loud = CloneRequest::from_text("Hello there.");
    loud.prosody.energy = 1.2;
    loud.exaggeration = 0.5;
    loud.cfg_weight = 0.25;
    loud.temperature = 1.0;
    loud.prosody.pre_pause_ms = 80;
    loud.prosody.pitch_st = 2.0;
    let (shaped, meta) = backend.speak(&loud).unwrap();
    let echo = meta.controls_echo.unwrap();
    assert!(echo.contains("exaggeration=0.5000"), "{echo}");
    assert!(echo.contains("cfg_weight=0.2500"), "{echo}");
    assert!(echo.contains("temperature=1.0000"), "{echo}");
    assert!(echo.contains("pitch_st=2.0000"), "{echo}");
    assert!(echo.contains("energy=1.2000"), "{echo}");
    assert!(echo.contains("pre_pause_ms=80"), "{echo}");

    let pause_n = ((80 * SAMPLE_RATE) / 1000) as usize;
    assert!(shaped.len() > pause_n);
    assert!(shaped[..pause_n].iter().all(|s| s.abs() < 1e-6));
    let voiced = &shaped[pause_n..];
    assert!(rms(voiced) > rms(&base) * 1.05, "energy should raise the stub");
    assert_ne!(shaped.len(), base.len() + pause_n as usize, "pitch resample changes length");
}

#[test]
fn breath_frames_leave_before_speech_arrives() {
    let moment = MomentFixture::new("Hello there friend.");
    let mut session = Session::open(&moment, &[]).unwrap();
    let early = session.drain_frames();
    assert!(!early.is_empty(), "the inhale is known before speech");
    assert!(rms(&early[0]) > 1e-4, "the first frame is an audible breath");
    assert!(rms(&early[0]) < 0.2);
    let tone = vec![0.2; SAMPLES_PER_FRAME * 4];
    session.push_speech(&tone);
    let _ = session.finish();
}

#[test]
fn bursts_and_the_room_change_the_mix() {
    let mut moment = MomentFixture::new("Hello there friend.");
    moment.environment = Environment {
        room: Room::Quiet,
        noise_db: 0.0,
        listener_distance_m: 1.0,
        baseline_distance_m: 1.0,
    };
    let burst = synthetic_burst("amusement", 0.0, 0.08);
    let mut piper = PiperBackend::stand_in();
    let with_burst = render(&moment, &mut piper, &[burst], Some("cpu")).unwrap();
    assert!(with_burst.breath_events >= 1);
    assert_eq!(with_burst.burst_count, 1);
    assert!(with_burst.stubbed.iter().any(|s| s.contains("S1")));
    assert!(with_burst.stubbed.iter().any(|s| s.contains("S5")));

    let mut piper = PiperBackend::stand_in();
    let quiet = render(&moment, &mut piper, &[], Some("cpu")).unwrap();
    assert!((quiet.environment.voice_gain - 1.0).abs() < 1e-9);

    moment.environment.room = Room::Large;
    moment.environment.listener_distance_m = 4.0;
    let mut piper = PiperBackend::stand_in();
    let far = render(&moment, &mut piper, &[], Some("cpu")).unwrap();
    assert!(far.environment.voice_gain < 0.5);
    assert!(rms(&far.samples) < rms(&quiet.samples));

    moment.environment.room = Room::Quiet;
    moment.environment.listener_distance_m = 1.0;
    moment.environment.noise_db = 40.0;
    let mut piper = PiperBackend::stand_in();
    let noisy = render(&moment, &mut piper, &[], Some("cpu")).unwrap();
    assert!(noisy.environment.noise_rms > 0.0);
    assert!(noisy.environment.noise_rms <= 10f64.powf(-24.0 / 20.0) + 1e-9);
}

#[test]
fn a_burst_is_audible_in_its_window() {
    let moment = MomentFixture::new("Hello.");
    let mut session = Session::open(&moment, &[]).unwrap();
    let known = session.known_until();
    assert!(known >= SAMPLES_PER_FRAME);
    let burst = Burst {
        category: "amusement".into(),
        start_s: 0.0,
        samples: vec![0.4; 200],
        sample_rate: SAMPLE_RATE as f64,
    };
    let mut with = Session::open(&moment, &[burst]).unwrap();
    let dry = session.drain_frames();
    let wet = with.drain_frames();
    assert!(rms(&wet[0]) > rms(&dry[0]));
}

#[cfg(feature = "real-backends")]
#[test]
fn real_backends_do_not_download_weights() {
    match galion_sound_out::ChatterboxBackend::real() {
        Ok(_) => {
            // Weights were already on disk. Do not synthesize and do not fetch.
        }
        Err(err) => {
            let err = err.to_lowercase();
            assert!(err.contains("not on disk") || err.contains("not downloaded"), "{err}");
        }
    }
    match galion_sound_out::PiperBackend::real_if_present() {
        Ok(_) => {}
        Err(err) => {
            let err = err.to_lowercase();
            assert!(
                err.contains("not on disk") || err.contains("unset") || err.contains("not downloaded"),
                "{err}"
            );
        }
    }
}
