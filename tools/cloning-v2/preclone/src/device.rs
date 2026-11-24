// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! CPU by default. A GPU is eligible only when this crate is built with
//! `--features gpu` and a device node is visible. Eligibility does not run a
//! kernel: the profile is always the CPU measurement, so the bytes do not
//! depend on the machine.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compute {
    Cpu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    pub nvidia: bool,
    pub kfd: bool,
}

impl Probe {
    pub fn this_host() -> Self {
        let nvidia = Path::new("/dev/nvidia0").exists() || Path::new("/dev/nvidiactl").exists();
        let kfd = Path::new("/dev/kfd").exists();
        Self { nvidia, kfd }
    }
}

/// The `gpu` cargo feature is the gate. Without it a visible device stays ineligible.
pub fn feature_enabled() -> bool {
    cfg!(feature = "gpu")
}

pub fn gpu_eligible(feature_on: bool, probe: Probe) -> bool {
    feature_on && (probe.nvidia || probe.kfd)
}

pub fn gpu_eligible_here(probe: Probe) -> bool {
    gpu_eligible(feature_enabled(), probe)
}

/// The path that writes the profile. Always CPU.
pub fn active_compute(_probe: Probe) -> Compute {
    Compute::Cpu
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_stays_off_unless_the_feature_and_a_device_are_both_present() {
        let nvidia = Probe {
            nvidia: true,
            kfd: false,
        };
        let none = Probe {
            nvidia: false,
            kfd: false,
        };
        assert!(!gpu_eligible(false, nvidia));
        assert!(!gpu_eligible(true, none));
        assert!(gpu_eligible(true, nvidia));
        assert!(gpu_eligible(
            true,
            Probe {
                nvidia: false,
                kfd: true,
            }
        ));
        assert_eq!(active_compute(nvidia), Compute::Cpu);
        assert_eq!(feature_enabled(), cfg!(feature = "gpu"));
    }
}
