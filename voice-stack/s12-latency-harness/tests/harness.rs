// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The harness times trait implementations. These tests use the fake stages
//! and, when the A21 pack is on disk, two warm Piper reps. They do not write
//! audio.

use s12_latency_harness::traits::{AsrStage, AudioOut, ReplyEngine, SpeechEnd};
use s12_latency_harness::{
    fixture_pcm, heartbeat_play_us, ms_text, percentile, rank, resample_to_frame, run_fake,
    run_piper, EnergyVad, FakeAsr, FakeReply, FrameSink, HarnessError, Machine, Pad, Rep, Report,
    StageNames, TurnOut, BAR_US, FRAME_SAMPLES, PHRASE, PIPER_HZ, SAMPLE_RATE_HZ,
};

fn names() -> StageNames {
    StageNames {
        vad: "energy-rms-80ms".into(),
        asr: "fake-rms-scan".into(),
        reply: "fake-horbm-slot".into(),
        tts: "fake-sine-frame".into(),
        audio_out: "heartbeat-frame-sink".into(),
    }
}

fn rep(asr: u64) -> Rep {
    Rep {
        vad_us: 5,
        eos_to_asr_us: asr,
        asr_to_reply_us: 7,
        reply_to_tts_us: 11,
        tts_to_audio_out_us: 13,
        eos_to_audio_out_us: asr + 7 + 11 + 13,
        eos_to_heartbeat_play_us: asr + 7 + 11,
        mood_ticks: 0,
        sink_peak_milli: 180,
    }
}

fn braces_balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let mut sq = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for c in s.chars() {
        if in_str {
            if esc {
                esc = false;
                continue;
            }
            if c == '\\' {
                esc = true;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => depth -= 1,
            '[' => sq += 1,
            ']' => sq -= 1,
            _ => {}
        }
        if depth < 0 || sq < 0 {
            return false;
        }
    }
    depth == 0 && sq == 0 && !in_str
}

#[test]
fn percentiles_match_the_heartbeat_rank() {
    let reps: Vec<Rep> = [10u64, 20, 30, 40, 100].into_iter().map(rep).collect();
    let report = Report::from_reps("fake", Machine::fixture(), names(), reps);
    let asr = report
        .summary
        .iter()
        .find(|s| s.key == "eos_to_asr_us")
        .unwrap();
    let samples = [10u64, 20, 30, 40, 100];
    assert_eq!(asr.p50_us, percentile(&samples, 50, 100));
    assert_eq!(asr.p95_us, percentile(&samples, 95, 100));
    assert_eq!(asr.p99_us, percentile(&samples, 99, 100));
    assert_eq!(asr.p50_us, 30);
    assert_eq!(rank(5, 99, 100), 4);
    assert!(asr.p99_us == samples[4]);
    let json = report.to_json();
    let md = report.to_markdown();
    assert!(braces_balanced(&json), "json braces\n{json}");
    assert!(json.contains("\"p50_us\": 30"));
    assert!(json.contains("\"cpu\": \"test-cpu\""));
    assert!(json.contains("\"commit\": \"abc\""));
    assert!(json.contains("\"date_utc\": \"2026-11-21T00:00:00Z\""));
    assert!(json.contains("\"product_claim\": false"));
    assert!(md.contains(&format!(
        "| end of speech to ASR final | measured | {} | {} |",
        ms_text(10),
        ms_text(30)
    )));
    assert!(md.contains("test-cpu"));
    assert!(md.contains("abc"));
    // The bar flags are computed from the same integers the table prints.
    assert_eq!(asr.p50_within_bar(), asr.p50_us <= BAR_US);
    assert!(json.contains("\"p50_within_bar\": true"));
}

#[test]
fn an_unmeasured_report_has_no_percentiles() {
    let report = Report::unmeasured("piper", "A21 Piper pack is not on disk", Machine::fixture());
    let json = report.to_json();
    let md = report.to_markdown();
    assert!(braces_balanced(&json), "{json}");
    assert!(json.contains("\"measured\": false"));
    assert!(!json.contains("p50_us"), "{json}");
    assert!(!json.contains("\"summary\""), "{json}");
    assert!(md.contains("No percentile table"));
    assert!(md.contains("not on disk"));
    assert!(json.contains("\"cpu\": \"test-cpu\""));
}

#[test]
fn fake_pipeline_stamps_are_ordered_and_recomputable() {
    let report = run_fake(8, 1).expect("fake backend");
    assert!(report.measured);
    assert_eq!(report.reps.len(), 8);
    assert!(!report.tts_pinned_voice);
    let json = report.to_json();
    assert!(json.contains("\"product_claim\": false"));
    assert!(json.contains("\"backend\": \"fake\""));
    assert!(json.contains("fake-horbm-slot"));
    assert!(json.contains("fake-rms-scan"));
    for rep in &report.reps {
        // Each stamp is truncated to microseconds on its own, so the parts can
        // miss the direct end-to-end clock by a few microseconds.
        let sum =
            rep.eos_to_asr_us + rep.asr_to_reply_us + rep.reply_to_tts_us + rep.tts_to_audio_out_us;
        assert!(
            sum.abs_diff(rep.eos_to_audio_out_us) <= 8,
            "chain {sum} e2e {}",
            rep.eos_to_audio_out_us
        );
        let first_tts = rep.eos_to_asr_us + rep.asr_to_reply_us + rep.reply_to_tts_us;
        assert!(rep.eos_to_heartbeat_play_us + 8 >= first_tts);
        assert!(rep.eos_to_audio_out_us + 8 >= first_tts);
        assert!(rep.sink_peak_milli > 0, "the sink frame was silent");
    }
    let asr: Vec<u64> = report.reps.iter().map(|r| r.eos_to_asr_us).collect();
    let mut sorted = asr.clone();
    sorted.sort_unstable();
    let row = report
        .summary
        .iter()
        .find(|s| s.key == "eos_to_asr_us")
        .unwrap();
    assert_eq!(row.p50_us, percentile(&sorted, 50, 100));
    assert_eq!(row.p95_us, percentile(&sorted, 95, 100));
    assert_eq!(row.p99_us, percentile(&sorted, 99, 100));
    assert_eq!(row.n, 8);
    let md = report.to_markdown();
    assert!(md.contains(&ms_text(row.p50_us)));
    assert!(md.contains(&report.machine.cpu));
    assert!(md.contains(&report.machine.commit));
}

#[test]
fn vad_finds_the_tone_and_asr_rejects_silence() {
    let pcm = fixture_pcm(16_000);
    let mut vad = EnergyVad::default();
    let end = SpeechEnd::end_sample(&mut vad, &pcm, 16_000).unwrap();
    // 320 ms at 16 kHz, on an 80 ms frame boundary.
    assert_eq!(end, 5_120);
    let mut asr = FakeAsr;
    assert_eq!(asr.finalize(&pcm[..end], 16_000).unwrap(), "hello there");
    assert!(asr.finalize(&[0i16; 1280], 16_000).is_err());
}

#[test]
fn mood_advances_on_heartbeat_ticks_and_the_sentence_does_not() {
    let mut reply = FakeReply::at(Pad {
        arousal: 1.0,
        valence: 1.0,
        dominance: 1.0,
    });
    reply.on_heartbeat(10 * 80_000);
    let turn: TurnOut = reply.turn("hello there").unwrap();
    assert_eq!(turn.reply, PHRASE);
    assert_eq!(reply.ticks(), 10);
    assert!(turn.mood.arousal < 1.0);
    assert!(turn.mood.arousal > Pad::RESTING.arousal);
    assert!(!turn.reply.to_lowercase().contains("happy"));
    let mut rest = FakeReply::resting();
    rest.on_heartbeat(80_000);
    let again = rest.turn("hello there").unwrap();
    assert!((again.mood.arousal - Pad::RESTING.arousal).abs() < 1e-5);
    assert!((again.mood.valence - Pad::RESTING.valence).abs() < 1e-5);
    assert!((again.mood.dominance - Pad::RESTING.dominance).abs() < 1e-5);
}

#[test]
fn resample_of_a_full_scale_piper_frame_stays_finite() {
    let n = s12_latency_harness::input_samples_for_one_frame(PIPER_HZ);
    let src = vec![32767i16; n];
    let mut out = vec![0.0f32; FRAME_SAMPLES];
    resample_to_frame(&src, PIPER_HZ, &mut out);
    assert!(out.iter().all(|s| s.is_finite()));
    assert!(out.iter().all(|s| *s > 0.99));
    let mut sink = FrameSink::default();
    AudioOut::emit_frame(&mut sink, &out).unwrap();
    assert_eq!(sink.frames, 1);
    assert!(sink.peak_max > 0.99);
    // Equal-rate copy is one heartbeat frame, not a resample stretch.
    let tone = fixture_pcm(SAMPLE_RATE_HZ);
    let mut frame = [0.0f32; FRAME_SAMPLES];
    resample_to_frame(&tone[..FRAME_SAMPLES], SAMPLE_RATE_HZ, &mut frame);
    assert!(frame.iter().any(|s| s.abs() > 0.1));
    let _ = heartbeat_play_us(1_001, 1_000);
}

#[test]
fn piper_warm_reps_when_the_pack_is_on_disk() {
    match run_piper(2, 1) {
        Err(HarnessError::Absent { reason }) => {
            eprintln!("piper pack absent, skip: {reason}");
        }
        Err(e) => panic!("piper pack is present but the run failed: {e}"),
        Ok(report) => {
            assert!(report.measured);
            assert_eq!(report.reps.len(), 2);
            assert!(report.tts_pinned_voice, "the A21 pin did not match");
            assert_eq!(report.voice.as_deref(), Some("en_US-ljspeech-medium"));
            let json = report.to_json();
            assert!(json.contains("\"product_claim\": false"));
            assert!(json.contains("\"tts_pinned_voice\": true"));
            assert!(json.contains("not Samantha"));
            assert!(!json.contains("GALION_SAMANTHA_REF="));
            for rep in &report.reps {
                let sum = rep.eos_to_asr_us
                    + rep.asr_to_reply_us
                    + rep.reply_to_tts_us
                    + rep.tts_to_audio_out_us;
                assert!(
                    sum.abs_diff(rep.eos_to_audio_out_us) <= 8,
                    "chain {sum} e2e {}",
                    rep.eos_to_audio_out_us
                );
                assert!(
                    rep.reply_to_tts_us > 1_000,
                    "first piper byte was {} us, expected a real child",
                    rep.reply_to_tts_us
                );
                assert!(
                    rep.reply_to_tts_us < 5_000_000,
                    "first piper byte took {} us",
                    rep.reply_to_tts_us
                );
                assert!(rep.sink_peak_milli > 0, "piper frame was silent");
            }
            let cold = report.cold.as_ref().expect("cold clocks");
            assert!(cold.spawn_to_first_pcm_us >= cold.warmup_write_to_first_pcm_us);
            assert!(cold.spawn_to_first_pcm_us > 0);
            let md = report.to_markdown();
            assert!(md.contains(&report.machine.cpu));
            assert!(md.contains("derived"));
        }
    }
}
