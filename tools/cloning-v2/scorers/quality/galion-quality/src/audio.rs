use std::path::Path;

/// Decoded WAV: per-channel f32 in [-1,1) (int PCM normalised by 2^(bits-1), like torchaudio.load).
pub struct Wav { pub sr: u32, pub chans: Vec<Vec<f32>>, pub bits: u16, pub float: bool }

impl Wav {
    pub fn len(&self) -> usize { self.chans.first().map(|c| c.len()).unwrap_or(0) }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn duration_s(&self) -> f64 { self.len() as f64 / self.sr as f64 }
    /// Channel mean (torch `w.mean(0)`).
    pub fn mono(&self) -> Vec<f32> {
        if self.chans.len() == 1 { return self.chans[0].clone(); }
        let n = self.chans.len() as f32;
        (0..self.len()).map(|i| self.chans.iter().map(|c| c[i]).sum::<f32>() / n).collect()
    }
}

pub fn load(p: &Path) -> Result<Wav, String> {
    let mut r = hound::WavReader::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let s = r.spec();
    let nc = s.channels as usize;
    let mut chans = vec![Vec::with_capacity(r.len() as usize / nc.max(1)); nc];
    match s.sample_format {
        hound::SampleFormat::Float => {
            for (i, x) in r.samples::<f32>().enumerate() { chans[i % nc].push(x.map_err(|e| e.to_string())?); }
        }
        hound::SampleFormat::Int => {
            let k = 1.0f32 / (1u64 << (s.bits_per_sample - 1)) as f32;
            for (i, x) in r.samples::<i32>().enumerate() { chans[i % nc].push(x.map_err(|e| e.to_string())? as f32 * k); }
        }
    }
    Ok(Wav { sr: s.sample_rate, chans, bits: s.bits_per_sample, float: s.sample_format == hound::SampleFormat::Float })
}

pub fn db(x: f64) -> f64 { if x > 0.0 { 20.0 * x.log10() } else { f64::NEG_INFINITY } }
