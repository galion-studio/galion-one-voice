//! Surface-agnostic visualization frame. Cheap to build (no heap churn beyond two short
//! strings), cheap to clone, trivially serializable without serde.

/// The two bands of the full-duplex loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    /// Human -> Galion (microphone / listen band).
    Listen,
    /// Galion -> human (speaker / speak band).
    Speak,
}

/// Coarse emotion label + intensity. Mirrors the emotion pipeline output (both directions).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emotion {
    pub label: &'static str,
    /// 0.0..=1.0
    pub intensity: f32,
    /// -1.0 (negative) ..= 1.0 (positive)
    pub valence: f32,
}

impl Default for Emotion {
    fn default() -> Self {
        Self { label: "neutral", intensity: 0.0, valence: 0.0 }
    }
}

/// Per-band state for one frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BandState {
    /// RMS level 0.0..=1.0.
    pub level: f32,
    /// True when VAD / model says this band is actively voicing.
    pub active: bool,
    pub emotion: Emotion,
    /// Last few words (tail of the running transcript), already trimmed by the producer.
    pub transcript_tail: String,
}

/// One visualization frame (target ~12.5 Hz = one per 80 ms codec frame).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VizFrame {
    /// Monotonic frame counter from the audio clock.
    pub seq: u64,
    /// Milliseconds since session start (audio clock, not wall clock).
    pub t_ms: u64,
    pub listen: BandState,
    pub speak: BandState,
    /// End-to-end latency estimate (mic in -> speaker out), ms.
    pub latency_ms: f32,
    /// Real-time factor of the model step (<1.0 = faster than real time).
    pub rtf: f32,
}

impl VizFrame {
    pub fn band(&self, b: Band) -> &BandState {
        match b {
            Band::Listen => &self.listen,
            Band::Speak => &self.speak,
        }
    }

    /// Dependency-free JSON encoding (used by the web surface and for logs).
    pub fn to_json(&self) -> String {
        fn esc(s: &str) -> String {
            let mut o = String::with_capacity(s.len() + 2);
            for c in s.chars() {
                match c {
                    '"' => o.push_str("\\\""),
                    '\\' => o.push_str("\\\\"),
                    '\n' => o.push_str("\\n"),
                    '\r' => o.push_str("\\r"),
                    '\t' => o.push_str("\\t"),
                    c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                    c => o.push(c),
                }
            }
            o
        }
        fn f(x: f32) -> String {
            if x.is_finite() { format!("{x:.4}") } else { "null".into() }
        }
        fn band(b: &BandState) -> String {
            format!(
                "{{\"level\":{},\"active\":{},\"emotion\":{{\"label\":\"{}\",\"intensity\":{},\"valence\":{}}},\"text\":\"{}\"}}",
                f(b.level), b.active, esc(b.emotion.label), f(b.emotion.intensity), f(b.emotion.valence), esc(&b.transcript_tail)
            )
        }
        format!(
            "{{\"seq\":{},\"t_ms\":{},\"listen\":{},\"speak\":{},\"latency_ms\":{},\"rtf\":{}}}",
            self.seq, self.t_ms, band(&self.listen), band(&self.speak), f(self.latency_ms), f(self.rtf)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_escapes() {
        let mut fr = VizFrame::default();
        fr.listen.transcript_tail = "say \"hi\"\n".into();
        let j = fr.to_json();
        assert!(j.contains("say \\\"hi\\\"\\n"));
        assert!(j.starts_with('{') && j.ends_with('}'));
    }
}
