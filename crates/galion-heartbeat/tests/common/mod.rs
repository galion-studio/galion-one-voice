// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
#![allow(dead_code)]
//! Shared synthetic you-band signals (same as the G6 fixtures: seed 13, 24 kHz, 1920 samples).
use galion_heartbeat::{FRAME_SAMPLES, SAMPLE_RATE};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum K {
    Room,
    Breath,
    Voice,
}

pub struct Lcg(pub u64);
impl Lcg {
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

pub fn script(parts: &[(K, usize)]) -> Vec<K> {
    parts.iter().flat_map(|&(k, n)| std::iter::repeat(k).take(n)).collect()
}

/// Pre-render every frame (allocation happens here, before the loop).
pub fn render(ks: &[K]) -> Vec<Vec<f32>> {
    let mut rng = Lcg(13);
    ks.iter()
        .enumerate()
        .map(|(f, &k)| match k {
            K::Room | K::Breath => {
                let a = if k == K::Breath { 0.003f32 } else { 0.001 } * 3f32.sqrt();
                (0..FRAME_SAMPLES).map(|_| a * rng.next()).collect()
            }
            K::Voice => (0..FRAME_SAMPLES)
                .map(|i| 0.1 * (2.0 * std::f32::consts::PI * 200.0 * ((f * FRAME_SAMPLES + i) as f32) / SAMPLE_RATE as f32).sin())
                .collect(),
        })
        .collect()
}

/// R1: 13 room, 5 breath (evidence frame 13), 5 voice (onset 18), 10 room.
pub fn r1() -> Vec<K> {
    script(&[(K::Room, 13), (K::Breath, 5), (K::Voice, 5), (K::Room, 10)])
}
