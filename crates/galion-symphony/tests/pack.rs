// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Pack round-trip from a tiny synthetic buffer.
//! The inputs are bytes in this file. No wav path and no weight file.

use std::path::Path;

use galion_symphony::host_cpu::HostId;
use galion_symphony::load::Prefault;
use galion_symphony::persona::{ModelId, PersonaId, TexturePolicy};
use galion_symphony::translate::{pack_buffers, BufferPack, BufferTensor, PackDType};
use galion_symphony::{open, sha, LANE_HZ, PRESENCE_SCHEMA};

#[test]
fn synthetic_buffer_pack_roundtrips_without_wav_or_weights() {
    let samples = [20_000i16, 1, 0, -4];
    let bed = b"clicks,alarms,ambience";
    let wav =
        galion_symphony::wav::encode_pcm16(48_000, 2, &samples, &[(*b"hub ", bed.as_slice())])
            .unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    let wav_sha = sha::hex(&sha::sha256(&wav));
    assert_ne!(
        wav_sha,
        PersonaId::Jarvis.ref_expect().sha256,
        "synthetic pcm is not the published reference"
    );

    let f32s: Vec<u8> = [0.5f32, -1.0, 0.25, 2.0]
        .into_iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let bf: Vec<u8> = [1.0f32, -2.0]
        .into_iter()
        .flat_map(|v| ((v.to_bits() >> 16) as u16).to_le_bytes())
        .collect();

    let dir = std::env::temp_dir().join(format!("galion-symphony-pack-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.join("pack");
    let manifest = pack_buffers(&BufferPack {
        output: out.clone(),
        persona: PersonaId::Jarvis,
        model: ModelId::Nano,
        host: HostId::Cx43,
        wav: Some(wav.clone()),
        force: false,
        tensors: vec![
            BufferTensor {
                name: "sym.voice_encoder.encoder_weight".into(),
                source: "encoder.weight".into(),
                dtype: PackDType::F32,
                shape: vec![4],
                bytes: f32s,
                file: "tensors/voice_encoder.gq".into(),
            },
            BufferTensor {
                name: "sym.speech_gen.flow_weight".into(),
                source: "flow.weight".into(),
                dtype: PackDType::BF16,
                shape: vec![2],
                bytes: bf.clone(),
                file: "tensors/speech_gen.gq".into(),
            },
        ],
    })
    .unwrap();
    assert_eq!(manifest.tensors.len(), 2);
    assert!(manifest
        .tensors
        .iter()
        .all(|t| t.name.starts_with("sym.") && t.source != t.name));

    let loaded = open(&out, Prefault::WillNeed).unwrap();
    let mut saw_f32 = false;
    let mut saw_bf = false;
    for g in &loaded.tensors {
        g.verify_all().unwrap();
        if g.entry("sym.voice_encoder.encoder_weight").is_some() {
            let view = g
                .f32_view("sym.voice_encoder.encoder_weight")
                .unwrap()
                .expect("aligned f32 view");
            assert_eq!(view, &[0.5f32, -1.0, 0.25, 2.0]);
            saw_f32 = true;
        }
        if let Ok(raw) = g.raw("sym.speech_gen.flow_weight") {
            assert_eq!(raw, bf.as_slice());
            saw_bf = true;
        }
    }
    assert!(saw_f32, "f32 buffer did not round-trip");
    assert!(saw_bf, "bf16 buffer did not round-trip");

    let clip = loaded.clip.as_ref().expect("clip");
    assert_eq!(clip.payload(), wav.as_slice());
    assert_eq!(clip.header.channels, 2);
    assert_eq!(clip.header.sample_rate, 48_000);
    assert_ne!(clip.header.sample_rate, LANE_HZ);
    assert_eq!(clip.header.texture_strip, 0);
    assert_eq!(clip.header.texture, TexturePolicy::Keep);
    assert!(clip.payload().windows(bed.len()).any(|w| w == bed));
    let pcm = clip.interleaved_i16().unwrap();
    assert_eq!(pcm, samples);
    assert_ne!(pcm[0], pcm[1], "left and right stay distinct");

    let text = std::fs::read_to_string(out.join("SYMPHONY.txt")).unwrap();
    assert!(text.lines().any(|l| l == format!("sample_rate {LANE_HZ}")));
    assert!(text.contains("clock_ms 80"));
    assert!(text.contains("frame_samples 1920"));
    assert!(text.contains(PRESENCE_SCHEMA));
    assert!(text.contains("bands you,me,feeling"));
    assert!(text.contains("empathy live-oklch"));
    assert!(text.contains("scripted_empathy false"));
    assert!(text.contains("late_policy pause"));
    assert!(!text.to_ascii_lowercase().contains("engagement"));
    assert_eq!(
        loaded.manifest.clip.as_ref().map(|c| c.sample_rate),
        Some(48_000)
    );

    assert!(
        galion_symphony::pins::matches_script(),
        "pin table must match scripts/clone/PINS.sha256 without opening a weight"
    );
    assert_pack_holds_no_wav_or_weights(&out);
    let _ = std::fs::remove_dir_all(&dir);
}

fn assert_pack_holds_no_wav_or_weights(dir: &Path) {
    let mut files = 0u32;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        for ent in std::fs::read_dir(&cur).unwrap() {
            let path = ent.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            files += 1;
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            assert!(
                name == "SYMPHONY.txt" || name == "SHA256SUMS" || ext == "gq" || ext == "gclip",
                "{name}"
            );
        }
    }
    assert!(files >= 4, "pack files: {files}");
}
