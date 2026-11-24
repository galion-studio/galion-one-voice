// Galion Voice — Perpetual Open MIT License
//! Energy VAD over 20 ms frames with an adaptive noise floor and hangover.
pub struct Vad { pub floor_db: f64, pub on_db: f64, pub speaking: bool, pub silent_ms: f64, voiced_ms: f64, pub hang_ms: f64, pub min_voiced_ms: f64, pub last_voiced: bool }
#[derive(Debug, PartialEq)]
pub enum VadEv { None, Start, End }
impl Vad {
    pub fn new(hang_ms: f64) -> Self { Self { floor_db: -60.0, on_db: 12.0, speaking: false, silent_ms: 0.0, voiced_ms: 0.0, hang_ms, min_voiced_ms: 60.0, last_voiced: false } }
    pub fn push(&mut self, frame: &[i16], frame_ms: f64) -> VadEv {
        let db = crate::audio::rms_db(frame);
        let voiced = db > (self.floor_db + self.on_db).max(-48.0);
        self.last_voiced = voiced;
        if !voiced { self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0); }
        if voiced { self.voiced_ms += frame_ms; self.silent_ms = 0.0; } else { self.silent_ms += frame_ms; }
        if !self.speaking && voiced && self.voiced_ms >= self.min_voiced_ms { self.speaking = true; return VadEv::Start; }
        if !voiced && !self.speaking { self.voiced_ms = 0.0; }
        if self.speaking && self.silent_ms >= self.hang_ms { self.speaking = false; self.voiced_ms = 0.0; return VadEv::End; }
        VadEv::None
    }
    /// Close the turn from outside (two-stage endpointer).
    pub fn end(&mut self) { self.speaking = false; self.voiced_ms = 0.0; }
}
