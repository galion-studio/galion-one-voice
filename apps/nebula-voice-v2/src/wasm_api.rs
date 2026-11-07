//! Browser entry. The fullscreen page uses this CPU renderer so a missing
//! WebGPU adapter still draws. Quality flags come from the query string.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::demo::frame_at;
use crate::frame::NebulaFrame;
use crate::quality::{
    budget_for, initial_normal_target, ladder_at_most, parse_quality, step_normal_target,
    GpuClass, QualityMode, TierBudget, HIGH_FPS_TARGET, NORMAL_FPS_LADDER, NORMAL_PACING, ULTRA_FRAME_BUDGET_MS,
};
use crate::render::render;
use crate::shade::Voice;
use crate::sim::Sim;

#[wasm_bindgen]
pub struct Nebula {
    voice: Voice,
    sim: Sim,
    frame: NebulaFrame,
    budget: TierBudget,
    refresh_hz: f32,
    gpu: GpuClass,
    yaw: f32,
    distance: f32,
    blend: f32,
    overlay: bool,
    smoothed_ms: f32,
    cool: u32,
    hot: u32,
    supersample: f32,
    sprite_buf: Vec<f32>,
    sprite_count: u32,
}

#[wasm_bindgen]
impl Nebula {
    #[wasm_bindgen(constructor)]
    pub fn new(voice: &str, quality: &str, css_w: f32, css_h: f32, refresh_hz: f32, discrete_gpu: bool) -> Nebula {
        console_error_panic_hook::set_once();
        let voice = Voice::parse(voice).unwrap_or(Voice::Samantha);
        let requested = parse_quality(quality).unwrap_or(QualityMode::Normal);
        let gpu = if discrete_gpu { GpuClass::Discrete } else { GpuClass::Integrated };
        let device = crate::quality::device_from_viewport(css_w, css_h);
        let mut budget = budget_for(requested, device, refresh_hz, gpu);
        if budget.mode == QualityMode::Normal {
            budget.fps_target = initial_normal_target(refresh_hz, gpu);
        }
        if budget.mode == QualityMode::High {
            let refresh = if refresh_hz.is_finite() && refresh_hz > 1.0 { refresh_hz } else { 60.0 };
            budget.fps_target = HIGH_FPS_TARGET.min(ladder_at_most(refresh));
        }
        let particles = budget.particle_ceiling.max(budget.particles);
        Nebula {
            voice,
            sim: Sim::create(11, particles),
            frame: frame_at(voice, 1.2),
            budget,
            refresh_hz: if refresh_hz.is_finite() && refresh_hz > 1.0 { refresh_hz } else { 60.0 },
            gpu,
            yaw: 0.2,
            distance: 3.15,
            blend: 1.2,
            overlay: false,
            smoothed_ms: 1000.0 / budget.fps_target.max(1.0),
            cool: 0,
            hot: 0,
            supersample: 1.0,
            sprite_buf: vec![0.0; particles.max(1) * crate::render::SPRITE_STRIDE * 2],
            sprite_count: 0,
        }
    }

    pub fn frame(&mut self, dt_ms: f32, canvas: &HtmlCanvasElement) {
        let dt = (dt_ms / 1000.0).clamp(1.0 / 240.0, 0.05);
        self.smoothed_ms = self.smoothed_ms * 0.9 + dt_ms.max(0.1) * 0.1;
        self.adapt();
        self.blend += dt;
        self.frame = frame_at(self.voice, self.blend);
        let drive = crate::frame::drive_from(&self.frame, self.budget.particles, self.budget.octaves);
        self.sim.step(&drive, 1.0 / self.budget.fps_target.max(1.0));
        let css_w = canvas.client_width().max(2) as u32;
        let css_h = canvas.client_height().max(2) as u32;
        let dpr = canvas_dpr(canvas, css_w, css_h);
        // `render` already multiplies by `budget.render_scale`. Passing
        // CSS × scale here used to shrink Pi previews to 0.25× and also
        // dropped the device pixel ratio, so phones looked soft.
        let buf_w = ((css_w as f32) * dpr * self.supersample).round().max(2.0) as u32;
        let buf_h = ((css_h as f32) * dpr * self.supersample).round().max(2.0) as u32;
        let image = render(&self.sim, &self.frame, buf_w, buf_h, &self.budget, true);
        paint_canvas(canvas, &image);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw += dx * 0.005;
        let _ = dy;
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(1.8, 7.0);
    }

    pub fn cycle(&mut self) {
        self.blend += 8.0;
    }

    pub fn step(&mut self, dt_ms: f32) {
        let dt = (dt_ms / 1000.0).clamp(1.0 / 240.0, 0.05);
        self.smoothed_ms = self.smoothed_ms * 0.9 + dt_ms.max(0.1) * 0.1;
        self.adapt();
        self.blend += dt;
        self.frame = frame_at(self.voice, self.blend);
        let drive = crate::frame::drive_from(&self.frame, self.budget.particles, self.budget.octaves);
        self.sim.step(&drive, 1.0 / self.budget.fps_target.max(1.0));
    }

    pub fn pack_sprites(&mut self, css_w: f32, css_h: f32, dpr: f32) -> u32 {
        let dpr = if dpr.is_finite() && dpr > 0.0 { dpr } else { 1.0 };
        let buf_w = (css_w.max(2.0) * dpr).round().max(2.0) as u32;
        let buf_h = (css_h.max(2.0) * dpr).round().max(2.0) as u32;
        let need = self.budget.particles.max(1) * crate::render::SPRITE_STRIDE * 2;
        if self.sprite_buf.len() < need {
            self.sprite_buf.resize(need, 0.0);
        }
        self.sprite_count = crate::render::write_sprites(
            &self.sim,
            &self.frame,
            buf_w,
            buf_h,
            &self.budget,
            true,
            &mut self.sprite_buf,
        );
        self.sprite_count
    }

    pub fn sprites_ptr(&self) -> u32 {
        self.sprite_buf.as_ptr() as u32
    }

    pub fn sprites_count(&self) -> u32 {
        self.sprite_count
    }

    pub fn apply_quality(&mut self, quality: &str) {
        let requested = parse_quality(quality).unwrap_or(QualityMode::Normal);
        let mut budget = budget_for(requested, self.budget.device, self.refresh_hz, self.gpu);
        if budget.mode == QualityMode::Normal {
            budget.fps_target = initial_normal_target(self.refresh_hz, self.gpu);
        }
        if budget.mode == QualityMode::High {
            budget.fps_target = HIGH_FPS_TARGET.min(ladder_at_most(self.refresh_hz));
        }
        budget.particle_ceiling = budget.particle_ceiling.min(self.sim.particles.len());
        budget.particles = budget.particles.min(self.sim.particles.len());
        self.budget = budget;
    }

    pub fn floor_rgb(&self) -> String {
        let c = match self.voice {
            Voice::Samantha => [0.012, 0.008, 0.004],
            Voice::Jarvis => [0.004, 0.006, 0.016],
        };
        format!("{{\"r\":{},\"g\":{},\"b\":{}}}", c[0], c[1], c[2])
    }

    pub fn toggle_overlay(&mut self) -> bool {
        self.overlay = !self.overlay;
        self.overlay
    }

    pub fn overlay(&self) -> bool {
        self.overlay
    }

    pub fn dominant_json(&self) -> String {
        let rows = crate::frame::dominant(&self.frame, 3);
        let items: Vec<String> = rows.iter().map(|line| {
            format!(
                "{{\"emotion\":\"{}\",\"gas\":\"{}\",\"nm\":{},\"weight\":{:.3}}}",
                line.emotion, line.gas, line.wavelength_nm, self.frame.weights[nebula_elements::DEMO.iter().position(|d| d.emotion == line.emotion).unwrap_or(0)]
            )
        }).collect();
        format!("[{}]", items.join(","))
    }

    pub fn debug(&self) -> String {
        format!(
            "{{\"mode\":\"{:?}\",\"pacing\":\"{:?}\",\"fps\":{},\"particles\":{},\"scale\":{},\"overlay\":{},\"supersample\":{},\"device\":\"{:?}\"}}",
            self.budget.mode, self.budget.pacing, self.budget.fps_target, self.budget.particles, self.budget.render_scale, self.overlay, self.supersample, self.budget.device
        )
    }
}

#[wasm_bindgen]
pub fn wasm_memory() -> JsValue {
    wasm_bindgen::memory()
}


impl Nebula {
    fn adapt(&mut self) {
        let budget_ms = 1000.0 / self.budget.fps_target.max(1.0);
        match self.budget.pacing {
            NORMAL_PACING => {
                if self.smoothed_ms > budget_ms * 1.25 {
                    self.hot += 1;
                    self.cool = 0;
                    if self.hot > 8 {
                        if self.budget.particles > self.budget.particle_floor {
                            self.budget.particles = (self.budget.particles as f32 * 0.85) as usize;
                        } else {
                            self.budget.fps_target = step_normal_target(self.budget.fps_target, self.refresh_hz, false);
                        }
                        self.hot = 0;
                    }
                } else if self.smoothed_ms < budget_ms * 0.7 {
                    self.cool += 1;
                    self.hot = 0;
                    if self.cool > 12 {
                        if self.budget.particles < self.budget.particle_ceiling {
                            self.budget.particles = (self.budget.particles as f32 * 1.12) as usize;
                            self.budget.particles = self.budget.particles.min(self.budget.particle_ceiling);
                        } else {
                            self.budget.fps_target = step_normal_target(self.budget.fps_target, self.refresh_hz, true);
                        }
                        self.cool = 0;
                    }
                }
                let _ = NORMAL_FPS_LADDER;
                let _ = ladder_at_most(self.refresh_hz);
            }
            crate::quality::FramePacing::FixedCap => {
                if self.smoothed_ms > budget_ms * 1.3 && self.budget.particles > self.budget.particle_floor {
                    self.budget.particles = (self.budget.particles as f32 * 0.85) as usize;
                }
            }
            crate::quality::FramePacing::UncappedMailbox => {
                if self.gpu != GpuClass::Discrete {
                    return;
                }
                if self.smoothed_ms <= ULTRA_FRAME_BUDGET_MS && self.budget.particles < self.budget.particle_ceiling {
                    self.budget.particles = (self.budget.particles + 2_000).min(self.budget.particle_ceiling);
                } else if self.smoothed_ms > ULTRA_FRAME_BUDGET_MS * 1.15 {
                    self.budget.particles = (self.budget.particles * 9 / 10).max(self.budget.particle_floor);
                } else if self.smoothed_ms <= ULTRA_FRAME_BUDGET_MS * 0.7 {
                    self.supersample = self.budget.max_render_scale;
                }
            }
        }
    }
}


fn canvas_dpr(canvas: &HtmlCanvasElement, css_w: u32, css_h: u32) -> f32 {
    if let Some(raw) = canvas.get_attribute("data-dpr") {
        if let Ok(value) = raw.parse::<f32>() {
            if value.is_finite() && value > 0.0 {
                return value;
            }
        }
    }
    let raw = web_sys::window()
        .map(|window| window.device_pixel_ratio() as f32)
        .unwrap_or(1.0);
    capped_dpr(raw, css_w, css_h)
}

fn capped_dpr(raw: f32, css_w: u32, css_h: u32) -> f32 {
    let raw = if raw.is_finite() && raw > 0.0 { raw } else { 1.0 };
    let cap = if css_w.min(css_h) < 800 { 1.5 } else { 2.0 };
    let mut dpr = raw.min(cap);
    let area = css_w as f32 * css_h as f32;
    // Match `chooseDpr()` in boot.js: iPhone 14 CSS size at the 1.5 cap.
    let max_pixels = 390.0 * 844.0 * 1.5 * 1.5;
    if area > 0.0 && area * dpr * dpr > max_pixels {
        dpr = (max_pixels / area).sqrt().max(1.0);
    }
    dpr
}

fn paint_canvas(canvas: &HtmlCanvasElement, image: &crate::render::Image) {
    use wasm_bindgen::JsCast;
    canvas.set_width(image.width);
    canvas.set_height(image.height);
    let ctx = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|c| c.dyn_into::<web_sys::CanvasRenderingContext2d>().ok());
    let Some(ctx) = ctx else { return };
    let data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&image.rgba),
        image.width,
        image.height,
    );
    if let Ok(data) = data {
        let _ = ctx.put_image_data(&data, 0.0, 0.0);
    }
}

fn preview_slot(voice: &str) -> &'static std::thread::LocalKey<RefCell<Option<Nebula>>> {
    thread_local! {
        static SAM: RefCell<Option<Nebula>> = RefCell::new(None);
        static JAR: RefCell<Option<Nebula>> = RefCell::new(None);
    }
    if voice.eq_ignore_ascii_case("jarvis") { &JAR } else { &SAM }
}

#[wasm_bindgen]
pub fn preview_frame(voice: &str, canvas: &HtmlCanvasElement) {
    preview_slot(voice).with(|slot| {
        let mut slot = slot.borrow_mut();
        let nebula = slot.get_or_insert_with(|| Nebula::new(voice, "normal", 320.0, 180.0, 30.0, false));
        nebula.frame(33.0, canvas);
    });
}
