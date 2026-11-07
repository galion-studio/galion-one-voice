//! CPU point-sprite splat. Pi adds one bloom pass and ordered dither.

use crate::frame::NebulaFrame;
use crate::quality::TierBudget;
use crate::shade::{shade, Voice};
use crate::sim::Sim;
use nebula_elements::DEMO;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn render(sim: &Sim, frame: &NebulaFrame, width: u32, height: u32, budget: &TierBudget, bloom: bool) -> Image {
    let scale = budget.render_scale.clamp(0.25, budget.max_render_scale);
    let w = ((width as f32) * scale).round().max(2.0) as u32;
    let h = ((height as f32) * scale).round().max(2.0) as u32;
    let mut hdr = vec![0.0f32; (w * h * 3) as usize];
    splat(sim, frame, w, h, budget, &mut hdr);
    if bloom && budget.bloom_passes > 0 {
        for _ in 0..budget.bloom_passes {
            bloom_once(&mut hdr, w, h);
        }
    }
    let rgba = quantize(&hdr, w, h, budget.dither, frame.voice);
    Image { width: w, height: h, rgba }
}

fn splat(sim: &Sim, frame: &NebulaFrame, w: u32, h: u32, budget: &TierBudget, hdr: &mut [f32]) {
    let aspect = w as f32 / h as f32;
    let fov = 50.0_f32.to_radians();
    let focal = 1.0 / (fov * 0.5).tan();
    let distance = 3.15_f32;
    let eye = [distance * 0.15, 0.05, distance];
    let forward = normalize([-eye[0], -eye[1], -eye[2]]);
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let active = budget.particles.min(sim.particles.len());
    let min_side = (w.min(h)) as f32;
    let target_px = match budget.device {
        crate::quality::DeviceClass::Pi => (min_side * 0.02).clamp(3.0, 8.0),
        crate::quality::DeviceClass::Phone => (min_side * 0.016).clamp(2.5, 9.0),
        _ => (min_side * 0.01).clamp(2.0, 12.0),
    };
    for particle in &sim.particles[..active] {
        let weight = frame.weights.get(particle.emotion as usize).copied().unwrap_or(0.0);
        if weight < 0.02 {
            continue;
        }
        let Some(line) = DEMO.get(particle.emotion as usize) else { continue };
        let rel = [particle.pos[0] - eye[0], particle.pos[1] - eye[1], particle.pos[2] - eye[2]];
        let cam_x = dot(rel, right);
        let cam_y = dot(rel, up);
        let cam_z = dot(rel, forward);
        if cam_z < 0.2 {
            continue;
        }
        let ndc_x = (focal * cam_x / cam_z) / aspect;
        let ndc_y = focal * cam_y / cam_z;
        let sx = (ndc_x * 0.5 + 0.5) * w as f32;
        let sy = (1.0 - (ndc_y * 0.5 + 0.5)) * h as f32;
        let world = target_px / (focal * h as f32 * 0.5) * cam_z * (0.75 + 0.5 * line.motion.min(1.2));
        let radius = (world / cam_z * focal * h as f32 * 0.5).clamp(1.5, 18.0);
        let density = sim.sample_density(particle.pos);
        let view_dir = normalize(rel);
        let star_dir = normalize([0.2, 0.4, 1.0]);
        let cos_theta = dot(view_dir, star_dir);
        let rgb = shade(line, weight, density, cos_theta, frame.voice, budget.scatter_samples);
        paint(hdr, w, h, sx, sy, radius, rgb, 0.22 + 0.5 * weight);
    }
}


/// Eight floats per GPU sprite: clip_x, clip_y, radius_px, alpha, r, g, b, pad.
pub const SPRITE_STRIDE: usize = 8;

/// Pack projected, shaded sprites for the GPU path.
///
/// `glow` uses larger additive halos and a stronger voice illuminant so a
/// high-tier Snapdragon/Adreno view reads as a dense coloured nebula
/// instead of a handful of grey CPU dots.
pub fn write_sprites(
    sim: &Sim,
    frame: &NebulaFrame,
    width: u32,
    height: u32,
    budget: &TierBudget,
    glow: bool,
    out: &mut [f32],
) -> u32 {
    let w = width.max(2);
    let h = height.max(2);
    let aspect = w as f32 / h as f32;
    let fov = 50.0_f32.to_radians();
    let focal = 1.0 / (fov * 0.5).tan();
    let distance = 3.15_f32;
    let eye = [distance * 0.15, 0.05, distance];
    let forward = normalize([-eye[0], -eye[1], -eye[2]]);
    let right = normalize(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let active = budget.particles.min(sim.particles.len());
    let min_side = (w.min(h)) as f32;
    let target_px = if glow {
        (min_side * 0.018).clamp(6.0, 24.0)
    } else {
        match budget.device {
            crate::quality::DeviceClass::Pi => (min_side * 0.02).clamp(3.0, 8.0),
            crate::quality::DeviceClass::Phone => (min_side * 0.016).clamp(2.5, 9.0),
            _ => (min_side * 0.01).clamp(2.0, 12.0),
        }
    };
    let star = frame.voice.star_linear();
    let mut n = 0usize;
    let slots = out.len() / SPRITE_STRIDE;
    for particle in &sim.particles[..active] {
        let weight = frame.weights.get(particle.emotion as usize).copied().unwrap_or(0.0);
        if weight < 0.02 {
            continue;
        }
        let Some(line) = DEMO.get(particle.emotion as usize) else { continue };
        let rel = [particle.pos[0] - eye[0], particle.pos[1] - eye[1], particle.pos[2] - eye[2]];
        let cam_x = dot(rel, right);
        let cam_y = dot(rel, up);
        let cam_z = dot(rel, forward);
        if cam_z < 0.2 {
            continue;
        }
        let ndc_x = (focal * cam_x / cam_z) / aspect;
        let ndc_y = focal * cam_y / cam_z;
        if ndc_x.abs() > 1.35 || ndc_y.abs() > 1.35 {
            continue;
        }
        let world = target_px / (focal * h as f32 * 0.5) * cam_z * (0.75 + 0.5 * line.motion.min(1.2));
        let mut radius = (world / cam_z * focal * h as f32 * 0.5).clamp(1.5, if glow { 28.0 } else { 18.0 });
        let density = sim.sample_density(particle.pos);
        let view_dir = normalize(rel);
        let star_dir = normalize([0.2, 0.4, 1.0]);
        let cos_theta = dot(view_dir, star_dir);
        let mut rgb = shade(line, weight, density, cos_theta, frame.voice, budget.scatter_samples);
        let mut alpha = 0.22 + 0.5 * weight;
        if glow {
            rgb = [
                rgb[0] * 0.82 + star[0] * 0.22,
                rgb[1] * 0.82 + star[1] * 0.22,
                rgb[2] * 0.82 + star[2] * 0.22,
            ];
            rgb = [rgb[0] * 1.18, rgb[1] * 1.18, rgb[2] * 1.18];
            alpha = 0.24 + 0.40 * weight;
            radius = (radius * 1.35).clamp(6.0, 28.0);
        }
        if n >= slots {
            break;
        }
        let i = n * SPRITE_STRIDE;
        out[i] = ndc_x;
        out[i + 1] = ndc_y;
        out[i + 2] = radius;
        out[i + 3] = alpha;
        out[i + 4] = rgb[0];
        out[i + 5] = rgb[1];
        out[i + 6] = rgb[2];
        out[i + 7] = 0.0;
        n += 1;
        if glow && n < slots {
            let j = n * SPRITE_STRIDE;
            out[j] = ndc_x;
            out[j + 1] = ndc_y;
            out[j + 2] = (radius * 2.05).min(46.0);
            out[j + 3] = alpha * 0.20;
            out[j + 4] = rgb[0] * 0.40 + star[0] * 0.60;
            out[j + 5] = rgb[1] * 0.40 + star[1] * 0.60;
            out[j + 6] = rgb[2] * 0.40 + star[2] * 0.60;
            out[j + 7] = 0.0;
            n += 1;
        }
    }
    n as u32
}

fn paint(hdr: &mut [f32], w: u32, h: u32, cx: f32, cy: f32, radius: f32, rgb: [f32; 3], alpha: f32) {
    let r = radius.ceil() as i32;
    let x0 = (cx as i32 - r).max(0);
    let y0 = (cy as i32 - r).max(0);
    let x1 = (cx as i32 + r).min(w as i32 - 1);
    let y1 = (cy as i32 + r).min(h as i32 - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let r2 = (dx * dx + dy * dy) / (radius * radius);
            if r2 > 1.0 {
                continue;
            }
            let falloff = (-r2 * 3.2).exp() * alpha;
            let i = ((y as u32 * w + x as u32) * 3) as usize;
            hdr[i] += rgb[0] * falloff;
            hdr[i + 1] += rgb[1] * falloff;
            hdr[i + 2] += rgb[2] * falloff;
        }
    }
}

fn bloom_once(hdr: &mut [f32], w: u32, h: u32) {
    let dw = (w / 4).max(1);
    let dh = (h / 4).max(1);
    let mut small = vec![0.0f32; (dw * dh * 3) as usize];
    for y in 0..dh {
        for x in 0..dw {
            let mut acc = [0.0f32; 3];
            let mut n = 0.0f32;
            for oy in 0..4 {
                for ox in 0..4 {
                    let sx = (x * 4 + ox).min(w - 1);
                    let sy = (y * 4 + oy).min(h - 1);
                    let i = ((sy * w + sx) * 3) as usize;
                    acc[0] += hdr[i];
                    acc[1] += hdr[i + 1];
                    acc[2] += hdr[i + 2];
                    n += 1.0;
                }
            }
            let j = ((y * dw + x) * 3) as usize;
            small[j] = acc[0] / n;
            small[j + 1] = acc[1] / n;
            small[j + 2] = acc[2] / n;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let u = x as f32 / w as f32 * dw as f32;
            let v = y as f32 / h as f32 * dh as f32;
            let sx = (u as u32).min(dw - 1);
            let sy = (v as u32).min(dh - 1);
            let j = ((sy * dw + sx) * 3) as usize;
            let i = ((y * w + x) * 3) as usize;
            hdr[i] += small[j] * 0.45;
            hdr[i + 1] += small[j + 1] * 0.45;
            hdr[i + 2] += small[j + 2] * 0.45;
        }
    }
}

fn quantize(hdr: &[f32], w: u32, h: u32, dither: bool, voice: Voice) -> Vec<u8> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let floor = match voice {
        Voice::Samantha => [0.012, 0.008, 0.004],
        Voice::Jarvis => [0.004, 0.006, 0.016],
    };
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 3) as usize;
            let o = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                let mut v = hdr[i + c] + floor[c];
                v = v / (1.0 + v);
                v = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
                if dither {
                    v += (bayer(x, y) - 0.5) / 255.0;
                }
                rgba[o + c] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            rgba[o + 3] = 255;
        }
    }
    rgba
}

fn bayer(x: u32, y: u32) -> f32 {
    const M: [u32; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    M[((y & 3) * 4 + (x & 3)) as usize] as f32 / 16.0
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(1e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

pub fn write_png(path: &std::path::Path, image: &Image) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encode_png(image))
}

pub fn encode_png(image: &Image) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFF;
        for b in data {
            c ^= *b as u32;
            for _ in 0..8 {
                let bit = c & 1;
                c >>= 1;
                if bit == 1 {
                    c ^= 0xEDB8_8320;
                }
            }
        }
        !c
    }
    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let sum = crc(&out[4..]);
        out.extend_from_slice(&sum.to_be_bytes());
        out
    }
    let mut raw = Vec::with_capacity((image.width * image.height * 4 + image.height) as usize);
    for y in 0..image.height {
        raw.push(0);
        let start = (y * image.width * 4) as usize;
        raw.extend_from_slice(&image.rgba[start..start + (image.width * 4) as usize]);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut i = 0;
    while i < raw.len() {
        let n = (raw.len() - i).min(65535);
        let last = i + n == raw.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend_from_slice(&(n as u16).to_le_bytes());
        zlib.extend_from_slice(&(!n as u16).to_le_bytes());
        zlib.extend_from_slice(&raw[i..i + n]);
        i += n;
    }
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in &raw {
        a = (a + *byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    zlib.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&image.width.to_be_bytes());
    ihdr.extend_from_slice(&image.height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib));
    png.extend(chunk(b"IEND", &[]));
    png
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::NebulaFrame;
    use crate::quality::{budget_for, DeviceClass, GpuClass, QualityMode};
    use crate::sim::Sim;

    #[test]
    fn pi_frame_is_half_resolution_and_not_empty() {
        let budget = budget_for(QualityMode::Pi, DeviceClass::Pi, 60.0, GpuClass::Integrated);
        let sim = Sim::create(3, budget.particles);
        let frame = NebulaFrame::neutral(Voice::Samantha);
        let image = render(&sim, &frame, 640, 360, &budget, true);
        assert_eq!(image.width, 320);
        assert_eq!(image.height, 180);
        assert!(image.rgba.iter().any(|b| *b > 8));
        assert_eq!(&encode_png(&image)[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn gpu_sprites_are_dense_and_warm_for_samantha() {
        let budget = budget_for(QualityMode::High, DeviceClass::Phone, 120.0, GpuClass::Integrated);
        let sim = Sim::create(11, budget.particles);
        let frame = crate::demo::frame_at(Voice::Samantha, 1.2);
        let mut buf = vec![0.0f32; budget.particles * SPRITE_STRIDE * 2];
        let n = write_sprites(&sim, &frame, 1080, 2400, &budget, true, &mut buf);
        assert!(n > 2_000, "high-tier glow should emit thousands of sprites, got {n}");
        let mut r = 0.0f32;
        let mut b = 0.0f32;
        let mut warm = 0u32;
        for i in 0..n as usize {
            let o = i * SPRITE_STRIDE;
            r += buf[o + 4];
            b += buf[o + 6];
            if buf[o + 4] > buf[o + 6] {
                warm += 1;
            }
        }
        assert!(r > b, "samantha sprites should lean warm, r={r} b={b}");
        assert!(warm * 2 > n, "most samantha sprites should have r > b");
        let mean_radius: f32 = (0..n as usize).map(|i| buf[i * SPRITE_STRIDE + 2]).sum::<f32>() / n as f32;
        assert!(mean_radius < 36.0, "glow sprites should not fill the screen, mean radius {mean_radius}");
    }
}
