//! Quality tiers and frame pacing for Nebula Galion Voice V2.
//!
//! Before this module existed, the repository had no nebula frame-pacing
//! constant at all: no laptop FPS value and no desktop FPS value. Phone,
//! laptop, and desktop share the normal tier below. Pi, high, and ultra are
//! the other pacing modes.

/// How frames are paced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePacing {
    /// Sleep or skip so the period is `1 / fps_target`. Pi and high.
    FixedCap,
    /// Pick one step of [`NORMAL_FPS_LADDER`], never above the display
    /// refresh, then present with vsync or `requestAnimationFrame`.
    AutoLadder,
    /// Mailbox or immediate present when the backend has it, otherwise
    /// vsync. Ultra only.
    UncappedMailbox,
}

/// Software cap for the Pi tier, in frames per second.
pub const PI_FPS_TARGET: f32 = 30.0;
/// Pi presents on a fixed 30 fps cap.
pub const PI_PACING: FramePacing = FramePacing::FixedCap;

/// Lowest automatic target for the normal tier.
pub const NORMAL_FPS_MIN: f32 = 30.0;
/// Highest automatic target for the normal tier.
pub const NORMAL_FPS_MAX: f32 = 120.0;
/// Discrete targets the normal tier may select. Phone, laptop, and desktop
/// all use this ladder unless the user opts into high or ultra.
pub const NORMAL_FPS_LADDER: [f32; 5] = [30.0, 45.0, 60.0, 90.0, 120.0];
/// Normal pacing follows the ladder and the display refresh. It is not a
/// single fixed cap. Laptop and desktop have no separate FPS constant.
pub const NORMAL_PACING: FramePacing = FramePacing::AutoLadder;

/// Software cap for the high tier, in frames per second.
pub const HIGH_FPS_TARGET: f32 = 120.0;
/// High presents on a fixed 120 fps cap.
pub const HIGH_PACING: FramePacing = FramePacing::FixedCap;

/// Opt-in ultra target, in frames per second. Never selected automatically,
/// and never used on an integrated GPU.
pub const ULTRA_FPS_TARGET: f32 = 240.0;
/// Ultra frame-time budget in milliseconds. Particle search may climb only
/// while a frame stays within this budget (`1000 / ULTRA_FPS_TARGET`).
pub const ULTRA_FRAME_BUDGET_MS: f32 = 1000.0 / ULTRA_FPS_TARGET;
/// Ultra prefers mailbox or immediate present and falls back to vsync.
pub const ULTRA_PACING: FramePacing = FramePacing::UncappedMailbox;

/// Pi particle count. Fewest of every tier.
pub const PI_PARTICLES: usize = 2_500;
/// Pi render scale. The framebuffer is half the logical viewport on each axis.
pub const PI_RENDER_SCALE: f32 = 0.5;
/// One cheap bloom pass on Pi. Ultra uses more.
pub const PI_BLOOM_PASSES: u32 = 1;
/// Curl-noise octaves on Pi.
pub const PI_OCTAVES: u32 = 2;

/// Normal-tier particle budgets by device class.
pub const PHONE_PARTICLES: usize = 8_000;
pub const LAPTOP_PARTICLES: usize = 18_000;
pub const DESKTOP_PARTICLES: usize = 32_000;
/// High-tier particle budget. Pacing stays at [`HIGH_FPS_TARGET`].
pub const HIGH_PARTICLES: usize = 48_000;

/// Ultra starts here and searches upward while frame time holds.
pub const ULTRA_PARTICLES_START: usize = 64_000;
/// Hard ceiling of the ultra particle search.
pub const ULTRA_PARTICLES_MAX: usize = 160_000;
/// Ultra native scale before the optional supersample.
pub const ULTRA_RENDER_SCALE: f32 = 1.0;
/// Optional supersample, used only after the particle search still has budget.
pub const ULTRA_SUPERSAMPLE: f32 = 1.5;
/// Enhanced multi-pass bloom on ultra.
pub const ULTRA_BLOOM_PASSES: u32 = 3;
/// Extra scattering taps on ultra.
pub const ULTRA_SCATTER_SAMPLES: u32 = 4;
/// Finer curl-noise stack on ultra.
pub const ULTRA_OCTAVES: u32 = 6;

/// User-selected pacing mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityMode {
    Pi,
    Normal,
    High,
    Ultra,
}

/// Viewport / memory class. This picks a particle budget inside the normal
/// tier. It is not a separate FPS target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    Pi,
    Phone,
    Laptop,
    Desktop,
}

/// What the adapter told us. Ultra requires a discrete GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuClass {
    Discrete,
    Integrated,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TierBudget {
    pub mode: QualityMode,
    pub pacing: FramePacing,
    pub fps_target: f32,
    pub particles: usize,
    pub particle_floor: usize,
    pub particle_ceiling: usize,
    pub render_scale: f32,
    pub max_render_scale: f32,
    pub octaves: u32,
    pub scatter_samples: u32,
    pub bloom_passes: u32,
    pub hdr: bool,
    pub dither: bool,
    pub device: DeviceClass,
}

/// Parse `--quality` / `?quality=`.
///
/// `phone`, `laptop`, and `desktop` are device-class aliases that keep
/// [`QualityMode::Normal`]. They do not introduce an FPS constant.
pub fn parse_quality(raw: &str) -> Option<QualityMode> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pi" => Some(QualityMode::Pi),
        "normal" | "phone" | "laptop" | "desktop" | "auto" => Some(QualityMode::Normal),
        "high" => Some(QualityMode::High),
        "ultra" => Some(QualityMode::Ultra),
        _ => None,
    }
}

pub fn parse_device_alias(raw: &str) -> Option<DeviceClass> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pi" => Some(DeviceClass::Pi),
        "phone" => Some(DeviceClass::Phone),
        "laptop" => Some(DeviceClass::Laptop),
        "desktop" => Some(DeviceClass::Desktop),
        _ => None,
    }
}

/// Highest ladder step that does not sit above the display refresh.
///
/// A 1 Hz tolerance keeps a 59.94 Hz panel on 60 and still refuses 120.
pub fn ladder_at_most(refresh_hz: f32) -> f32 {
    let mut best = NORMAL_FPS_LADDER[0];
    for step in NORMAL_FPS_LADDER {
        if step <= refresh_hz + 1.0 {
            best = step;
        }
    }
    best
}

/// First normal-tier guess from the display and the GPU, before frame time.
pub fn initial_normal_target(refresh_hz: f32, gpu: GpuClass) -> f32 {
    let refresh = if refresh_hz.is_finite() && refresh_hz >= NORMAL_FPS_MIN {
        refresh_hz
    } else {
        60.0
    };
    let capped = refresh.min(NORMAL_FPS_MAX);
    let guess = match gpu {
        GpuClass::Discrete => capped,
        GpuClass::Integrated | GpuClass::Unknown => capped.min(60.0),
    };
    ladder_at_most(guess)
}

/// Ultra is opt-in and discrete-GPU only. An iGPU or an unknown adapter
/// cannot enter it, even when the flag is set.
pub fn resolve_mode(requested: QualityMode, gpu: GpuClass) -> QualityMode {
    if requested == QualityMode::Ultra && gpu != GpuClass::Discrete {
        QualityMode::Normal
    } else {
        requested
    }
}

pub fn device_particles(device: DeviceClass) -> usize {
    match device {
        DeviceClass::Pi => PI_PARTICLES,
        DeviceClass::Phone => PHONE_PARTICLES,
        DeviceClass::Laptop => LAPTOP_PARTICLES,
        DeviceClass::Desktop => DESKTOP_PARTICLES,
    }
}

pub fn budget_for(mode: QualityMode, device: DeviceClass, refresh_hz: f32, gpu: GpuClass) -> TierBudget {
    let mode = resolve_mode(mode, gpu);
    let device = if mode == QualityMode::Pi { DeviceClass::Pi } else { device };
    match mode {
        QualityMode::Pi => TierBudget {
            mode,
            pacing: PI_PACING,
            fps_target: PI_FPS_TARGET,
            particles: PI_PARTICLES,
            particle_floor: 1_200,
            particle_ceiling: PI_PARTICLES,
            render_scale: PI_RENDER_SCALE,
            max_render_scale: PI_RENDER_SCALE,
            octaves: PI_OCTAVES,
            scatter_samples: 1,
            bloom_passes: PI_BLOOM_PASSES,
            hdr: false,
            dither: true,
            device: DeviceClass::Pi,
        },
        QualityMode::Normal => {
            let particles = device_particles(device);
            TierBudget {
                mode,
                pacing: NORMAL_PACING,
                fps_target: initial_normal_target(refresh_hz, gpu),
                particles,
                particle_floor: (particles / 3).max(2_000),
                particle_ceiling: particles,
                render_scale: 1.0,
                max_render_scale: 1.0,
                octaves: match device {
                    DeviceClass::Desktop => 4,
                    DeviceClass::Pi => PI_OCTAVES,
                    _ => 3,
                },
                scatter_samples: 1,
                bloom_passes: 0,
                hdr: false,
                dither: false,
                device,
            }
        }
        QualityMode::High => TierBudget {
            mode,
            pacing: HIGH_PACING,
            fps_target: HIGH_FPS_TARGET,
            particles: HIGH_PARTICLES,
            particle_floor: DESKTOP_PARTICLES,
            particle_ceiling: HIGH_PARTICLES,
            render_scale: 1.0,
            max_render_scale: 1.0,
            octaves: 4,
            scatter_samples: 2,
            bloom_passes: 1,
            hdr: false,
            dither: true,
            device,
        },
        QualityMode::Ultra => TierBudget {
            mode,
            pacing: ULTRA_PACING,
            fps_target: ULTRA_FPS_TARGET,
            particles: ULTRA_PARTICLES_START,
            particle_floor: HIGH_PARTICLES,
            particle_ceiling: ULTRA_PARTICLES_MAX,
            render_scale: ULTRA_RENDER_SCALE,
            max_render_scale: ULTRA_SUPERSAMPLE,
            octaves: ULTRA_OCTAVES,
            scatter_samples: ULTRA_SCATTER_SAMPLES,
            bloom_passes: ULTRA_BLOOM_PASSES,
            hdr: true,
            dither: true,
            device: DeviceClass::Desktop,
        },
    }
}

/// Move the normal-tier target along the ladder. `refresh_hz` is a hard ceiling.
pub fn step_normal_target(current: f32, refresh_hz: f32, faster: bool) -> f32 {
    let cap = ladder_at_most(refresh_hz.max(NORMAL_FPS_MIN));
    let mut idx = 0usize;
    for (i, step) in NORMAL_FPS_LADDER.iter().enumerate() {
        if (*step - current).abs() < 0.5 {
            idx = i;
        }
    }
    let next = if faster { idx.saturating_add(1) } else { idx.saturating_sub(1) };
    let next = next.min(NORMAL_FPS_LADDER.len() - 1);
    NORMAL_FPS_LADDER[next].min(cap)
}

/// Detect a Raspberry Pi from `/proc/device-tree/model` when that file exists.
pub fn is_raspberry_pi() -> bool {
    std::fs::read("/proc/device-tree/model")
        .ok()
        .map(|bytes| {
            let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
            text.contains("raspberry pi")
        })
        .unwrap_or(false)
}

/// `MemTotal` in MiB from `/proc/meminfo`, when the host exposes it.
pub fn mem_total_mib() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

pub fn low_memory() -> bool {
    matches!(mem_total_mib(), Some(mib) if mib <= 2_048)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacing_constants_match_the_revised_tiers() {
        assert_eq!(PI_FPS_TARGET, 30.0);
        assert_eq!(PI_PACING, FramePacing::FixedCap);
        assert_eq!(NORMAL_FPS_MIN, 30.0);
        assert_eq!(NORMAL_FPS_MAX, 120.0);
        assert_eq!(NORMAL_FPS_LADDER, [30.0, 45.0, 60.0, 90.0, 120.0]);
        assert_eq!(NORMAL_PACING, FramePacing::AutoLadder);
        assert_eq!(HIGH_FPS_TARGET, 120.0);
        assert_eq!(HIGH_PACING, FramePacing::FixedCap);
        assert_eq!(ULTRA_FPS_TARGET, 240.0);
        assert!((ULTRA_FRAME_BUDGET_MS - 4.1666665).abs() < 1e-3);
        assert_eq!(ULTRA_PACING, FramePacing::UncappedMailbox);
        assert!(PI_PARTICLES < PHONE_PARTICLES);
        assert!((PI_RENDER_SCALE - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn ladder_never_exceeds_refresh() {
        assert_eq!(ladder_at_most(60.0), 60.0);
        assert_eq!(ladder_at_most(59.94), 60.0);
        assert_eq!(ladder_at_most(50.0), 45.0);
        assert_eq!(ladder_at_most(144.0), 120.0);
        assert_eq!(ladder_at_most(90.0), 90.0);
        assert_eq!(initial_normal_target(144.0, GpuClass::Integrated), 60.0);
        assert_eq!(initial_normal_target(144.0, GpuClass::Discrete), 120.0);
        assert_eq!(initial_normal_target(60.0, GpuClass::Discrete), 60.0);
    }

    #[test]
    fn ultra_never_engages_on_an_igpu() {
        assert_eq!(resolve_mode(QualityMode::Ultra, GpuClass::Integrated), QualityMode::Normal);
        assert_eq!(resolve_mode(QualityMode::Ultra, GpuClass::Unknown), QualityMode::Normal);
        assert_eq!(resolve_mode(QualityMode::Ultra, GpuClass::Discrete), QualityMode::Ultra);
        let forced = budget_for(QualityMode::Ultra, DeviceClass::Desktop, 240.0, GpuClass::Integrated);
        assert_eq!(forced.mode, QualityMode::Normal);
        assert_eq!(forced.pacing, NORMAL_PACING);
        let ultra = budget_for(QualityMode::Ultra, DeviceClass::Laptop, 240.0, GpuClass::Discrete);
        assert_eq!(ultra.fps_target, ULTRA_FPS_TARGET);
        assert!(ultra.hdr);
        assert_eq!(ultra.bloom_passes, ULTRA_BLOOM_PASSES);
        assert_eq!(ultra.max_render_scale, ULTRA_SUPERSAMPLE);
        assert_eq!(ultra.particle_ceiling, ULTRA_PARTICLES_MAX);
    }

    #[test]
    fn pi_and_high_are_fixed_caps() {
        let pi = budget_for(QualityMode::Pi, DeviceClass::Desktop, 144.0, GpuClass::Integrated);
        assert_eq!(pi.fps_target, 30.0);
        assert_eq!(pi.pacing, FramePacing::FixedCap);
        assert_eq!(pi.particles, PI_PARTICLES);
        assert_eq!(pi.render_scale, 0.5);
        assert_eq!(pi.bloom_passes, 1);
        assert!(pi.dither);
        let high = budget_for(QualityMode::High, DeviceClass::Laptop, 144.0, GpuClass::Integrated);
        assert_eq!(high.fps_target, 120.0);
        assert_eq!(high.pacing, FramePacing::FixedCap);
    }

    #[test]
    fn phone_laptop_desktop_aliases_stay_on_the_normal_ladder() {
        for name in ["phone", "laptop", "desktop", "normal"] {
            assert_eq!(parse_quality(name), Some(QualityMode::Normal));
        }
        let laptop = budget_for(QualityMode::Normal, DeviceClass::Laptop, 60.0, GpuClass::Integrated);
        let desktop = budget_for(QualityMode::Normal, DeviceClass::Desktop, 60.0, GpuClass::Integrated);
        assert_eq!(laptop.fps_target, desktop.fps_target);
        assert_eq!(laptop.pacing, NORMAL_PACING);
        assert!(laptop.particles < desktop.particles);
        assert_eq!(step_normal_target(60.0, 144.0, true), 90.0);
        assert_eq!(step_normal_target(60.0, 60.0, true), 60.0);
        assert_eq!(step_normal_target(60.0, 144.0, false), 45.0);
    }
}
