// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! `galion-symphony translate|load|clip|eval|hosts`

#![forbid(unsafe_code)]

use galion_symphony::clip::{self, WrapOpts};
use galion_symphony::cpu::HostId;
use galion_symphony::persona::{ModelId, PersonaId};
use galion_symphony::translate::TranslateOpts;
use galion_symphony::{self_eval, translate};
use galion_tensor::gq::Prefault;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("translate") => translate_cmd(args),
        Some("load") => load_cmd(args),
        Some("clip") => clip_cmd(args),
        Some("eval") => {
            let report = self_eval();
            print!("{}", report.markdown());
            if report.fails() == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Some("hosts") => {
            for h in galion_symphony::cpu::hosts() {
                println!("{}\n  {}", h.line(), h.note);
            }
            println!("{}", galion_symphony::cpu::voice_budget().line());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!(
                "usage:\n  galion-symphony translate --in DIR --out DIR --persona samantha|jarvis --model nano|turbo --host epyc9655p|cx23|cx43 [--wav FILE] [--fixture] [--force]\n  galion-symphony load DIR [--prefault willneed|populate|touch|none]\n  galion-symphony clip wrap --persona ID --wav FILE --out FILE [--check-pin]\n  galion-symphony clip show FILE\n  galion-symphony eval\n  galion-symphony hosts"
            );
            ExitCode::from(2)
        }
    }
}

fn translate_cmd(args: impl Iterator<Item = String>) -> ExitCode {
    let mut input = None;
    let mut output = None;
    let mut persona = None;
    let mut model = None;
    let mut host = HostId::Epyc9655p;
    let mut wav = None;
    let mut fixture = false;
    let mut force = false;
    let mut it = args;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--in" => input = it.next().map(PathBuf::from),
            "--out" => output = it.next().map(PathBuf::from),
            "--persona" => persona = it.next().as_deref().and_then(PersonaId::parse),
            "--model" => model = it.next().as_deref().and_then(ModelId::parse),
            "--host" => {
                host = match it.next().as_deref().and_then(HostId::parse) {
                    Some(h) => h,
                    None => {
                        eprintln!("galion-symphony: host is epyc9655p, cx23, or cx43");
                        return ExitCode::from(2);
                    }
                }
            }
            "--wav" => wav = it.next().map(PathBuf::from),
            "--fixture" => fixture = true,
            "--force" => force = true,
            other => {
                eprintln!("galion-symphony: unknown {other}");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(input), Some(output), Some(persona), Some(model)) = (input, output, persona, model)
    else {
        eprintln!("galion-symphony: translate needs --in --out --persona --model");
        return ExitCode::from(2);
    };
    match translate(&TranslateOpts {
        input,
        output: output.clone(),
        persona,
        model,
        host,
        wav,
        fixture,
        force,
    }) {
        Ok(m) => {
            println!(
                "galion-symphony: wrote {} persona={} model={} tensors={} clip={}",
                output.display(),
                m.persona.as_str(),
                m.model.as_str(),
                m.tensors.len(),
                m.clip.is_some()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("galion-symphony: {e}");
            ExitCode::from(2)
        }
    }
}

fn load_cmd(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(dir) = args.next() else {
        eprintln!("galion-symphony: load needs a pack directory");
        return ExitCode::from(2);
    };
    let mut prefault = Prefault::WillNeed;
    while let Some(a) = args.next() {
        if a == "--prefault" {
            prefault = match args.next().as_deref() {
                Some("willneed") => Prefault::WillNeed,
                Some("populate") => Prefault::Populate,
                Some("touch") => Prefault::Touch,
                Some("none") => Prefault::None,
                _ => {
                    eprintln!("galion-symphony: prefault is willneed, populate, touch, or none");
                    return ExitCode::from(2);
                }
            };
        }
    }
    match galion_symphony::open(Path::new(&dir), prefault) {
        Ok(pack) => {
            println!(
                "galion-symphony: loaded persona={} model={} host={} tensors={} clip={} device=cpu container=gq",
                pack.manifest.persona.as_str(),
                pack.manifest.model.as_str(),
                pack.manifest.host.id.as_str(),
                pack.manifest.tensors.len(),
                pack.clip.is_some()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("galion-symphony: {e}");
            ExitCode::from(1)
        }
    }
}

fn clip_cmd(mut args: impl Iterator<Item = String>) -> ExitCode {
    match args.next().as_deref() {
        Some("wrap") => {
            let mut persona = None;
            let mut wav = None;
            let mut out = None;
            let mut check_pin = false;
            while let Some(a) = args.next() {
                match a.as_str() {
                    "--persona" => persona = args.next().as_deref().and_then(PersonaId::parse),
                    "--wav" => wav = args.next().map(PathBuf::from),
                    "--out" => out = args.next().map(PathBuf::from),
                    "--check-pin" => check_pin = true,
                    other => {
                        eprintln!("galion-symphony: unknown {other}");
                        return ExitCode::from(2);
                    }
                }
            }
            let (Some(persona), Some(wav), Some(out)) = (persona, wav, out) else {
                eprintln!("galion-symphony: clip wrap needs --persona --wav --out");
                return ExitCode::from(2);
            };
            match clip::wrap(WrapOpts {
                persona,
                wav: &wav,
                out: &out,
                check_pin,
            }) {
                Ok(h) => {
                    println!(
                        "galion-symphony: clip {} persona={} {} Hz {} ch strip={} texture={}",
                        out.display(),
                        h.persona.as_str(),
                        h.sample_rate,
                        h.channels,
                        h.texture_strip,
                        h.texture.as_str()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("galion-symphony: {e}");
                    ExitCode::from(2)
                }
            }
        }
        Some("show") => {
            let Some(path) = args.next() else {
                eprintln!("galion-symphony: clip show FILE");
                return ExitCode::from(2);
            };
            match clip::open(path.as_ref()) {
                Ok(c) => {
                    println!(
                        "persona={} texture={} strip={} bits={:#x} {} Hz {} ch {} bit {} ms sha256={}",
                        c.header.persona.as_str(),
                        c.header.texture.as_str(),
                        c.header.texture_strip,
                        c.header.texture_bits,
                        c.header.sample_rate,
                        c.header.channels,
                        c.header.bits,
                        c.header.duration_ms,
                        galion_symphony::sha::hex(&c.header.payload_sha256)
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("galion-symphony: {e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("galion-symphony: clip wrap|show");
            ExitCode::from(2)
        }
    }
}
