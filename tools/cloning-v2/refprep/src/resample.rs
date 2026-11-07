//! Bit-for-bit port of torchaudio.functional.resample (sinc_interp_hann, lowpass_filter_width=6, rolloff=0.99).
fn gcd(a: u32, b: u32) -> u32 { if b == 0 { a } else { gcd(b, a % b) } }

pub struct Resampler { orig: usize, new: usize, width: usize, kern: Vec<Vec<f32>> }

impl Resampler {
    pub fn new(orig_sr: u32, new_sr: u32) -> Self {
        let g = gcd(orig_sr, new_sr);
        let (orig, new) = ((orig_sr / g) as usize, (new_sr / g) as usize);
        let lpw = 6.0f64;
        let base = (orig.min(new) as f64) * 0.99;
        let width = (lpw * orig as f64 / base).ceil() as usize;
        let klen = 2 * width + orig;
        let mut kern = vec![vec![0f32; klen]; new];
        for (j, row) in kern.iter_mut().enumerate() {
            // torch: arange(0,-new,-1) (int64) / new -> float32, then promoted to float64 by idx.
            let tj = (-(j as f32) / new as f32) as f64;
            for (k, v) in row.iter_mut().enumerate() {
                let idx = (k as f64 - width as f64) / orig as f64;
                let mut t = (tj + idx) * base;
                t = t.clamp(-lpw, lpw);
                let w = (t * std::f64::consts::PI / lpw / 2.0).cos().powi(2);
                t *= std::f64::consts::PI;
                let s = if t == 0.0 { 1.0 } else { t.sin() / t };
                *v = (s * (w * (base / orig as f64))) as f32;
            }
        }
        Resampler { orig, new, width, kern }
    }
    pub fn process(&self, x: &[f32]) -> Vec<f32> {
        if self.orig == self.new { return x.to_vec(); }
        let n = x.len();
        let mut pad = vec![0f32; n + 2 * self.width + self.orig];
        pad[self.width..self.width + n].copy_from_slice(x);
        let klen = self.kern[0].len();
        let frames = (pad.len() - klen) / self.orig + 1;
        let target = ((self.new as f64 * n as f64) / self.orig as f64).ceil() as usize;
        let mut out = Vec::with_capacity(frames * self.new);
        'o: for i in 0..frames {
            let seg = &pad[i * self.orig..i * self.orig + klen];
            for k in &self.kern {
                if out.len() >= target { break 'o; }
                out.push(seg.iter().zip(k).map(|(a, b)| a * b).sum::<f32>());
            }
        }
        out.truncate(target);
        out
    }
}

pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> { Resampler::new(from, to).process(x) }
