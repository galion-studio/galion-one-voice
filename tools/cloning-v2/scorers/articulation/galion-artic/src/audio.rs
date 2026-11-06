//! WAV loading (any PCM/float format, mixed to mono) and rubato resampling.
use rubato::{FftFixedIn, Resampler};

pub struct Audio {
    pub fs: u32,
    pub x: Vec<f32>,
}

pub fn load_wav(path: &str) -> Result<Audio, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| format!("{path}: {e}"))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let inter: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.unwrap_or(0) as f32 * scale).collect()
        }
    };
    let n = inter.len() / ch;
    let mut x = vec![0f32; n];
    for i in 0..n {
        let mut s = 0.0;
        for c in 0..ch {
            s += inter[i * ch + c];
        }
        x[i] = s / ch as f32;
    }
    Ok(Audio { fs: spec.sample_rate, x })
}

/// Band-limited resampling with rubato's FFT resampler; output length = round(n*out/in), delay-compensated.
pub fn resample(x: &[f32], fs_in: u32, fs_out: u32) -> Vec<f32> {
    if fs_in == fs_out {
        return x.to_vec();
    }
    let chunk = 1024usize;
    let mut rs = FftFixedIn::<f32>::new(fs_in as usize, fs_out as usize, chunk, 2, 1).expect("resampler");
    let delay = rs.output_delay();
    let want = ((x.len() as f64) * fs_out as f64 / fs_in as f64).round() as usize;
    let mut out: Vec<f32> = Vec::with_capacity(want + delay + chunk * 2);
    let mut pos = 0usize;
    let mut buf = vec![0f32; chunk];
    while out.len() < want + delay {
        let need = rs.input_frames_next();
        if buf.len() != need {
            buf.resize(need, 0.0);
        }
        for i in 0..need {
            buf[i] = if pos + i < x.len() { x[pos + i] } else { 0.0 };
        }
        pos += need;
        let o = rs.process(&[&buf[..]], None).expect("resample");
        out.extend_from_slice(&o[0]);
    }
    out.drain(..delay);
    out.truncate(want);
    out
}
