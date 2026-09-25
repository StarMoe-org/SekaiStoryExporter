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
//! ## Pipeline (per frame)
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

mod effect;
mod fx;
mod gpu;
mod hologram;
mod movie;
mod native_ui;
mod particle;

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
    pub font_body: PathBuf,
    pub font_name: PathBuf,
    /// Directory holding the game's UI sprites under their sprite names (`native_ui`).
    pub sprites: PathBuf,
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

/// `ScenarioLayer` (the characters) canvas sorting order: effects sorted
/// below it are drawn before the characters.
const SCENARIO_LAYER_ORDER: i32 = 220;

pub struct Renderer {
    gpu: gpu::Gpu,
    cfg: RenderConfig,
    models: Vec<gpu::GpuModel>,
    images: BTreeMap<String, gpu::Image>,
    ui: UiAssets,
    dialog_overlay: Option<gpu::Image>,
    native: native_ui::NativeUi,
    movie: movie::MovieDecoder,
    movie_image: gpu::Image,
    movie_key: Option<(String, u32)>,
    body_font: sse_text::Font,
    name_font: sse_text::Font,
    text_key: Option<String>,
    lib: Library,
    fps: u32,
    /// `tex_common_tri_01` (the transition triangles' 4×4 atlas) and the prefab's particle
    /// systems, from the `--ui` dir; the transition is not drawn without both.
    fx: Option<(gpu::Image, std::sync::Arc<fx::FxPrefab>)>,
    /// `tex_transition_top` / `tex_transition_left` (the side fade's feathered edges).
    side_edges: Option<(gpu::Image, gpu::Image)>,
    /// Scenario effect prefabs by (bundle, name); `None` when loading failed.
    prefabs: BTreeMap<(String, String), Option<std::sync::Arc<effect::Prefab>>>,
    /// Live instances by (bundle, name, seed): the instance and its age in frames.
    effects: BTreeMap<(String, String, u32), (effect::EffectInstance, u32)>,
    white: gpu::Image,
    /// `holo` (the hologram shader's `_SubTex`) from the `--ui` dir; without it the scan-line
    /// term is left out.
    scan: Option<hologram::ScanTexture>,
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
        let load_ui =
            |gpu: &mut gpu::Gpu, p: &Option<PathBuf>| -> Result<Option<gpu::Image>, RenderError> {
                match p {
                    Some(p) => Ok(Some(gpu.image(&sse_assets::load_png(p)?))),
                    None => Ok(None),
                }
            };
        let dialog_overlay = load_ui(&mut gpu, &ui.dialog)?;
        let native = native_ui::NativeUi::load(&mut gpu, &ui.sprites);
        let movie_image = gpu.image(&image::RgbaImage::new(cfg.width, cfg.height));
        let fx = match (
            sse_assets::load_png(&ui.sprites.join("tex_common_tri_01.png")).ok(),
            fx::FxPrefab::load(&ui.sprites.join(fx::FxPrefab::FILE)).ok(),
        ) {
            (Some(atlas), Some(prefab)) => Some((gpu.image(&atlas), std::sync::Arc::new(prefab))),
            _ => None,
        };
        let edge = |n: &str| sse_assets::load_png(&ui.sprites.join(n)).ok();
        let side_edges = match (
            edge("tex_transition_top.png"),
            edge("tex_transition_left.png"),
        ) {
            (Some(t), Some(l)) => Some((gpu.image(&t), gpu.image(&l))),
            _ => None,
        };
        let scan = hologram::ScanTexture::load(&ui.sprites.join(hologram::ScanTexture::FILE));
        let font =
            |p: &PathBuf| sse_text::Font::load(p).map_err(|e| RenderError::Other(e.to_string()));
        Ok(Self {
            body_font: font(&ui.font_body)?,
            name_font: font(&ui.font_name)?,
            cfg,
            models,
            images: BTreeMap::new(),
            ui,
            dialog_overlay,
            native,
            movie: movie::MovieDecoder::new(
                "ffmpeg".into(),
                cfg.width,
                cfg.height,
                movie_rect(&cfg),
                table.fps,
            ),
            movie_image,
            movie_key: None,
            text_key: None,
            lib: lib.clone(),
            fps: table.fps,
            fx,
            side_edges,
            prefabs: BTreeMap::new(),
            effects: BTreeMap::new(),
            white: gpu.image(&image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([255, 255, 255, 255]),
            )),
            scan,
            gpu,
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
            let a = if frame.background.previous.is_some() {
                frame.background.mix
            } else {
                1.0
            };
            plan.scene
                .push(gpu::QuadDraw::image(id, bg_rect, [1.0, 1.0, 1.0, a]));
        }

        // 2. characters (in order, each composited right after its RT render)
        for c in &frame.characters {
            let [rtw, rth] = consts::LIVE2D_RT_SIZE;
            let s = content[1] * c.scale / consts::LIVE2D_SCALE_REFERENCE_HEIGHT;
            let (qw, qh) = (rtw as f32 * s * k, rth as f32 * s * k);
            let cx = (content[0] * 0.5 + c.x) * k;
            // `Live2DModelView.UpdateRenderOrientation`: anchors (0.5, 0), prefab
            // pivot (0.5, 0), `anchoredPosition` = transform data (x, y) — the RT's bottom
            // edge sits on the screen's bottom edge (`scenarioLayer` fills the screen).
            let top = h - qh - c.y * k;
            plan.characters.push(gpu::CharacterDraw {
                model: c.model,
                params: c.params.clone(),
                opacity: c.opacity,
                color: c.color,
                rect: [cx - qw * 0.5, top, qw, qh],
                hologram: c.hologram.map(|h| gpu::Hologram {
                    line: h.line,
                    alpha: h.alpha,
                    // no texture: a sample above every `_Line` adds nothing
                    scan: self.scan.as_ref().map_or(1.0, |s| s.sample(h.time)),
                }),
            });
        }

        // ShakeScreen moves `ScenarioRoot` (background, characters) in canvas pixels, +y up
        let [sx, sy] = frame.scenario_shake;
        if sx != 0.0 || sy != 0.0 {
            let (ox, oy) = (sx * k, -sy * k);
            for q in &mut plan.scene {
                q.translate(ox, oy);
            }
            for c in &mut plan.characters {
                c.rect[0] += ox;
                c.rect[1] += oy;
            }
        }

        // `PlayScenarioEffect` prefabs (EffectLayer), in canvas pixels → target pixels
        self.scenario_effects(frame, &mut plan, content, k, [w, h])?;

        // EffectLayer: `fx_transition_scenario`, re-simulated from its instantiation
        if let (Some(fx), Some((atlas, prefab))) = (&frame.fx, &self.fx) {
            let mut sim = fx::TransitionFx::new(prefab.clone(), fx.seed);
            let dt = 1.0 / self.fps as f32;
            for _ in 0..fx.age_frames {
                sim.step(dt);
            }
            for (mat, corners, uv, color) in sim.billboards([w, h]) {
                // corners are (-,-) (+,-) (+,+) (-,+) with y up; `uv` is Unity's v-up tile
                // rect, and our atlas rows are top-down, so v flips
                let [u0, v0, u1, v1] = uv;
                plan.particles.push(gpu::ParticleDraw {
                    additive: mat == fx::Material::Additive,
                    image: atlas.id,
                    corners,
                    uvs: [
                        [u0, 1.0 - v0],
                        [u1, 1.0 - v0],
                        [u1, 1.0 - v1],
                        [u0, 1.0 - v1],
                    ],
                    color,
                });
            }
        }

        // 3. fader, post
        if frame.fader[3] > 0.0 {
            plan.overlay
                .push(gpu::QuadDraw::solid([0.0, 0.0, w, h], frame.fader));
        }
        plan.blur = frame.blur;
        plan.camera_color = frame.camera_color;

        // 4. UI
        if let Some(m) = &frame.movie {
            plan.ui_top
                .push(gpu::QuadDraw::solid([0.0, 0.0, w, h], [0.0, 0.0, 0.0, 1.0]));
            if let Some(file) = &m.file {
                let index = (m.time * self.movie_fps() as f32).round() as u32;
                let key = (file.clone(), index);
                if self.movie_key.as_ref() != Some(&key) {
                    let path = self.lib.path(file);
                    let rgba = self
                        .movie
                        .frame(&path, index)
                        .map_err(RenderError::Other)?
                        .to_vec();
                    self.gpu.upload_image(self.movie_image.id, &rgba);
                    self.movie_key = Some(key);
                }
                plan.ui_top.push(gpu::QuadDraw::image(
                    self.movie_image.id,
                    [0.0, 0.0, w, h],
                    [1.0; 4],
                ));
            }
        }
        if let Some(t) = &frame.talk {
            if self.native.has_window() {
                let auto_w = self.name_font.preferred_width(
                    "AUTO",
                    native_ui::layout::AUTO_TEXT_SIZE,
                    native_ui::layout::AUTO_TEXT_SPACING,
                );
                // ShakeWindow moves `windowRectTransform`, which the prefab points at
                // `Window/ContentRoot/Content/Text`: only the name and words layers (shaken in
                // `text_canvas`). The window, name bar and AUTO signal stay put.
                self.native
                    .talk(&mut plan.ui, k, t.window_alpha, t.auto_time, auto_w);
            } else if let Some(img) = &self.dialog_overlay {
                plan.ui.push(gpu::QuadDraw::image(
                    img.id,
                    [0.0, 0.0, w, h],
                    [1.0, 1.0, 1.0, t.window_alpha],
                ));
            }
        }
        if frame.menu_alpha > 0.0 {
            self.native.menu(&mut plan.ui, k, frame.menu_alpha);
        }
        native_ui::cinemascope(&mut plan.ui_top, k, w, h, frame.cinemascope);
        let telop_text = frame
            .telop
            .as_ref()
            .map(|t| self.native.telop(&mut plan.ui, k, t.show, t.hide));
        if let Some(p) = &frame.place_info {
            self.native.place_info(&mut plan.ui, k, p.x);
        }
        plan.text = Some(self.text_canvas(frame, k, telop_text)?);
        if let Some(p) = frame.side_fade {
            self.side_fade(&mut plan.cover, k, content, p);
        }

        Ok(self.gpu.render(&mut self.models, &plan)?)
    }

    /// Steps (or rebuilds) every live scenario effect to its age and plans its quads.
    fn scenario_effects(
        &mut self,
        frame: &FrameState,
        plan: &mut gpu::FramePlan,
        content: [f32; 2],
        k: f32,
        screen: [f32; 2],
    ) -> Result<(), RenderError> {
        let dt = 1.0 / self.fps as f32;
        let mut keep = std::collections::BTreeSet::new();
        // prefabs attached to a model view: its RectTransform (pivot (0.5, 0) at the
        // transform data, sizeDelta = RT size, localScale = contentH × baseScale / 1024)
        let mut parents = BTreeMap::new();
        for e in &frame.effects {
            let Some(id) = e.character else { continue };
            let Some(c) = frame.characters.iter().find(|c| c.character == id) else {
                continue;
            };
            let s = content[1] * c.scale / consts::LIVE2D_SCALE_REFERENCE_HEIGHT;
            let [rtw, rth] = consts::LIVE2D_RT_SIZE.map(|v| v as f32);
            parents.insert(
                (e.bundle.clone(), e.name.clone(), e.seed),
                effect::Parent {
                    matrix: [s, 0.0, c.x, 0.0, s, c.y - content[1] * 0.5],
                    rect: [-rtw * 0.5, 0.0, rtw, rth],
                },
            );
        }
        for e in &frame.effects {
            let pk = (e.bundle.clone(), e.name.clone());
            if !self.prefabs.contains_key(&pk) {
                let p = effect::Prefab::load(&self.lib, &e.bundle, &e.name)
                    .ok()
                    .map(std::sync::Arc::new);
                self.prefabs.insert(pk.clone(), p);
            }
            let Some(prefab) = self.prefabs[&pk].clone() else {
                continue;
            };
            let key = (e.bundle.clone(), e.name.clone(), e.seed);
            keep.insert(key.clone());
            let stale = self
                .effects
                .get(&key)
                .is_none_or(|(_, age)| *age > e.age_frames);
            if stale {
                self.effects.insert(
                    key.clone(),
                    (effect::EffectInstance::new(prefab, e.seed, dt), 0),
                );
            }
            let (inst, age) = self.effects.get_mut(&key).expect("inserted");
            if e.stop_age == Some(*age) {
                inst.stop();
            }
            while *age < e.age_frames {
                inst.step();
                *age += 1;
                if e.stop_age == Some(*age) {
                    inst.stop();
                }
            }
        }
        self.effects.retain(|k, _| keep.contains(k));
        let [sx, sy] = frame.scenario_shake;
        let to_screen = |p: [f32; 2]| {
            [
                screen[0] * 0.5 + (p[0] + sx) * k,
                screen[1] * 0.5 - (p[1] + sy) * k,
            ]
        };
        let attached: std::collections::BTreeSet<_> = frame
            .effects
            .iter()
            .filter(|e| e.character.is_some())
            .map(|e| (e.bundle.clone(), e.name.clone(), e.seed))
            .collect();
        let quads: Vec<effect::EffectQuad> = self
            .effects
            .iter()
            .flat_map(|(key, (inst, _))| match parents.get(key) {
                Some(p) => inst.quads_under(content, p),
                // attached to a character that is not drawn this frame
                None if attached.contains(key) => Vec::new(),
                None => inst.quads(content),
            })
            .collect();
        if std::env::var_os("SSE_DEBUG_EFFECTS").is_some() {
            for ((b, n, s), (inst, age)) in &self.effects {
                let q = inst.quads(content);
                eprintln!(
                    "effect {b} {n} seed {s} age {age} finished {} quads {}",
                    inst.finished,
                    q.len()
                );
                for x in q.iter().take(4) {
                    eprintln!(
                        "   order {} corners {:?} color {:?} tex {:?}",
                        x.order, x.corners, x.color, x.tex
                    );
                }
            }
        }
        for q in quads {
            let image = match &q.tex {
                Some(t) => self.image(t)?,
                None => self.white.id,
            };
            // `EffectQuad` UVs are image-space (v down); ParticleDraw takes per-corner UVs
            let [u0, v0, u1, v1] = q.uv;
            let d = gpu::ParticleDraw {
                additive: q.blend == effect::Blend::Additive,
                image,
                corners: q.corners.map(to_screen),
                uvs: [[u0, v1], [u1, v1], [u1, v0], [u0, v0]],
                color: q.color,
            };
            if q.order < SCENARIO_LAYER_ORDER {
                plan.effects_back.push(d);
            } else {
                plan.effects_front.push(d);
            }
        }
        Ok(())
    }

    /// `SideFadePlayer` prefab (JP 6.8.1 `resources.assets|121155`): the root stretches over the
    /// canvas with sizeDelta 64×64 and is a black `AtlasImage`; four 512-px `CustomImage`
    /// children in Tiled mode sit outside its edges — `top` (anchors top, pivot (0,0)) and
    /// `bottom` (its y-mirror) with `tex_transition_top`, `left` (anchors left, pivot (1,0)) and
    /// `right` (its x-mirror) with `tex_transition_left`. Sprite PPU 1 on a reference-PPU-1
    /// canvas → 512-px tiles; UGUI's tiled path maps each tile onto the sprite's `textureRect`
    /// (top: Unity y 0–384 of 512 → image rows 128–512; left: x 128–512), starting at the
    /// rect's minimum corner. `p` is the root's anchoredPosition (+y up).
    fn side_fade(&self, out: &mut Vec<gpu::QuadDraw>, k: f32, content: [f32; 2], p: [f32; 2]) {
        const EDGE: f32 = 512.0;
        const TRIM: f32 = 128.0 / 512.0;
        let [cw, ch] = content;
        // root rect in canvas pixels, top-left origin
        let (x0, y0, rw, rh) = (-32.0 + p[0], -32.0 - p[1], cw + 64.0, ch + 64.0);
        let px = |r: [f32; 4]| [r[0] * k, r[1] * k, r[2] * k, r[3] * k];
        out.push(gpu::QuadDraw::solid(
            px([x0, y0, rw, rh]),
            [0.0, 0.0, 0.0, 1.0],
        ));
        let Some((top, left)) = &self.side_edges else {
            return;
        };
        // horizontal strips: tiles run left → right from x0
        let mut x = 0.0;
        while x < rw {
            let tw = (rw - x).min(EDGE);
            let u1 = tw / EDGE;
            // top: dense rows (image bottom) against the panel
            out.push(gpu::QuadDraw::image_uv(
                top.id,
                px([x0 + x, y0 - EDGE, tw, EDGE]),
                [0.0, TRIM, u1, 1.0],
                [1.0; 4],
            ));
            // bottom: y-mirror
            out.push(gpu::QuadDraw::image_uv(
                top.id,
                px([x0 + x, y0 + rh, tw, EDGE]),
                [0.0, 1.0, u1, TRIM],
                [1.0; 4],
            ));
            x += EDGE;
        }
        // vertical strips: tiles run bottom → top (Unity y) from the root's bottom
        let mut y = 0.0;
        while y < rh {
            let th = (rh - y).min(EDGE);
            let v0 = 1.0 - th / EDGE;
            let ty = y0 + rh - y - th;
            // left: dense columns (image right) against the panel
            out.push(gpu::QuadDraw::image_uv(
                left.id,
                px([x0 - EDGE, ty, EDGE, th]),
                [TRIM, v0, 1.0, 1.0],
                [1.0; 4],
            ));
            // right: x-mirror
            out.push(gpu::QuadDraw::image_uv(
                left.id,
                px([x0 + rw, ty, EDGE, th]),
                [1.0, v0, TRIM, 1.0],
                [1.0; 4],
            ));
            y += EDGE;
        }
    }

    /// Rasterises all text of the frame; re-uploads only when it changed.
    fn text_canvas(
        &mut self,
        frame: &FrameState,
        k: f32,
        telop: Option<(f32, f32)>,
    ) -> Result<gpu::ImageId, RenderError> {
        let key = format!(
            "{:?}|{:?}|{:?}|{:?}|{}",
            frame.talk.as_ref().map(|t| {
                let s = frame.window_shake;
                (
                    &t.name,
                    &t.body,
                    t.visible,
                    (t.window_alpha * 255.0) as u8,
                    (s[0] * 4.0) as i32,
                    (s[1] * 4.0) as i32,
                )
            }),
            frame.telop.as_ref().map(|b| (
                &b.text,
                telop.map(|(x, a)| ((x * 4.0) as i32, (a * 255.0) as u8))
            )),
            frame
                .place_info
                .as_ref()
                .map(|b| (&b.text, (b.x * 4.0) as i32)),
            frame.full_screen_text.as_ref().map(|b| (
                &b.text,
                (b.progress * 64.0) as u32,
                (b.alpha * 255.0) as u8
            )),
            frame.movie.as_ref().map_or("", |m| if m.file.is_some() {
                ""
            } else {
                m.name.as_str()
            })
        );
        if self.text_key.as_deref() == Some(key.as_str()) {
            return Ok(self.gpu.text_image());
        }
        let (w, h) = (self.cfg.width as usize, self.cfg.height as usize);
        let mut canvas = sse_text::Canvas::new(w, h);
        let outline = Some([0.266_667, 0.266_667, 0.4, 0.6]);
        // `Words`: enableAutoSizing picks the largest size in [22, 44] that fits
        let body = sse_text::Style {
            size: 44.0,
            min_size: 22.0,
            auto_size: true,
            line_spacing: -80.0,
            color: [1.0, 1.0, 1.0, 1.0],
            outline,
            underlay: None,
            char_spacing: 0.0,
        };
        let name = sse_text::Style {
            size: 44.0,
            min_size: 18.0,
            auto_size: false,
            line_spacing: -80.0,
            color: [0.921_568_6, 0.921_568_6, 0.949_019_6, 1.0],
            outline,
            underlay: None,
            char_spacing: 0.0,
        };
        let banner = sse_text::Style {
            auto_size: false,
            ..body
        };
        let frame_at =
            |x: f32, y: f32, bw: f32, bh: f32, align: f32, valign: f32| sse_text::Frame {
                x: x * k,
                y: y * k,
                width: bw,
                height: bh,
                scale: k,
                align,
                valign,
            };
        let rect =
            |r: [f32; 4], align: f32, valign: f32| frame_at(r[0], r[1], r[2], r[3], align, valign);
        if let Some(t) = &frame.talk {
            use native_ui::layout as l;
            let [wx, wy] = frame.window_shake;
            let shaken = |r: [f32; 4]| [r[0] + wx, r[1] - wy, r[2], r[3]];
            sse_text::draw(
                &mut canvas,
                &self.name_font,
                &t.name,
                u32::MAX,
                rect(shaken(l::NAME), 0.0, 0.0),
                &name,
                t.window_alpha,
            );
            sse_text::draw(
                &mut canvas,
                &self.body_font,
                &t.body,
                t.visible,
                rect(shaken(l::WORDS), 0.0, 0.0),
                &body,
                t.window_alpha,
            );
            if self.native.has_window() {
                // `AutoSignalText`: 32, white, centre / middle, characterSpacing −4, no outline
                let auto = sse_text::Style {
                    size: 32.0,
                    auto_size: false,
                    outline: None,
                    color: [1.0; 4],
                    char_spacing: l::AUTO_TEXT_SPACING,
                    ..name
                };
                let auto_w =
                    self.name_font
                        .preferred_width("AUTO", l::AUTO_TEXT_SIZE, l::AUTO_TEXT_SPACING);
                sse_text::draw(
                    &mut canvas,
                    &self.name_font,
                    "AUTO",
                    u32::MAX,
                    rect(shaken(l::auto_text(auto_w)), 0.5, 0.5),
                    &auto,
                    t.window_alpha,
                );
            }
        }
        let plain = sse_text::Style {
            auto_size: false,
            outline: None,
            ..body
        };
        if let (Some(b), Some((x, a))) = (&frame.telop, telop) {
            let [l, t, w, h] = native_ui::layout::TELOP_TEXT;
            sse_text::draw(
                &mut canvas,
                &self.body_font,
                &b.text,
                u32::MAX,
                rect([l + x, t, w, h], 0.5, 0.5),
                &plain,
                a,
            );
        }
        if let Some(p) = &frame.place_info {
            let [l, t, w, h] = native_ui::layout::PLACE_TEXT;
            sse_text::draw(
                &mut canvas,
                &self.body_font,
                &p.text,
                u32::MAX,
                rect([l + p.x, t, w, h], 0.0, 0.5),
                &sse_text::Style {
                    size: 40.0,
                    ..plain
                },
                1.0,
            );
        }
        if let Some(b) = &frame.full_screen_text {
            // `ScenarioFullScreenTextDialog/Text` (`resources.assets|576995`): 56, left,
            // middle, line spacing −32, word wrap; `TextAppearFade` per character
            let fst = sse_text::Style {
                size: 56.0,
                min_size: 56.0,
                auto_size: false,
                line_spacing: -32.0,
                color: [1.0; 4],
                outline: None,
                // SDF_Base_Scenario_Full: underlay black, offset (0, −1) → 1 × GradientScale 6 ×
                // ScaleRatioC 0.677 atlas texels at point size 35
                underlay: Some(([0.0, 0.0, 0.0, 1.0], 6.0 * 0.677_083_3 / 35.0)),
                char_spacing: 0.0,
            };
            let text = b.text.trim_start_matches(['\n', '\r']);
            let progress = b.progress;
            sse_text::draw_faded(
                &mut canvas,
                &self.body_font,
                text,
                u32::MAX,
                rect(native_ui::layout::FST_TEXT, 0.0, 0.5),
                &fst,
                b.alpha,
                &|i| (progress - i as f32).clamp(0.0, 1.0),
            );
        }
        if let Some(m) = frame.movie.as_ref().filter(|m| m.file.is_none()) {
            let msg = format!("[movie: {}]", m.name);
            sse_text::draw(
                &mut canvas,
                &self.body_font,
                &msg,
                u32::MAX,
                frame_at(0.0, 500.0, 1920.0, 80.0, 0.5, 0.5),
                &banner,
                0.6,
            );
        }
        self.gpu.upload_text(&canvas.to_rgba8());
        self.text_key = Some(key);
        Ok(self.gpu.text_image())
    }

    pub fn notes() -> Vec<String> {
        vec![
            "masks rendered per drawable at RT resolution (game: shared 1024² × 4 atlas)".into(),
            "text rasterised from the --ui font (CN Source Han Sans SC, JP FOT-RodinNTLG Pro; not TMP SDF); boxes, sizes and underlay from the prefabs, TMP line breaking approximated".into(),
            "scenario UI rebuilt from the prefabs (talk-window sprites user-supplied)".into(),
        ]
    }

    /// Movie frames are decoded at the table's frame rate.
    fn movie_fps(&self) -> u32 {
        self.fps
    }

    /// Uses this `ffmpeg` for movie frames.
    pub fn set_ffmpeg(&mut self, ffmpeg: PathBuf) {
        self.movie = movie::MovieDecoder::new(
            ffmpeg,
            self.cfg.width,
            self.cfg.height,
            movie_rect(&self.cfg),
            self.fps,
        );
    }

    /// Notes that depend on the supplied assets.
    pub fn asset_notes(&self) -> Vec<String> {
        let mut notes: Vec<String> = self
            .native
            .missing
            .iter()
            .map(|s| format!("UI sprite {s}.png not supplied; that element is not drawn"))
            .collect();
        if self.fx.is_none() {
            notes.push(format!(
                "tex_common_tri_01.png or {} not supplied; the transition particles are not drawn",
                fx::FxPrefab::FILE
            ));
        }
        if self.side_edges.is_none() {
            notes.push(
                "tex_transition_top.png / tex_transition_left.png not supplied; side fades are not drawn"
                    .into(),
            );
        }
        notes
    }

    pub fn ui(&self) -> &UiAssets {
        &self.ui
    }
}

/// `ScenarioPlayer.movieResolution` (2338, 1080) in target pixels.
fn movie_rect(cfg: &RenderConfig) -> (u32, u32) {
    let k = cfg.ui_scale();
    let [w, h] = consts::MOVIE_RESOLUTION;
    ((w * k).round() as u32, (h * k).round() as u32)
}
