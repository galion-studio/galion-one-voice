//! Browser entry. The fullscreen page uses this CPU renderer so a missing
//! WebGPU adapter still draws. Quality flags come from the query string.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::demo::frame_at;
use crate::frame::NebulaFrame;
use crate::quality::{
    budget_for, initial_normal_target, ladder_at_most, parse_quality, step_normal_target, DeviceClass,
    GpuClass, QualityMode, TierBudget, NORMAL_FPS_LADDER, NORMAL_PACING, ULTRA_FRAME_BUDGET_MS,
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
}

#[wasm_bindgen]
impl Nebula {
    #[wasm_bindgen(constructor)]
    pub fn new(voice: &str, quality: &str, css_w: f32, css_h: f32, refresh_hz: f32, discrete_gpu: bool) -> Nebula {
        console_error_panic_hook::set_once();
        let voice = Voice::parse(voice).unwrap_or(Voice::Samantha);
        let requested = parse_quality(quality).unwrap_or(QualityMode::Normal);
        let gpu = if discrete_gpu { GpuClass::Discrete } else { GpuClass::Integrated };
        let device = device_from_css(css_w, css_h);
        let mut budget = budget_for(requested, device, refresh_hz, gpu);
        if budget.mode == QualityMode::Normal {
            budget.fps_target = initial_normal_target(refresh_hz, gpu);
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
        let scale = self.budget.render_scale * self.supersample;
        let image = render(&self.sim, &self.frame, (css_w as f32 * scale) as u32, (css_h as f32 * scale) as u32, &self.budget, true);
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
            "{{\"mode\":\"{:?}\",\"pacing\":\"{:?}\",\"fps\":{},\"particles\":{},\"scale\":{},\"overlay\":{},\"supersample\":{}}}",
            self.budget.mode, self.budget.pacing, self.budget.fps_target, self.budget.particles, self.budget.render_scale, self.overlay, self.supersample
        )
    }
}

fn device_from_css(w: f32, h: f32) -> DeviceClass {
    let px = w * h;
    if px < 520_000.0 {
        DeviceClass::Phone
    } else if px < 1_400_000.0 {
        DeviceClass::Laptop
    } else {
        DeviceClass::Desktop
    }
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
        let nebula = slot.get_or_insert_with(|| Nebula::new(voice, "pi", 320.0, 180.0, 30.0, false));
        nebula.frame(33.0, canvas);
    });
}
