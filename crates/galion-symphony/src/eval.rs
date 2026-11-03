// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Self-eval for the translator. A missing owner file is BLOCKED, not a pass.

use crate::cpu::{self, HostId};
use crate::persona::{ModelId, PersonaId};
use crate::pins;
use crate::safetensors::{self, StDtype};
use crate::sha;
use crate::translate::{self, TranslateOpts};
use crate::wav;
use crate::{load, Error, LANE_HZ};
use galion_emotion::{DefaultConsentGate, Purpose};
use galion_heartbeat::{FrameIn, Heartbeat, HeartbeatConfig, FRAME_SAMPLES};
use galion_tensor::gq::Prefault;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Blocked,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Blocked => "BLOCKED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gate {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub gates: Vec<Gate>,
    pub translate_us: u128,
    pub load_us: u128,
}

impl Report {
    pub fn fails(&self) -> usize {
        self.gates
            .iter()
            .filter(|g| g.status == Status::Fail)
            .count()
    }

    pub fn infra_pass(&self) -> bool {
        self.fails() == 0
            && self
                .gates
                .iter()
                .any(|g| g.name == "gq-roundtrip" && g.status == Status::Pass)
            && self
                .gates
                .iter()
                .any(|g| g.name == "clip-abi" && g.status == Status::Pass)
    }

    pub fn markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# Symphony self-eval\n\n");
        s.push_str("Maciej Grajczyk · 2026-11-03 Europe/Zagreb · t287\n\n");
        s.push_str("Section: Rust translation of the clone interchange into the Galion voice pack and clip ABI.\n\n");
        s.push_str("| Gate | Status | Detail |\n|---|---|---|\n");
        for g in &self.gates {
            let detail = g.detail.replace('|', "/");
            s.push_str(&format!(
                "| {} | {} | {} |\n",
                g.name,
                g.status.as_str(),
                detail
            ));
        }
        s.push('\n');
        s.push_str(&format!(
            "Infra: {}\n\nFails: {}\n\nFixture translate: {} µs. Fixture load: {} µs. The 80 ms bar is the heartbeat frame (`late` pauses). It is not the wall clock of packing a weight file.\n\n",
            if self.infra_pass() { "PASS" } else { "FAIL" },
            self.fails(),
            self.translate_us,
            self.load_us
        ));
        s.push_str("Signed: Maciej Grajczyk\n");
        s
    }
}

fn gate(name: &'static str, status: Status, detail: impl Into<String>) -> Gate {
    Gate {
        name,
        status,
        detail: detail.into(),
    }
}

pub fn run() -> Report {
    let dir = std::env::temp_dir().join(format!("galion-symphony-eval-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let input = dir.join("in");
    let output = dir.join("out");
    let wav = dir.join("jarvis-fixture.wav");
    let built = build_fixture(&input, &wav);
    let mut gates = Vec::new();
    if let Err(e) = &built {
        gates.push(gate("fixture", Status::Fail, e.to_string()));
        let _ = std::fs::remove_dir_all(&dir);
        return Report {
            gates,
            translate_us: 0,
            load_us: 0,
        };
    }
    let t0 = Instant::now();
    let translated = translate::translate(&TranslateOpts {
        input: input.clone(),
        output: output.clone(),
        persona: PersonaId::Jarvis,
        model: ModelId::Nano,
        host: HostId::Cx43,
        wav: Some(wav.clone()),
        fixture: true,
        force: false,
    });
    let translate_us = t0.elapsed().as_micros();
    let mut load_us = 0u128;
    match translated {
        Err(e) => gates.push(gate("gq-roundtrip", Status::Fail, e.to_string())),
        Ok(_) => {
            let t1 = Instant::now();
            match load::open(&output, Prefault::WillNeed) {
                Err(e) => gates.push(gate("gq-roundtrip", Status::Fail, e.to_string())),
                Ok(loaded) => {
                    load_us = t1.elapsed().as_micros();
                    gates.extend(check_pack(&loaded, &wav, &output));
                }
            }
        }
    }
    gates.push(foreign_gate(&dir));
    gates.push(cpu_gate());
    gates.push(pins_gate());
    gates.push(heartbeat_gate());
    gates.push(landing(
        "landing-samantha-wav",
        "docs/galion-voice-v2/personas/refs/samantha.wav",
        PersonaId::Samantha.ref_expect().sha256,
    ));
    gates.push(landing(
        "landing-jarvis-wav",
        "docs/galion-voice-v2/personas/refs/jarvis.wav",
        PersonaId::Jarvis.ref_expect().sha256,
    ));
    gates.push(landing(
        "landing-clone-weights",
        "weights/clone/shared/ve.safetensors",
        pins::PINS
            .iter()
            .find(|p| p.rel.ends_with("ve.safetensors"))
            .map(|p| p.sha256)
            .unwrap_or(""),
    ));
    gates.push(gate(
        "clone-generate",
        Status::Blocked,
        "this crate does not spawn the clone child and does not call generate; weights are not in the tree",
    ));
    let _ = std::fs::remove_dir_all(&dir);
    Report {
        gates,
        translate_us,
        load_us,
    }
}

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn build_fixture(input: &Path, wav: &Path) -> std::result::Result<(), Error> {
    let f32s: Vec<u8> = [0.5f32, -1.0, 0.25, 2.0]
        .into_iter()
        .flat_map(|x| x.to_le_bytes())
        .collect();
    let bf: Vec<u8> = [1.0f32, -2.0]
        .into_iter()
        .flat_map(|x| ((x.to_bits() >> 16) as u16).to_le_bytes())
        .collect();
    safetensors::write_safetensors(
        &input.join("shared/ve.safetensors"),
        &[("encoder.weight", StDtype::F32, &[4], &f32s)],
    )?;
    safetensors::write_safetensors(
        &input.join("shared/s3gen.safetensors"),
        &[("flow.weight", StDtype::BF16, &[2], &bf)],
    )?;
    safetensors::write_safetensors(
        &input.join("shared/s3gen_meanflow.safetensors"),
        &[("mean.bias", StDtype::F32, &[2, 2], &f32s)],
    )?;
    safetensors::write_safetensors(
        &input.join("nano/t3_nano_v1.safetensors"),
        &[("block.0.attn.q", StDtype::F32, &[4], &f32s)],
    )?;
    let samples = [20_000i16, 1, 0, -4];
    wav::write_pcm16(
        wav,
        48_000,
        2,
        &samples,
        &[(*b"hub ", b"clicks,alarms,ambience")],
    )?;
    Ok(())
}

fn check_pack(loaded: &load::Loaded, wav: &Path, output: &Path) -> Vec<Gate> {
    let mut gates = Vec::new();
    let raw = std::fs::read(wav).unwrap_or_default();
    let mut roundtrip_ok = true;
    let mut roundtrip_detail =
        String::from("GqFile::verify_all and payload bytes match the fixture");
    if let Some(entry) = loaded
        .tensors
        .iter()
        .find_map(|g| g.entry("sym.voice_encoder.encoder_weight"))
    {
        if entry.dtype != galion_tensor::gq::GqDType::F32 {
            roundtrip_ok = false;
            roundtrip_detail = "voice encoder dtype".into();
        }
    } else {
        roundtrip_ok = false;
        roundtrip_detail = "missing sym.voice_encoder.encoder_weight".into();
    }
    for g in &loaded.tensors {
        if g.verify_all().is_err() {
            roundtrip_ok = false;
            roundtrip_detail = "verify_all".into();
        }
        if let Ok(Some(view)) = g.f32_view("sym.voice_encoder.encoder_weight") {
            let expect = [0.5f32, -1.0, 0.25, 2.0];
            if view != expect {
                roundtrip_ok = false;
                roundtrip_detail = "f32 bytes differ".into();
            }
        }
    }
    gates.push(gate(
        "gq-roundtrip",
        if roundtrip_ok {
            Status::Pass
        } else {
            Status::Fail
        },
        roundtrip_detail,
    ));

    let names_ok = loaded
        .manifest
        .tensors
        .iter()
        .all(|t| t.name.starts_with("sym.") && t.source != t.name);
    gates.push(gate(
        "namespace",
        if names_ok { Status::Pass } else { Status::Fail },
        "every .gq name is in sym.; the interchange name stays in the manifest",
    ));

    let dirty = walk_has_foreign(output);
    gates.push(gate(
        "output-clean",
        if dirty { Status::Fail } else { Status::Pass },
        "pack directory holds .gq, SYMPHONY.txt, SHA256SUMS, and clip.gclip",
    ));

    match &loaded.clip {
        Some(clip)
            if clip.payload() == raw.as_slice()
                && clip.header.texture_strip == 0
                && clip.header.channels == 2 =>
        {
            let pcm = clip.interleaved_i16().unwrap_or_default();
            let distinct = pcm.len() == 4 && pcm[0] != pcm[1];
            let marker = b"clicks,alarms,ambience";
            let bed = clip.payload().windows(marker.len()).any(|w| w == marker);
            gates.push(gate(
                "clip-abi",
                if distinct && bed {
                    Status::Pass
                } else {
                    Status::Fail
                },
                "GALIONCL payload is the WAV; stereo samples stay distinct; hub chunk kept",
            ));
            gates.push(gate(
                "texture-keep",
                if loaded.manifest.texture == crate::persona::TexturePolicy::Keep && bed {
                    Status::Pass
                } else {
                    Status::Fail
                },
                "jarvis policy keep; clicks, alarms, ambience declared; strip flag 0",
            ));
            let lane_ok = loaded.manifest.clip.as_ref().map(|c| c.sample_rate) == Some(48_000)
                && loaded.tensors.iter().all(|_| true);
            let manifest_rate = {
                let text = std::fs::read_to_string(output.join("SYMPHONY.txt")).unwrap_or_default();
                text.lines().any(|l| l == "sample_rate 24000") && clip.header.sample_rate != LANE_HZ
            };
            gates.push(gate(
                "clip-not-lane",
                if lane_ok && manifest_rate {
                    Status::Pass
                } else {
                    Status::Fail
                },
                "clip stays 48 kHz stereo; the heartbeat lane stays 24 kHz",
            ));
        }
        _ => {
            gates.push(gate(
                "clip-abi",
                Status::Fail,
                "clip missing or payload changed",
            ));
            gates.push(gate("texture-keep", Status::Fail, "clip missing"));
            gates.push(gate("clip-not-lane", Status::Fail, "clip missing"));
        }
    }

    let text = std::fs::read_to_string(output.join("SYMPHONY.txt")).unwrap_or_default();
    let presence_ok = text.contains(crate::PRESENCE_SCHEMA)
        && text.contains("bands you,me,feeling")
        && !text.to_ascii_lowercase().contains("engagement")
        && text.contains("scripted_empathy false")
        && text.contains("late_policy pause");
    gates.push(gate(
        "presence",
        if presence_ok {
            Status::Pass
        } else {
            Status::Fail
        },
        "schema galion-presence-report/v1, bands you/me/feeling, no engagement field",
    ));
    gates.push(gate(
        "empathy",
        if text.contains("empathy live-oklch") && text.contains("scripted_empathy false") {
            Status::Pass
        } else {
            Status::Fail
        },
        "live OKLCH weight; scripted empathy false",
    ));
    let clock_ok = text.contains("clock_ms 80")
        && text.contains("frame_samples 1920")
        && text.contains("sample_rate 24000");
    gates.push(gate(
        "clock",
        if clock_ok { Status::Pass } else { Status::Fail },
        "80 ms, 24 kHz, 1920 samples, same constants as galion-heartbeat and galion-ml-voice",
    ));
    gates
}

fn walk_has_foreign(dir: &Path) -> bool {
    let mut bad = false;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                bad |= walk_has_foreign(&path);
            } else {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if name.ends_with(".safetensors") || safetensors::refused_extension(ext) {
                    bad = true;
                }
            }
        }
    }
    bad
}

fn foreign_gate(dir: &Path) -> Gate {
    let bad = dir.join("foreign.safetensors");
    let mut bytes = vec![0u8; 16];
    bytes[0] = b'G';
    bytes[1] = b'G';
    bytes[2] = b'U';
    bytes[3] = b'F';
    if std::fs::write(&bad, &bytes).is_err() {
        return gate("foreign-refused", Status::Fail, "could not write probe");
    }
    let magic_refused = matches!(safetensors::read_safetensors(&bad), Err(Error::Refused(_)));
    let ext_refused =
        safetensors::refused_extension("pt") && safetensors::refused_extension("ggml");
    let _ = std::fs::remove_file(&bad);
    gate(
        "foreign-refused",
        if magic_refused && ext_refused {
            Status::Pass
        } else {
            Status::Fail
        },
        "foreign weight magic and refused extensions do not enter the pack",
    )
}

fn cpu_gate() -> Gate {
    let hosts = cpu::hosts();
    let b = cpu::voice_budget();
    let ok = hosts.len() == 3
        && hosts[0].id == HostId::Epyc9655p
        && hosts[0].cores == Some(96)
        && hosts[0].threads == Some(192)
        && hosts[0].ram_gb == Some(768)
        && hosts[0].mem_bw_gbps == Some(614)
        && hosts[1].id == HostId::Cx23
        && hosts[1].ram_gb == Some(16)
        && hosts[1].ssd_gb == Some(100)
        && hosts[1].price_usd_mo == Some(20)
        && hosts[2].id == HostId::Cx43
        && hosts[2].ssd_gb == Some(2)
        && hosts[2].price_usd_mo == Some(220)
        && hosts[2].ram_gb.is_none()
        && (b.vcpu_asr + b.vcpu_tts + b.vcpu_vad_ser) <= b.vcpu_avg
        && b.vcpu_burst == 4.0
        && b.non_inference_pct_max == 1
        && b.tick_p99_us_ceiling == 800
        && b.ram_gb_lo == 0.65
        && b.ram_gb_hi == 1.15;
    gate(
        "cpu-budgets",
        if ok { Status::Pass } else { Status::Fail },
        "EPYC 9655P, CX23, CX43; session about 1 vCPU, burst 4, 0.65–1.15 GB, non-inference ceiling 1%, tick ceiling 800 µs",
    )
}

fn pins_gate() -> Gate {
    gate(
        "pins-table",
        if pins::matches_script() {
            Status::Pass
        } else {
            Status::Fail
        },
        "five interchange pins match scripts/clone/PINS.sha256",
    )
}

fn heartbeat_gate() -> Gate {
    let mut hb = Heartbeat::new(HeartbeatConfig::default(), DefaultConsentGate::default());
    let pcm = vec![0f32; FRAME_SAMPLES];
    let on = hb.step(&FrameIn {
        you_pcm: &pcm,
        affect: None,
        tension: None,
        consent_bits: 0,
        purpose: Purpose::PersonalAssistant,
        session_open: false,
        me_speaking: false,
        late: false,
    });
    let late = hb.step(&FrameIn {
        you_pcm: &pcm,
        affect: None,
        tension: None,
        consent_bits: 0,
        purpose: Purpose::PersonalAssistant,
        session_open: false,
        me_speaking: false,
        late: true,
    });
    let short_pcm = [0.0f32; 8];
    let short = hb.step(&FrameIn {
        you_pcm: &short_pcm,
        affect: None,
        tension: None,
        consent_bits: 0,
        purpose: Purpose::PersonalAssistant,
        session_open: false,
        me_speaking: false,
        late: false,
    });
    let ok = !on.paused && late.paused && short.paused && FRAME_SAMPLES == 1920;
    gate(
        "heartbeat-pause",
        if ok { Status::Pass } else { Status::Fail },
        "on-time 1920-sample frame runs; a late frame and a short buffer pause",
    )
}

fn landing(name: &'static str, rel: &str, expect_sha: &str) -> Gate {
    let path = repo_path(rel);
    if !path.is_file() {
        return gate(
            name,
            Status::Blocked,
            format!("{rel} is not on this machine"),
        );
    }
    match sha::hash_file(&path) {
        Ok(got) if sha::hex(&got) == expect_sha => gate(
            name,
            Status::Pass,
            format!("{rel} matches the published sha256"),
        ),
        Ok(_) => gate(
            name,
            Status::Fail,
            format!("{rel} is present and its sha256 does not match"),
        ),
        Err(e) => gate(name, Status::Fail, e.to_string()),
    }
}
