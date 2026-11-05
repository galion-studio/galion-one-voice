// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! CPU classes the symphony pack records. Voice stays CPU-only.
//! Session budgets are the same on every class. The class is capacity.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostId {
    Epyc9655p,
    Cx23,
    Cx43,
}

impl HostId {
    pub fn as_str(self) -> &'static str {
        match self {
            HostId::Epyc9655p => "epyc9655p",
            HostId::Cx23 => "cx23",
            HostId::Cx43 => "cx43",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "epyc9655p" | "epyc" => Some(HostId::Epyc9655p),
            "cx23" | "plus" => Some(HostId::Cx23),
            "cx43" | "ultra" => Some(HostId::Cx43),
            _ => None,
        }
    }

    pub fn spec(self) -> HostClass {
        match self {
            HostId::Epyc9655p => HostClass {
                id: self,
                role: "flagship",
                cores: Some(96),
                threads: Some(192),
                ram_gb: Some(768),
                ssd_gb: None,
                mem_bw_gbps: Some(614),
                price_usd_mo: None,
                note: "App and voice node. 1U 1P AMD EPYC 9655P, 768 GB DDR5, air cooled. No GPU. The LLM node on this CPU is 384 GB and is not this voice pack.",
            },
            HostId::Cx23 => HostClass {
                id: self,
                role: "plus",
                cores: None,
                threads: None,
                ram_gb: Some(16),
                ssd_gb: Some(100),
                mem_bw_gbps: None,
                price_usd_mo: Some(20),
                note: "Plus. CX23-class. $20/mo. COGS about $6. 16 GB RAM, 100 GB SSD. Voice-to-voice. CPU only.",
            },
            HostId::Cx43 => HostClass {
                id: self,
                role: "ultra",
                cores: None,
                threads: None,
                ram_gb: None,
                ssd_gb: Some(2),
                mem_bw_gbps: None,
                price_usd_mo: Some(220),
                note: "Ultra. CX43-class, Titanaxe Max 3XL. $220/mo. 2 GB SSD. CPU only.",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostClass {
    pub id: HostId,
    pub role: &'static str,
    pub cores: Option<u32>,
    pub threads: Option<u32>,
    pub ram_gb: Option<u32>,
    pub ssd_gb: Option<u32>,
    pub mem_bw_gbps: Option<u32>,
    pub price_usd_mo: Option<u32>,
    pub note: &'static str,
}

pub fn hosts() -> [HostClass; 3] {
    [
        HostId::Epyc9655p.spec(),
        HostId::Cx23.spec(),
        HostId::Cx43.spec(),
    ]
}

/// One live voice session. Averages from the CPU foundation, not a measured run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceBudget {
    pub vcpu_avg: f32,
    pub vcpu_burst: f32,
    pub vcpu_asr: f32,
    pub vcpu_tts: f32,
    pub vcpu_vad_ser: f32,
    pub ram_gb_lo: f32,
    pub ram_gb_hi: f32,
    /// Non-inference share of the host, percent. The one-percent ceiling.
    pub non_inference_pct_max: u32,
    /// Tick work must stay under this. Late frames pause.
    pub tick_p99_us_ceiling: u32,
}

pub const VOICE_BUDGET: VoiceBudget = VoiceBudget {
    vcpu_avg: 1.0,
    vcpu_burst: 4.0,
    vcpu_asr: 0.4,
    vcpu_tts: 0.3,
    vcpu_vad_ser: 0.15,
    ram_gb_lo: 0.65,
    ram_gb_hi: 1.15,
    non_inference_pct_max: 1,
    tick_p99_us_ceiling: 800,
};

pub fn voice_budget() -> VoiceBudget {
    VOICE_BUDGET
}

impl VoiceBudget {
    pub fn line(self) -> String {
        format!(
            "budget {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} {} {}",
            self.vcpu_avg,
            self.vcpu_burst,
            self.vcpu_asr,
            self.vcpu_tts,
            self.vcpu_vad_ser,
            self.ram_gb_lo,
            self.ram_gb_hi,
            self.non_inference_pct_max,
            self.tick_p99_us_ceiling
        )
    }

    pub fn parse(rest: &str) -> Result<VoiceBudget> {
        let p: Vec<&str> = rest.split_whitespace().collect();
        if p.len() != 9 {
            return Err(Error::Bad("budget line".into()));
        }
        let f = |i: usize| {
            p[i].parse::<f32>()
                .map_err(|_| Error::Bad(format!("budget field {}", p[i])))
        };
        let u = |i: usize| {
            p[i].parse::<u32>()
                .map_err(|_| Error::Bad(format!("budget field {}", p[i])))
        };
        Ok(VoiceBudget {
            vcpu_avg: f(0)?,
            vcpu_burst: f(1)?,
            vcpu_asr: f(2)?,
            vcpu_tts: f(3)?,
            vcpu_vad_ser: f(4)?,
            ram_gb_lo: f(5)?,
            ram_gb_hi: f(6)?,
            non_inference_pct_max: u(7)?,
            tick_p99_us_ceiling: u(8)?,
        })
    }
}

pub fn opt_token(n: Option<u32>) -> String {
    n.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

pub fn parse_opt(s: &str) -> Result<Option<u32>> {
    if s == "-" {
        Ok(None)
    } else {
        s.parse::<u32>()
            .map(Some)
            .map_err(|_| Error::Bad(format!("host field {s}")))
    }
}

impl HostClass {
    pub fn line(self) -> String {
        format!(
            "host_spec {} {} {} {} {} {} {} {}",
            self.id.as_str(),
            self.role,
            opt_token(self.cores),
            opt_token(self.threads),
            opt_token(self.ram_gb),
            opt_token(self.ssd_gb),
            opt_token(self.mem_bw_gbps),
            opt_token(self.price_usd_mo)
        )
    }
}
