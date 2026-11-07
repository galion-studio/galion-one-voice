//! Value noise with an analytic gradient, and a curl of a vector potential.
//!
//! Octave amplitudes on the potential follow `f^(-4/3)`, so the curl (one
//! derivative) follows a Kolmogorov-like `f^(-1/3)` velocity amplitude.
//! Bridson, Houriham, and Nordenstam, SIGGRAPH 2007, is the curl construction.

#[derive(Clone, Copy)]
pub struct Grad {
    pub value: f32,
    pub d: [f32; 3],
}

fn hash01(ix: i32, iy: i32, iz: i32, seed: u32) -> f32 {
    let mut n = (ix as u32).wrapping_mul(3_747_613_93);
    n = n.wrapping_add((iy as u32).wrapping_mul(668_265_263));
    n = n.wrapping_add((iz as u32).wrapping_mul(2_147_483_647));
    n ^= seed.wrapping_mul(1_274_126_177);
    n ^= n >> 13;
    n = n.wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0x00FF_FFFF) as f32 / 16_777_215.0 * 2.0 - 1.0
}

fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn fade_d(t: f32) -> f32 {
    6.0 * t * (1.0 - t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn value_noise(p: [f32; 3], seed: u32) -> Grad {
    let x0 = p[0].floor() as i32;
    let y0 = p[1].floor() as i32;
    let z0 = p[2].floor() as i32;
    let tx = p[0] - x0 as f32;
    let ty = p[1] - y0 as f32;
    let tz = p[2] - z0 as f32;
    let fx = fade(tx);
    let fy = fade(ty);
    let fz = fade(tz);
    let dx = fade_d(tx);
    let dy = fade_d(ty);
    let dz = fade_d(tz);
    let n = |x, y, z| hash01(x, y, z, seed);
    let n000 = n(x0, y0, z0);
    let n100 = n(x0 + 1, y0, z0);
    let n010 = n(x0, y0 + 1, z0);
    let n110 = n(x0 + 1, y0 + 1, z0);
    let n001 = n(x0, y0, z0 + 1);
    let n101 = n(x0 + 1, y0, z0 + 1);
    let n011 = n(x0, y0 + 1, z0 + 1);
    let n111 = n(x0 + 1, y0 + 1, z0 + 1);
    let nx00 = lerp(n000, n100, fx);
    let nx10 = lerp(n010, n110, fx);
    let nx01 = lerp(n001, n101, fx);
    let nx11 = lerp(n011, n111, fx);
    let nxy0 = lerp(nx00, nx10, fy);
    let nxy1 = lerp(nx01, nx11, fy);
    let value = lerp(nxy0, nxy1, fz);
    let dnx00 = (n100 - n000) * dx;
    let dnx10 = (n110 - n010) * dx;
    let dnx01 = (n101 - n001) * dx;
    let dnx11 = (n111 - n011) * dx;
    let dn_dx = lerp(lerp(dnx00, dnx10, fy), lerp(dnx01, dnx11, fy), fz);
    let dn_dy = lerp((nx10 - nx00) * dy, (nx11 - nx01) * dy, fz);
    let dn_dz = (nxy1 - nxy0) * dz;
    Grad { value, d: [dn_dx, dn_dy, dn_dz] }
}

/// Curl of a 3-octave (or `octaves`) vector potential.
pub fn curl(p: [f32; 3], time: f32, octaves: u32, seed: u32) -> [f32; 3] {
    let octaves = octaves.clamp(1, 6);
    let mut vel = [0.0f32; 3];
    let mut norm = 0.0f32;
    let mut freq = 0.55f32;
    for octave in 0..octaves {
        let psi_amp = freq.powf(-4.0 / 3.0);
        let vel_amp = freq.powf(-1.0 / 3.0);
        let shift = time * 0.07 + octave as f32 * 19.17;
        let sample = |offset: [f32; 3], salt: u32| {
            let q = [
                p[0] * freq + offset[0] + shift,
                p[1] * freq + offset[1],
                p[2] * freq + offset[2],
            ];
            let g = value_noise(q, seed.wrapping_add(salt));
            Grad { value: g.value, d: [g.d[0] * freq, g.d[1] * freq, g.d[2] * freq] }
        };
        let px = sample([0.0, 0.0, 0.0], 11);
        let py = sample([17.2, 3.1, 0.0], 29);
        let pz = sample([4.4, 9.7, 13.0], 47);
        // curl ψ = (dψz/dy - dψy/dz, dψx/dz - dψz/dx, dψy/dx - dψx/dy)
        vel[0] += psi_amp * (pz.d[1] - py.d[2]);
        vel[1] += psi_amp * (px.d[2] - pz.d[0]);
        vel[2] += psi_amp * (py.d[0] - px.d[1]);
        norm += vel_amp;
        freq *= 2.0;
    }
    let scale = if norm > 1e-6 { 1.0 / norm } else { 0.0 };
    [vel[0] * scale, vel[1] * scale, vel[2] * scale]
}

pub fn henyey_greenstein(cos_theta: f32, g: f32) -> f32 {
    let g = g.clamp(-0.95, 0.95);
    let denom = 1.0 + g * g - 2.0 * g * cos_theta;
    (1.0 - g * g) / (4.0 * std::f32::consts::PI * denom.powf(1.5).max(1e-4))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn div_at(p: [f32; 3], octaves: u32) -> f32 {
        let eps = 0.02f32;
        let vx1 = curl([p[0] + eps, p[1], p[2]], 0.3, octaves, 7);
        let vx0 = curl([p[0] - eps, p[1], p[2]], 0.3, octaves, 7);
        let vy1 = curl([p[0], p[1] + eps, p[2]], 0.3, octaves, 7);
        let vy0 = curl([p[0], p[1] - eps, p[2]], 0.3, octaves, 7);
        let vz1 = curl([p[0], p[1], p[2] + eps], 0.3, octaves, 7);
        let vz0 = curl([p[0], p[1], p[2] - eps], 0.3, octaves, 7);
        (vx1[0] - vx0[0] + vy1[1] - vy0[1] + vz1[2] - vz0[2]) / (2.0 * eps)
    }

    #[test]
    fn curl_is_approximately_divergence_free() {
        for p in [[0.2, -0.4, 0.1], [1.1, 0.3, -0.7], [-0.6, 0.8, 0.4]] {
            let d = div_at(p, 3).abs();
            assert!(d < 0.08, "divergence {d} at {p:?}");
        }
    }
}
