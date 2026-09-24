//! # sse-render -- Pass 2: stateless rendering
//!
//! `render(frame)` takes one frame of the parameter table and produces pixels. Caches hold
//! immutable resources only (textures, static mesh data); Cubism Core's model is updated
//! from the frame's parameters every time, so the output depends on nothing but the frame.
//!
//! ## Hard rule
//! > **This crate must not depend on `sse-timeline` or `sse-scenario`.**
//! > Rendering only ever sees the parameter table (`sse-params`).
//!
//! ## Pipeline (per frame, reference: `docs/reverse/versions/cn-6.4.0/rendering.md`, `live2d.md`)
//! 1. Background: base 2338×1440 with cover-but-never-shrink; crossfade previous → current
//! 2. Each character: Cubism drawables into a transparent 2304×1536 RT (premultiplied,
//!    per-drawable masks, Normal / Additive / Multiply blends), then composited with
//!    `UI/Default` semantics (`rgb × a` again) at `ContentSize.y × baseScale / 1024`
//! 3. `ColorFader`, blur, monotone post effect
//! 4. UI: dialog, banners and text (CPU-rasterised canvas)
//!
//! ## Known deviations (reported in the export notes)
//! - Masks are per-drawable at RT resolution, not the game's shared 1024² × 4 atlas
//! - Hardware bilinear sampling (determinism R-8 asks for manual bilinear)

mod gpu;

use std::collections::BTreeMap;
use std::path::PathBuf;

use sse_assets::Library;
use sse_core::consts;
use sse_params::{FrameState, ParamTable};

pub use gpu::GpuError;

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error(transparent)]
    Gpu(#[from] GpuError),
    #[error(transparent)]
    Asset(#[from] sse_assets::AssetError),
    #[error("{0}")]
    Other(String),
}

/// User-supplied presentation assets that are not in the CDN bundles.
#[derive(Debug, Clone)]
pub struct UiAssets {
    /// Full-screen 1920×1080 overlay of the dialog window.
    pub dialog: Option<PathBuf>,
    /// Full-screen overlay of the telop band.
    pub telop: Option<PathBuf>,
    /// Full-screen overlay of the place-info band.
    pub place_info: Option<PathBuf>,
    pub font_body: PathBuf,
    pub font_name: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
}

impl RenderConfig {
    /// `CanvasScaler` (ScaleWithScreenSize, 1920×1080, match = h/w < 1080/1920 ? 1 : 0).
    pub fn content_size(&self) -> [f32; 2] {
        let [rw, rh] = consts::BASE_SCREEN_SIZE;
        let (w, h) = (self.width as f32, self.height as f32);
        if h / w < rh / rw {
            // match height
            [w * rh / h, rh]
        } else {
            [rw, h * rw / w]
        }
    }

    /// UI units → target pixels.
    pub fn ui_scale(&self) -> f32 {
        self.width as f32 / self.content_size()[0]
    }
}

pub struct Renderer {
    gpu: gpu::Gpu,
    cfg: RenderConfig,
    models: Vec<gpu::GpuModel>,
    images: BTreeMap<String, gpu::Image>,
    ui: UiAssets,
    ui_images: [Option<gpu::Image>; 3],
    body_font: sse_text::Font,
    name_font: sse_text::Font,
    text_key: Option<String>,
    lib: Library,
}

impl Renderer {
    pub fn new(
        lib: &Library,
        table: &ParamTable,
        cfg: RenderConfig,
        ui: UiAssets,
    ) -> Result<Self, RenderError> {
        let mut gpu = gpu::Gpu::new(cfg.width, cfg.height)?;
        let mut models = Vec::new();
        for bundle in &table.models {
            models.push(gpu.load_model(lib, bundle)?);
        }
        let load_ui = |gpu: &mut gpu::Gpu, p: &Option<PathBuf>| -> Result<Option<gpu::Image>, RenderError> {
            match p {
                Some(p) => Ok(Some(gpu.image(&sse_assets::load_png(p)?))),
                None => Ok(None),
            }
        };
        let ui_images = [
            load_ui(&mut gpu, &ui.dialog)?,
            load_ui(&mut gpu, &ui.telop)?,
            load_ui(&mut gpu, &ui.place_info)?,
        ];
        let font = |p: &PathBuf| sse_text::Font::load(p).map_err(|e| RenderError::Other(e.to_string()));
        Ok(Self {
            body_font: font(&ui.font_body)?,
            name_font: font(&ui.font_name)?,
            gpu,
            cfg,
            models,
            images: BTreeMap::new(),
            ui,
            ui_images,
            text_key: None,
            lib: lib.clone(),
        })
    }

    fn image(&mut self, path: &str) -> Result<gpu::ImageId, RenderError> {
        if !self.images.contains_key(path) {
            let img = self.lib.load_png(path)?;
            let gi = self.gpu.image(&img);
            self.images.insert(path.to_owned(), gi);
        }
        Ok(self.images[path].id)
    }

    /// Renders one frame; returns tightly packed RGBA8 rows.
    pub fn render(&mut self, frame: &FrameState) -> Result<Vec<u8>, RenderError> {
        let k = self.cfg.ui_scale();
        let content = self.cfg.content_size();
        let (w, h) = (self.cfg.width as f32, self.cfg.height as f32);
        let mut plan = gpu::FramePlan::default();

        // 1. background
        let [bw, bh] = consts::BACKGROUND_BASE_SIZE;
        let (dx, dy) = (content[0] - bw, content[1] - bh);
        let scale = if dx <= 0.0 && dy <= 0.0 {
            1.0
        } else if dx <= 0.0 {
            content[1] / bh
        } else if dy <= 0.0 {
            content[0] / bw
        } else {
            (content[0] / bw).max(content[1] / bh)
        };
        let bg_rect = [
            (w - bw * scale * k) * 0.5,
            (h - bh * scale * k) * 0.5,
            bw * scale * k,
            bh * scale * k,
        ];
        if let Some(prev) = &frame.background.previous {
            let id = self.image(prev)?;
            plan.scene.push(gpu::QuadDraw::image(id, bg_rect, [1.0; 4]));
        }
        if let Some(cur) = &frame.background.current {
            let id = self.image(cur)?;
            let a = if frame.background.previous.is_some() { frame.background.mix } else { 1.0 };
            plan.scene.push(gpu::QuadDraw::image(id, bg_rect, [1.0, 1.0, 1.0, a]));
        }

        // 2. characters (in order, each composited right after its RT render)
        for c in &frame.characters {
            let [rtw, rth] = consts::LIVE2D_RT_SIZE;
            let s = content[1] * c.scale / consts::LIVE2D_SCALE_REFERENCE_HEIGHT;
            let (qw, qh) = (rtw as f32 * s * k, rth as f32 * s * k);
            let cx = (content[0] * 0.5 + c.x) * k;
            // Vertical anchor not reversed yet (open question #43): the RT top is placed at
            // the screen top, which matches the game's framing (head at the top, hips at
            // the bottom) at 16:9.
            let bottom = qh - c.y * k;
            plan.characters.push(gpu::CharacterDraw {
                model: c.model,
                params: c.params.clone(),
                opacity: c.opacity,
                color: c.color,
                rect: [cx - qw * 0.5, bottom - qh, qw, qh],
            });
        }

        // 3. fader, post
        if frame.fader[3] > 0.0 {
            plan.overlay.push(gpu::QuadDraw::solid([0.0, 0.0, w, h], frame.fader));
        }
        plan.blur = frame.blur;
        plan.camera_color = frame.camera_color;

        // 4. UI
        if frame.movie.is_some() {
            plan.ui.push(gpu::QuadDraw::solid([0.0, 0.0, w, h], [0.0, 0.0, 0.0, 1.0]));
        }
        if let Some(t) = &frame.talk
            && let Some(img) = &self.ui_images[0] {
                plan.ui.push(gpu::QuadDraw::image(img.id, [0.0, 0.0, w, h], [1.0, 1.0, 1.0, t.window_alpha]));
            }
        if let Some(b) = &frame.telop
            && let Some(img) = &self.ui_images[1] {
                plan.ui.push(gpu::QuadDraw::image(img.id, [0.0, 0.0, w, h], [1.0, 1.0, 1.0, b.alpha]));
            }
        if frame.place_info.is_some()
            && let Some(img) = &self.ui_images[2] {
                plan.ui.push(gpu::QuadDraw::image(img.id, [0.0, 0.0, w, h], [1.0; 4]));
            }
        plan.text = Some(self.text_canvas(frame, k)?);

        Ok(self.gpu.render(&mut self.models, &plan)?)
    }

    /// Rasterises all text of the frame; re-uploads only when it changed.
    fn text_canvas(&mut self, frame: &FrameState, k: f32) -> Result<gpu::ImageId, RenderError> {
        let key = format!(
            "{:?}|{:?}|{:?}|{:?}|{}",
            frame.talk.as_ref().map(|t| (&t.name, &t.body, t.visible, (t.window_alpha * 255.0) as u8)),
            frame.telop.as_ref().map(|b| (&b.text, (b.alpha * 255.0) as u8)),
            frame.place_info.as_ref().map(|b| &b.text),
            frame.full_screen_text.as_ref().map(|b| (&b.text, (b.alpha * 255.0) as u8)),
            frame.movie.as_deref().unwrap_or("")
        );
        if self.text_key.as_deref() == Some(key.as_str()) {
            return Ok(self.gpu.text_image());
        }
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let mut canvas = sse_text::Canvas::new(w, h);
        let outline = Some([0.266_667, 0.266_667, 0.4, 0.6]);
        let body = sse_text::Style {
            size: 40.0,
            min_size: 22.0,
            auto_size: true,
            line_spacing: -80.0,
            color: [1.0, 1.0, 1.0, 1.0],
            outline,
        };
        let name = sse_text::Style {
            size: 44.0,
            min_size: 18.0,
            auto_size: false,
            line_spacing: -80.0,
            color: [0.921_568_6, 0.921_568_6, 0.949_019_6, 1.0],
            outline,
        };
        let banner = sse_text::Style { auto_size: false, ..body };
        let frame_at = |x: f32, y: f32, bw: f32, bh: f32, align: f32, valign: f32| sse_text::Frame {
            x: x * k,
            y: y * k,
            width: bw,
            height: bh,
            scale: k,
            align,
            valign,
        };
        if let Some(t) = &frame.talk {
            sse_text::draw(&mut canvas, &self.name_font, &t.name, u32::MAX, frame_at(225.0, 775.0, 1400.0, 60.0, 0.0, 0.0), &name, t.window_alpha);
            sse_text::draw(&mut canvas, &self.body_font, &t.body, t.visible, frame_at(245.0, 845.0, 1368.3, 154.0, 0.0, 0.0), &body, t.window_alpha);
        }
        if let Some(b) = &frame.telop {
            sse_text::draw(&mut canvas, &self.body_font, &b.text, u32::MAX, frame_at(0.0, 480.0, 1920.0, 120.0, 0.5, 0.5), &banner, b.alpha);
        }
        if let Some(b) = &frame.place_info {
            sse_text::draw(&mut canvas, &self.body_font, &b.text, u32::MAX, frame_at(40.0, 31.0, 800.0, 63.0, 0.0, 0.5), &sse_text::Style { size: 36.0, ..banner }, b.alpha);
        }
        if let Some(b) = &frame.full_screen_text {
            sse_text::draw(&mut canvas, &self.body_font, &b.text, u32::MAX, frame_at(160.0, 140.0, 1600.0, 800.0, 0.5, 0.5), &banner, b.alpha);
        }
        if let Some(m) = &frame.movie {
            let msg = format!("[movie: {m}]");
            sse_text::draw(&mut canvas, &self.body_font, &msg, u32::MAX, frame_at(0.0, 500.0, 1920.0, 80.0, 0.5, 0.5), &banner, 0.6);
        }
        self.gpu.upload_text(&canvas.to_rgba8());
        self.text_key = Some(key);
        Ok(self.gpu.text_image())
    }

    pub fn notes() -> Vec<String> {
        vec![
            "masks rendered per drawable at RT resolution (game: shared 1024² × 4 atlas)".into(),
            "text rasterised from Source Han Sans (not TMP SDF); layout approximated".into(),
            "dialog / telop / place-info artwork from user-supplied overlays".into(),
            "character vertical anchor approximated (RT top at screen top, open question #43)".into(),
        ]
    }

    pub fn ui(&self) -> &UiAssets {
        &self.ui
    }
}
