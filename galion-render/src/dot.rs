//! The one dot (BRANDING.md V4: "The interface is one dot. Nothing else on screen.").
//!
//! [`DotState`] is derived from heartbeat state only (the per-80 ms [`VizFrame`]), and
//! [`DotParams`] (radius + uniform brightness) is a pure function of `(state, level, t_ms)`
//! so every surface — terminal, web, window, `NullSurface` goldens — draws the same dot.
//!
//! Hard rules enforced here:
//! * the dot colour is always [`palette::DOT`] scaled uniformly by `brightness`;
//! * the gradient ([`palette::background_at`]) is sampled for background pixels only;
//! * only the outer one-pixel edge of the dot is antialiased against the background.
//!
//! No text, no captions, no chrome: the helpers below output nothing but the dot and
//! its background.

use crate::frame::VizFrame;
use crate::palette::{self, Rgb};

/// One heartbeat frame, ms (Mimi 12.5 Hz).
pub const FRAME_MS: u64 = 80;
/// Idle pulse loop (≈1.6 s).
pub const IDLE_PERIOD_MS: u64 = 1_600;
/// Listening breathe loop (slow, almost still).
pub const LISTEN_PERIOD_MS: u64 = 3_200;

/// What the voice model is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DotState {
    /// Resting: slow subtle pulse, slightly dimmed.
    #[default]
    Idle,
    /// Human is speaking: a little larger, almost still.
    Listening,
    /// Galion is speaking: rhythmic scale, one step per 80 ms frame.
    Speaking,
}

impl DotState {
    pub fn as_str(self) -> &'static str {
        match self {
            DotState::Idle => "idle",
            DotState::Listening => "listening",
            DotState::Speaking => "speaking",
        }
    }

    /// Heartbeat → state. Speak band wins when both are active (the model is voicing;
    /// barge-in handling flips `speak.active` off upstream, which then shows Listening).
    pub fn from_frame(f: &VizFrame) -> Self {
        if f.speak.active {
            DotState::Speaking
        } else if f.listen.active {
            DotState::Listening
        } else {
            DotState::Idle
        }
    }
}

/// Geometry + brightness for one frame of the dot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotParams {
    pub state: DotState,
    /// Radius as a fraction of the surface's shorter side (0..=0.5).
    pub radius: f32,
    /// Uniform brightness multiplier applied to [`palette::DOT`], 0..=1.
    pub brightness: f32,
}

fn clamp01(x: f32) -> f32 { if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 } }

fn osc(t_ms: u64, period_ms: u64) -> f32 {
    let p = (t_ms % period_ms) as f32 / period_ms as f32;
    (p * std::f32::consts::TAU).sin()
}

impl DotParams {
    pub const IDLE_RADIUS: f32 = 0.100;
    pub const LISTEN_RADIUS: f32 = 0.120;
    pub const SPEAK_RADIUS: f32 = 0.115;

    /// Pure mapping. `level` is the RMS (0..=1) of the band that drives the state
    /// (speak level when speaking, listen level when listening, ignored when idle).
    pub fn compute(state: DotState, level: f32, t_ms: u64) -> Self {
        let level = clamp01(level);
        let (radius, brightness) = match state {
            DotState::Idle => {
                let s = osc(t_ms, IDLE_PERIOD_MS);
                (Self::IDLE_RADIUS + 0.006 * s, 0.84 + 0.03 * s)
            }
            DotState::Listening => {
                let s = osc(t_ms, LISTEN_PERIOD_MS);
                (Self::LISTEN_RADIUS + 0.002 * s + 0.004 * level, 0.96)
            }
            DotState::Speaking => {
                // Quantised to the 80 ms grid: one step per frame, never in between.
                // Continuous motion between these targets is `OrbEngine`. Do not
                // smooth this function; G4 hashes it.
                let step = (t_ms / FRAME_MS) % 4;
                let rhythm = [0.0f32, 0.5, 1.0, 0.5][step as usize];
                let drive = 0.65 * level + 0.35 * rhythm;
                (Self::SPEAK_RADIUS + 0.030 * drive, 0.97 + 0.03 * drive)
            }
        };
        Self { state, radius: radius.clamp(0.0, 0.5), brightness: clamp01(brightness) }
    }

    /// Everything a surface needs, from the heartbeat frame alone.
    /// An idle frame carrying a Rika lean-in ([`lean_in_of`]) is drawn with
    /// [`DotParams::compute_lean`]; the locked state set stays idle/listening/speaking.
    pub fn from_frame(f: &VizFrame) -> Self {
        let state = DotState::from_frame(f);
        let level = match state {
            DotState::Speaking => f.speak.level,
            DotState::Listening => f.listen.level,
            DotState::Idle => 0.0,
        };
        Self::compute_lean(state, level, lean_in_of(f), f.t_ms)
    }

    /// V1-01 · Rika lean-in ("the feeling band notices you before you speak").
    /// Only modulates **Idle** (Rika arms inside silence): the idle pulse is held
    /// (held breath, no oscillation), the dot leans toward listening size and
    /// brightens slightly. Size + uniform brightness only; never a tint.
    /// `lean <= 0` (or non-idle state) is exactly [`DotParams::compute`].
    pub fn compute_lean(state: DotState, level: f32, lean: f32, t_ms: u64) -> Self {
        let lean = clamp01(lean);
        if state != DotState::Idle || lean <= 0.0 {
            return Self::compute(state, level, t_ms);
        }
        let radius = Self::IDLE_RADIUS + (Self::LISTEN_RADIUS - Self::IDLE_RADIUS) * 0.6 * lean;
        let brightness = 0.86 + 0.08 * lean;
        Self { state, radius: radius.clamp(0.0, 0.5), brightness: clamp01(brightness) }
    }

    /// The single flat dot colour for this frame. Neutral, never tinted.
    pub fn color(&self) -> Rgb { palette::DOT.scaled(self.brightness) }

    /// Radius in pixels for a `w × h` surface.
    pub fn radius_px(&self, w: u32, h: u32) -> f32 { self.radius * w.min(h) as f32 }

    /// Web helper: compact JSON for the `/dot` endpoint (no serde). Carries the dot and
    /// the background tokens separately so a client can never mix them.
    pub fn to_json(&self, t_ms: u64) -> String {
        format!(
            "{{\"t_ms\":{t_ms},\"state\":\"{}\",\"radius\":{:.4},\"brightness\":{:.4},\"dot\":\"{}\",\"bg\":{{\"base\":\"{}\",\"from\":\"{}\",\"to\":\"{}\"}}}}",
            self.state.as_str(), self.radius, self.brightness, self.color().hex(),
            palette::OBSIDIAN_HEX, palette::AMBER_HEX, palette::VIOLET_HEX
        )
    }
}

/// Rika lean-in amount (0..=1) carried on the heartbeat frame. galion-console
/// `viz_from_heartbeat` writes it as `speak.emotion = {label: "lean-in", intensity}`.
pub fn lean_in_of(f: &VizFrame) -> f32 {
    if f.speak.emotion.label == "lean-in" { clamp01(f.speak.emotion.intensity) } else { 0.0 }
}

/// Screen sub-state for the single V1 screen (`/dot` JSON `"mode"`): the locked
/// [`DotState`] plus `"rika"` when an idle frame carries a lean-in.
pub fn screen_mode(f: &VizFrame) -> &'static str {
    match DotState::from_frame(f) {
        DotState::Idle if lean_in_of(f) > 0.0 => "rika",
        s => s.as_str(),
    }
}

// ---------------------------------------------------------------------------------------
// Terminal helper (block glyphs; half-blocks give ~square pixels on 1:2 cells).
// ---------------------------------------------------------------------------------------

/// Options for [`render_terminal`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalDotOpts {
    /// Emit 24-bit ANSI colour. When false: plain glyphs only (tests, `NO_COLOR`).
    pub color: bool,
    /// Paint the amber→violet wash in the background cells. Off by default: one bg colour
    /// (obsidian) keeps an SSH session to a few KB per frame.
    pub gradient_bg: bool,
}

impl Default for TerminalDotOpts {
    fn default() -> Self { Self { color: true, gradient_bg: false } }
}

/// Render the dot centred in a `cols × rows` terminal. Returns rows joined with `\n`
/// (no cursor control; the caller owns positioning). Glyphs: `█` `▀` `▄` and space.
/// The dot uses only the foreground colour (flat [`DotParams::color`]); the background
/// colour is only ever set as the cell background.
pub fn render_terminal(d: &DotParams, cols: u16, rows: u16, t_ms: u64, o: TerminalDotOpts) -> String {
    let (cols, rows) = (cols.max(1) as usize, rows.max(1) as usize);
    let (pw, ph) = (cols as f32, (rows * 2) as f32); // pixel grid, 2 px per cell vertically
    let r = d.radius * pw.min(ph);
    let (cx, cy) = (pw / 2.0, ph / 2.0);
    let inside = |x: usize, y: usize| {
        let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
        dx * dx + dy * dy <= r * r
    };
    let fg = d.color();
    let mut s = String::with_capacity(cols * rows * 4 + 64);
    if o.color {
        s.push_str(&format!("\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2));
    }
    let mut last_bg: Option<Rgb> = None;
    for row in 0..rows {
        for col in 0..cols {
            if o.color {
                let bg = if o.gradient_bg {
                    // Quantise to 4 levels/channel steps so runs compress.
                    let b = palette::background_at((col as f32 + 0.5) / pw, (row as f32 + 0.5) / rows as f32, t_ms);
                    Rgb(b.0 & !3, b.1 & !3, b.2 & !3)
                } else {
                    palette::OBSIDIAN
                };
                if last_bg != Some(bg) {
                    s.push_str(&format!("\x1b[48;2;{};{};{}m", bg.0, bg.1, bg.2));
                    last_bg = Some(bg);
                }
            }
            s.push(match (inside(col, row * 2), inside(col, row * 2 + 1)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        if row + 1 < rows {
            if o.color { s.push_str("\x1b[0m"); last_bg = None; s.push('\n'); s.push_str(&format!("\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2)); } else { s.push('\n'); }
        }
    }
    if o.color { s.push_str("\x1b[0m"); }
    s
}

// ---------------------------------------------------------------------------------------
// Window / framebuffer helper (CPU, 0x00RRGGBB — softbuffer, /dev/fb0, DRM dumb buffer).
// ---------------------------------------------------------------------------------------

/// Rasterise one full frame: background wash (or flat obsidian) then the dot.
/// Interior dot pixels are exactly `d.color()`; only the outer edge pixel blends.
/// Panics if `buf.len() != w * h`.
pub fn rasterize(buf: &mut [u32], w: u32, h: u32, d: &DotParams, t_ms: u64, gradient_bg: bool) {
    assert_eq!(buf.len(), (w as usize) * (h as usize), "framebuffer size mismatch");
    if w == 0 || h == 0 { return; }
    let dot = d.color();
    let r = d.radius_px(w, h);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let (fw, fh) = (w as f32, h as f32);
    for y in 0..h {
        let v = (y as f32 + 0.5) / fh;
        let dy = y as f32 + 0.5 - cy;
        let row = &mut buf[(y * w) as usize..((y + 1) * w) as usize];
        for (x, px) in row.iter_mut().enumerate() {
            let dx = x as f32 + 0.5 - cx;
            let dist = (dx * dx + dy * dy).sqrt();
            let cov = (r + 0.5 - dist).clamp(0.0, 1.0);
            *px = if cov >= 1.0 {
                dot.to_u32()
            } else {
                let bg = if gradient_bg { palette::background_at((x as f32 + 0.5) / fw, v, t_ms) } else { palette::OBSIDIAN };
                if cov <= 0.0 { bg.to_u32() } else { bg.lerp(dot, cov).to_u32() }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(listen: bool, speak: bool, t_ms: u64) -> VizFrame {
        let mut f = VizFrame { t_ms, ..Default::default() };
        f.listen.active = listen;
        f.speak.active = speak;
        f.listen.level = 0.5;
        f.speak.level = 0.8;
        f
    }

    #[test]
    fn state_from_heartbeat() {
        assert_eq!(DotState::from_frame(&frame(false, false, 0)), DotState::Idle);
        assert_eq!(DotState::from_frame(&frame(true, false, 0)), DotState::Listening);
        assert_eq!(DotState::from_frame(&frame(false, true, 0)), DotState::Speaking);
        assert_eq!(DotState::from_frame(&frame(true, true, 0)), DotState::Speaking);
    }

    #[test]
    fn radius_and_brightness_ordering() {
        // Averaged over a full idle loop: idle < listening in size and brightness.
        let avg = |s: DotState| {
            let n = 200;
            let (mut r, mut b) = (0.0, 0.0);
            for i in 0..n { let p = DotParams::compute(s, 0.5, i * 16); r += p.radius; b += p.brightness; }
            (r / n as f32, b / n as f32)
        };
        let (ri, bi) = avg(DotState::Idle);
        let (rl, bl) = avg(DotState::Listening);
        let (rs, bs) = avg(DotState::Speaking);
        assert!(ri < rl && bi < bl, "idle {ri},{bi} listening {rl},{bl}");
        assert!(bs >= bl && rs > ri);
    }

    #[test]
    fn speaking_steps_once_per_80ms_frame() {
        for k in 0..16u64 {
            let a = DotParams::compute(DotState::Speaking, 0.3, k * FRAME_MS);
            let b = DotParams::compute(DotState::Speaking, 0.3, k * FRAME_MS + 79);
            assert_eq!(a, b, "changed inside frame {k}");
        }
        let a = DotParams::compute(DotState::Speaking, 0.3, 0);
        let b = DotParams::compute(DotState::Speaking, 0.3, FRAME_MS);
        assert_ne!(a.radius, b.radius);
    }

    #[test]
    fn dot_colour_is_neutral_uniform_scale() {
        for s in [DotState::Idle, DotState::Listening, DotState::Speaking] {
            for t in (0..3200).step_by(80) {
                let c = DotParams::compute(s, 1.0, t).color();
                assert_eq!(c, palette::DOT.scaled(DotParams::compute(s, 1.0, t).brightness));
                // Neutral: R == G, B within 2 of R (DOT is F5F5F7).
                assert_eq!(c.0, c.1);
                assert!(c.2 >= c.0 && c.2 - c.0 <= 2);
            }
        }
    }

    #[test]
    fn deterministic_and_bounded() {
        for t in [0, 1, 79, 80, 1599, 1600, u64::MAX] {
            for l in [-1.0, 0.0, 0.5, 1.0, 9.0, f32::NAN] {
                for s in [DotState::Idle, DotState::Listening, DotState::Speaking] {
                    let p = DotParams::compute(s, l, t);
                    assert_eq!(p, DotParams::compute(s, l, t));
                    assert!((0.0..=0.5).contains(&p.radius) && (0.0..=1.0).contains(&p.brightness));
                }
            }
        }
    }

    #[test]
    fn terminal_plain_is_only_dot_glyphs() {
        let d = DotParams::compute(DotState::Listening, 0.0, 0);
        let s = render_terminal(&d, 40, 12, 0, TerminalDotOpts { color: false, gradient_bg: false });
        assert!(!s.contains('\x1b'));
        assert_eq!(s.lines().count(), 12);
        assert!(s.chars().all(|c| matches!(c, '█' | '▀' | '▄' | ' ' | '\n')));
        assert!(s.contains('█'));
        // Centred: middle row has the dot, first/last rows don't.
        let lines: Vec<&str> = s.lines().collect();
        assert!(lines[0].trim().is_empty() && lines[11].trim().is_empty());
        assert!(lines[6].contains('█'));
    }

    #[test]
    fn terminal_colour_dot_fg_is_flat_dot_colour() {
        let d = DotParams::compute(DotState::Speaking, 1.0, 160);
        let s = render_terminal(&d, 30, 10, 0, TerminalDotOpts { color: true, gradient_bg: true });
        let c = d.color();
        let fg = format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2);
        // Every foreground escape is the dot colour; no amber/violet ever used as fg.
        for (i, _) in s.match_indices("\x1b[38;2;") {
            assert!(s[i..].starts_with(&fg));
        }
    }

    #[test]
    fn framebuffer_dot_interior_flat_edges_obsidian() {
        let (w, h) = (64u32, 48u32);
        let mut fb = vec![0u32; (w * h) as usize];
        let d = DotParams::compute(DotState::Listening, 0.0, 0);
        rasterize(&mut fb, w, h, &d, 0, true);
        let dot = d.color().to_u32();
        assert_eq!(fb[(24 * w + 32) as usize], dot);
        // Every pixel well inside the radius is exactly the dot colour.
        let r = d.radius_px(w, h) - 1.0;
        for y in 0..h { for x in 0..w {
            let (dx, dy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 24.0);
            if (dx * dx + dy * dy).sqrt() < r { assert_eq!(fb[(y * w + x) as usize], dot); }
        }}
        assert_eq!(fb[0], palette::OBSIDIAN.to_u32());
        assert_eq!(fb[(w * h - 1) as usize], palette::OBSIDIAN.to_u32());
    }

    #[test]
    fn rika_lean_in_holds_idle_and_leans_toward_listening() {
        let mut f = frame(false, false, 0);
        assert_eq!(lean_in_of(&f), 0.0);
        assert_eq!(screen_mode(&f), "idle");
        assert_eq!(DotParams::from_frame(&f), DotParams::compute(DotState::Idle, 0.0, 0));
        f.speak.emotion = crate::frame::Emotion { label: "lean-in", intensity: 1.0, valence: 0.0 };
        assert_eq!(screen_mode(&f), "rika");
        let a = DotParams::from_frame(&f);
        assert_eq!(a.state, DotState::Idle);
        // Held breath: no pulse across the idle period.
        for t in (0..IDLE_PERIOD_MS).step_by(80) {
            f.t_ms = t;
            assert_eq!(DotParams::from_frame(&f), a);
        }
        let idle_max = DotParams::IDLE_RADIUS + 0.006;
        assert!(a.radius > idle_max && a.radius < DotParams::LISTEN_RADIUS);
        assert!(a.brightness > 0.87 && a.brightness < 0.96);
        // Listening / speaking win over lean-in.
        f.listen.active = true;
        assert_eq!(screen_mode(&f), "listening");
        assert_eq!(DotParams::from_frame(&f).state, DotState::Listening);
        // Colour stays neutral (uniform scale only).
        assert_eq!(a.color(), palette::DOT.scaled(a.brightness));
    }

    #[test]
    fn web_json_shape() {
        let j = DotParams::compute(DotState::Idle, 0.0, 0).to_json(0);
        assert!(j.contains("\"state\":\"idle\"") && j.contains("\"base\":\"#0A0A0C\""));
        assert!(j.starts_with('{') && j.ends_with('}'));
    }
}
