//! Filament dust. Curl advection, mild grid gravity, spine attraction.
//!
//! Particles are born along open curves, not on a sphere. The same seed and
//! the same drive reproduce the same positions.

use crate::noise::curl;
use crate::quality::PI_PARTICLES;
use nebula_elements::{demo, DemoLine, DEMO};

#[derive(Clone, Copy)]
pub struct Particle {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub emotion: u8,
    pub spine: u8,
    pub along: f32,
    pub phase: f32,
}

#[derive(Clone)]
pub struct Sim {
    pub particles: Vec<Particle>,
    pub seed: u32,
    pub time: f32,
    pub mass: Vec<f32>,
    pub mass_n: i32,
}

#[derive(Clone, Copy)]
pub struct Drive {
    pub weights: [f32; 13],
    pub turbulence: f32,
    pub clump: f32,
    pub pulse: f32,
    pub active: usize,
    pub octaves: u32,
}

impl Drive {
    pub fn neutral(active: usize, octaves: u32) -> Self {
        let mut weights = [0.0; 13];
        weights[0] = 1.0;
        Self { weights, turbulence: 0.35, clump: 0.4, pulse: 0.0, active, octaves }
    }
}

struct Rng(u32);
impl Rng {
    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }
}

pub fn spine_point(spine: u8, along: f32, time: f32) -> [f32; 3] {
    let s = along.clamp(0.0, 1.0) * 2.0 - 1.0;
    let wobble = (time * 0.15 + spine as f32).sin() * 0.08;
    match spine % 5 {
        0 => [s * 1.35, (s * 1.7).sin() * 0.45 + wobble, (s * 2.1).cos() * 0.25],
        1 => [(s * 1.3).cos() * 0.35, s * 1.2, (s * 1.8).sin() * 0.55 + wobble],
        2 => [(s * 2.2).sin() * 0.5, (s * 1.4).cos() * 0.3 + wobble, s * 1.25],
        3 => [s * 1.05 + 0.3, (s * 2.4).sin() * 0.4, (s * 1.1).cos() * 0.45 - 0.2],
        _ => [-0.35 + (s * 1.6).sin() * 0.3, -0.15 + s * 0.95, (s * 2.0).cos() * 0.6 + wobble],
    }
}

impl Sim {
    pub fn create(seed: u32, count: usize) -> Self {
        let count = count.max(1);
        let mut rng = Rng(seed | 1);
        let mut particles = Vec::with_capacity(count);
        for i in 0..count {
            let emotion = (i % 13) as u8;
            let spine = (i % 5) as u8;
            let along = rng.unit();
            let centre = spine_point(spine, along, 0.0);
            let jitter = 0.05 + rng.unit() * 0.28;
            let ang = rng.unit() * std::f32::consts::TAU;
            let pos = [
                centre[0] + jitter * ang.cos(),
                centre[1] + (rng.unit() - 0.5) * 0.2,
                centre[2] + jitter * ang.sin(),
            ];
            particles.push(Particle {
                pos,
                vel: [0.0; 3],
                emotion,
                spine,
                along,
                phase: rng.unit() * std::f32::consts::TAU,
            });
        }
        Self { particles, seed, time: 0.0, mass: Vec::new(), mass_n: 0 }
    }

    pub fn active(&self, n: usize) -> usize {
        n.min(self.particles.len())
    }

    fn deposit(&mut self, drive: &Drive) {
        let n = 12i32;
        self.mass_n = n;
        self.mass.clear();
        self.mass.resize((n * n * n) as usize, 0.0);
        let active = self.active(drive.active);
        for particle in &self.particles[..active] {
            let weight = drive.weights.get(particle.emotion as usize).copied().unwrap_or(0.0);
            if weight < 0.01 {
                continue;
            }
            let idx = |x: f32| ((x + 1.8) / 3.6 * n as f32).clamp(0.0, (n - 1) as f32) as i32;
            let (ix, iy, iz) = (idx(particle.pos[0]), idx(particle.pos[1]), idx(particle.pos[2]));
            let slot = ((iz * n + iy) * n + ix) as usize;
            self.mass[slot] += weight;
        }
    }

    fn mass_at(&self, pos: [f32; 3]) -> f32 {
        mass_lookup(&self.mass, self.mass_n, pos)
    }

    pub fn sample_density(&self, pos: [f32; 3]) -> f32 {
        let here = self.mass_at(pos);
        let peak = self.mass.iter().copied().fold(0.0f32, f32::max).max(1e-4);
        (here / peak).clamp(0.0, 1.0)
    }

    pub fn step(&mut self, drive: &Drive, dt: f32) {
        let dt = dt.clamp(1.0 / 240.0, 0.05);
        let active = self.active(drive.active);
        self.deposit(drive);
        self.time += dt;
        let n = self.mass_n;
        let mass = self.mass.clone();
        for particle in &mut self.particles[..active] {
            let line = DEMO.get(particle.emotion as usize);
            let weight = drive.weights.get(particle.emotion as usize).copied().unwrap_or(0.0);
            let motion = line.map(|l| l.motion).unwrap_or(0.4);
            let pulse = line.map(|l| l.pulse).unwrap_or(0.0);
            let flow = curl(particle.pos, self.time, drive.octaves, self.seed);
            let mut acc = [
                flow[0] * drive.turbulence * motion,
                flow[1] * drive.turbulence * motion,
                flow[2] * drive.turbulence * motion,
            ];
            let centre = spine_point(particle.spine, particle.along, self.time);
            acc[0] += (centre[0] - particle.pos[0]) * (0.35 + 0.8 * drive.clump);
            acc[1] += (centre[1] - particle.pos[1]) * (0.35 + 0.8 * drive.clump);
            acc[2] += (centre[2] - particle.pos[2]) * (0.35 + 0.8 * drive.clump);
            if n >= 3 {
                let eps = 3.6 / n as f32;
                let dx = mass_lookup(&mass, n, [particle.pos[0] + eps, particle.pos[1], particle.pos[2]])
                    - mass_lookup(&mass, n, [particle.pos[0] - eps, particle.pos[1], particle.pos[2]]);
                let dy = mass_lookup(&mass, n, [particle.pos[0], particle.pos[1] + eps, particle.pos[2]])
                    - mass_lookup(&mass, n, [particle.pos[0], particle.pos[1] - eps, particle.pos[2]]);
                let dz = mass_lookup(&mass, n, [particle.pos[0], particle.pos[1], particle.pos[2] + eps])
                    - mass_lookup(&mass, n, [particle.pos[0], particle.pos[1], particle.pos[2] - eps]);
                let g = drive.clump * 0.15;
                acc[0] += dx * g;
                acc[1] += dy * g;
                acc[2] += dz * g;
            }
            let breathe = (self.time * (2.0 + 3.0 * drive.pulse) + particle.phase).sin();
            let radial = [
                particle.pos[0] - centre[0],
                particle.pos[1] - centre[1],
                particle.pos[2] - centre[2],
            ];
            let radial_len = (radial[0] * radial[0] + radial[1] * radial[1] + radial[2] * radial[2]).sqrt().max(1e-3);
            let kick = drive.pulse * (0.25 + pulse) * breathe * weight;
            acc[0] += radial[0] / radial_len * kick;
            acc[1] += radial[1] / radial_len * kick;
            acc[2] += radial[2] / radial_len * kick;
            particle.vel[0] = particle.vel[0] * 0.92 + acc[0] * dt;
            particle.vel[1] = particle.vel[1] * 0.92 + acc[1] * dt;
            particle.vel[2] = particle.vel[2] * 0.92 + acc[2] * dt;
            let speed2 = particle.vel.iter().map(|v| v * v).sum::<f32>();
            if speed2 > 1.4 {
                let scale = (1.4f32 / speed2).sqrt();
                particle.vel[0] *= scale;
                particle.vel[1] *= scale;
                particle.vel[2] *= scale;
            }
            particle.pos[0] += particle.vel[0] * dt;
            particle.pos[1] += particle.vel[1] * dt;
            particle.pos[2] += particle.vel[2] * dt;
            for axis in 0..3 {
                if particle.pos[axis] > 1.9 {
                    particle.pos[axis] = 1.9;
                    particle.vel[axis] *= -0.2;
                } else if particle.pos[axis] < -1.9 {
                    particle.pos[axis] = -1.9;
                    particle.vel[axis] *= -0.2;
                }
            }
            particle.along = (particle.along + dt * (0.02 + 0.05 * drive.turbulence)).fract();
        }
        let mut energy = 0.0f32;
        for particle in &self.particles[..active] {
            energy += particle.vel.iter().map(|v| v * v).sum::<f32>();
        }
        let cap = active as f32 * 0.45;
        if energy > cap && energy > 0.0 {
            let scale = (cap / energy).sqrt();
            for particle in &mut self.particles[..active] {
                particle.vel[0] *= scale;
                particle.vel[1] *= scale;
                particle.vel[2] *= scale;
            }
        }
    }

    pub fn kinetic_energy(&self, active: usize) -> f32 {
        self.particles[..self.active(active)].iter().map(|p| p.vel.iter().map(|v| v * v).sum::<f32>()).sum()
    }
}

fn mass_lookup(mass: &[f32], n: i32, pos: [f32; 3]) -> f32 {
    if n < 2 || mass.is_empty() {
        return 0.0;
    }
    let idx = |x: f32| ((x + 1.8) / 3.6 * n as f32).clamp(0.0, (n - 1) as f32) as i32;
    let (ix, iy, iz) = (idx(pos[0]), idx(pos[1]), idx(pos[2]));
    mass[((iz * n + iy) * n + ix) as usize]
}

pub fn line_for(index: u8) -> Option<&'static DemoLine> {
    DEMO.get(index as usize).or_else(|| demo(if index == 0 { "neutral" } else { "" }))
}

pub fn minimum_particles() -> usize {
    PI_PARTICLES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_steps_match_and_energy_stays_bounded() {
        let mut a = Sim::create(42, 600);
        let mut b = Sim::create(42, 600);
        let drive = Drive { weights: [0.1, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 0.0, 0.0], turbulence: 1.3, clump: 0.9, pulse: 0.8, active: 600, octaves: 3 };
        for _ in 0..40 {
            a.step(&drive, 1.0 / 60.0);
            b.step(&drive, 1.0 / 60.0);
        }
        for (p, q) in a.particles.iter().zip(b.particles.iter()) {
            assert_eq!(p.pos, q.pos);
        }
        let energy = a.kinetic_energy(600);
        assert!(energy <= 600.0 * 0.45 + 1e-3, "energy {energy}");
        assert!(a.particles.iter().all(|p| p.pos.iter().all(|c| c.is_finite())));
    }

    #[test]
    fn birth_positions_are_not_a_spherical_shell() {
        let sim = Sim::create(9, 800);
        let mut mean_r = 0.0f32;
        let mut radii = Vec::with_capacity(sim.particles.len());
        for p in &sim.particles {
            let r = (p.pos[0] * p.pos[0] + p.pos[1] * p.pos[1] + p.pos[2] * p.pos[2]).sqrt();
            radii.push(r);
            mean_r += r;
        }
        mean_r /= radii.len() as f32;
        let var = radii.iter().map(|r| (r - mean_r).powi(2)).sum::<f32>() / radii.len() as f32;
        assert!(var.sqrt() > 0.15, "radial stddev {} looks like a shell", var.sqrt());
    }
}
