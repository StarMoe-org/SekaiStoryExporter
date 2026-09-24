//! The scenario UI rebuilt from the game's prefabs (`TalkWindow` `resources.assets|7281`,
//! `ScreenLayerScenario` `|6451`, `ScenarioFullScreenTextDialog` `|44706`); geometry and
//! colours in `docs/reverse/notes/2026-09-24-native-capture-visual.md`.
//!
//! The sprites come from the game client (`data.unity3d`, not the CDN bundles), so the user
//! supplies them in the `--ui` directory under their sprite names. A missing sprite drops
//! that element and is reported.

use std::path::Path;

use image::RgbaImage;

use crate::gpu::{self, QuadDraw};

/// Reference-pixel rectangles (1920×1080, y down).
pub mod layout {
    /// `TalkWindow/Window`: bottom-centre anchored 2800×312.
    pub const WINDOW: [f32; 4] = [-440.0, 768.0, 2800.0, 312.0];
    /// `Window` `CustomImage.m_Color`.
    pub const WINDOW_COLOR: [f32; 4] = [0.0, 0.0, 0.133_333_34, 1.0];
    /// `NameBackground` (16×720 rotated +90°, pivot top-left at Content + (586, −48)).
    pub const NAME_BAR: [f32; 4] = [210.0, 824.0, 720.0, 16.0];
    /// `GradientAlpha` on `NameBackground`: alpha at the name end (the far end is 0).
    pub const NAME_BAR_ALPHA: f32 = 0.6;
    /// `Name`: 593×44, top-left aligned.
    pub const NAME: [f32; 4] = [230.0, 790.0, 593.0, 44.0];
    /// `Words`: 1368.3×154, top-left aligned.
    pub const WORDS: [f32; 4] = [250.0, 858.0, 1368.3, 154.0];
    /// `AutoSignal`: 200×48 at Content's bottom-right − (440, 0).
    pub const AUTO: [f32; 4] = [1656.0, 1008.0, 200.0, 48.0];
    /// `AutoSignal/bg` colour.
    pub const AUTO_BG_COLOR: [f32; 4] = [0.266_666_68, 0.266_666_68, 0.4, 0.8];
    /// `AutoSignalIcon1` (28×22, rotated −90° about its centre) after the
    /// `HorizontalLayoutGroup` (MiddleCenter, spacing 6): centre (1716, 1032).
    pub const AUTO_ICON: [f32; 4] = [1705.0, 1018.0, 22.0, 28.0];
    /// `AutoSignalText` (73.82×32).
    pub const AUTO_TEXT: [f32; 4] = [1736.0, 1016.0, 73.82, 32.0];
    /// `TweenAlpha` on the icon: visible for the first 0.5028 s of each 1 s loop.
    pub const AUTO_ICON_ON: f32 = 0.502_762_44;
    /// `UIPartsMenuButton`: 96×96 centred at top-right − (64, 64).
    pub const MENU: [f32; 4] = [1808.0, 16.0, 96.0, 96.0];
    /// `MenuIcon`: 52×12, centred.
    pub const MENU_ICON: [f32; 4] = [1830.0, 58.0, 52.0, 12.0];
    pub const MENU_ICON_COLOR: [f32; 4] = [0.266_666_68, 0.266_666_68, 0.4, 1.0];
    /// `ScenarioFullScreenTextDialog/Content/Text`: stretched, size (−280, −480), pos (140, 0).
    pub const FST_TEXT: [f32; 4] = [280.0, 240.0, 1640.0, 600.0];
}

pub struct NativeUi {
    window: Option<gpu::Image>,
    name_bar: Option<gpu::Image>,
    auto_bg: Option<gpu::Image>,
    auto_icon: Option<gpu::Image>,
    menu: Option<gpu::Image>,
    menu_icon: Option<gpu::Image>,
    pub missing: Vec<&'static str>,
}

fn load(dir: &Path, name: &'static str, missing: &mut Vec<&'static str>) -> Option<RgbaImage> {
    match sse_assets::load_png(&dir.join(format!("{name}.png"))) {
        Ok(img) => Some(img),
        Err(_) => {
            missing.push(name);
            None
        }
    }
}

impl NativeUi {
    pub fn load(gpu: &mut gpu::Gpu, dir: &Path) -> Self {
        let mut missing = Vec::new();
        let window = load(dir, "bg_story_adv", &mut missing).map(|i| gpu.image(&i));
        let name_bar = load(dir, "bg_base_half_r8_wh", &mut missing).map(|i| gpu.image(&name_bar(&i)));
        let auto_bg = load(dir, "bg_base_round_h48_wh", &mut missing)
            .map(|i| gpu.image(&slice_h(&i, 25, 25, layout::AUTO[2] as u32)));
        let auto_icon = load(dir, "icon_triangle_h22_wh", &mut missing)
            .map(|i| gpu.image(&image::imageops::rotate90(&i)));
        let menu = load(dir, "btn_circle_h80_wh", &mut missing).map(|i| gpu.image(&i));
        let menu_icon = load(dir, "icon_menu_story_wh", &mut missing).map(|i| gpu.image(&i));
        Self { window, name_bar, auto_bg, auto_icon, menu, menu_icon, missing }
    }

    pub fn has_window(&self) -> bool {
        self.window.is_some()
    }

    /// Talk window background, name bar and auto signal. `auto_time` is the seconds since
    /// the auto signal was enabled (drives the icon blink).
    pub fn talk(&self, out: &mut Vec<QuadDraw>, k: f32, alpha: f32, auto_time: f32) {
        let r = |a: [f32; 4]| [a[0] * k, a[1] * k, a[2] * k, a[3] * k];
        let with = |c: [f32; 4], a: f32| [c[0], c[1], c[2], c[3] * a];
        if let Some(w) = &self.window {
            out.push(QuadDraw::image(w.id, r(layout::WINDOW), with(layout::WINDOW_COLOR, alpha)));
        }
        if let Some(b) = &self.name_bar {
            out.push(QuadDraw::image(b.id, r(layout::NAME_BAR), [1.0, 1.0, 1.0, alpha]));
        }
        if let Some(b) = &self.auto_bg {
            out.push(QuadDraw::image(b.id, r(layout::AUTO), with(layout::AUTO_BG_COLOR, alpha)));
        }
        if let Some(i) = &self.auto_icon
            && auto_time.rem_euclid(1.0) < layout::AUTO_ICON_ON
        {
            out.push(QuadDraw::image(i.id, r(layout::AUTO_ICON), [1.0, 1.0, 1.0, alpha]));
        }
    }

    pub fn menu(&self, out: &mut Vec<QuadDraw>, k: f32, alpha: f32) {
        let r = |a: [f32; 4]| [a[0] * k, a[1] * k, a[2] * k, a[3] * k];
        if let Some(m) = &self.menu {
            out.push(QuadDraw::image(m.id, r(layout::MENU), [1.0, 1.0, 1.0, alpha]));
        }
        if let Some(i) = &self.menu_icon {
            let c = layout::MENU_ICON_COLOR;
            out.push(QuadDraw::image(i.id, r(layout::MENU_ICON), [c[0], c[1], c[2], c[3] * alpha]));
        }
    }
}

/// `PlayCinemascope`: `Base` (black, alpha 0.5·e) and the two black bars (height 240·e).
pub fn cinemascope(out: &mut Vec<QuadDraw>, k: f32, w: f32, h: f32, e: f32) {
    if e <= 0.0 {
        return;
    }
    out.push(QuadDraw::solid([0.0, 0.0, w, h], [0.0, 0.0, 0.0, sse_core::consts::FST_BASE_ALPHA * e]));
    let bar = sse_core::consts::FST_CINEMASCOPE_HEIGHT * e * k;
    out.push(QuadDraw::solid([0.0, 0.0, w, bar], [0.0, 0.0, 0.0, 1.0]));
    out.push(QuadDraw::solid([0.0, h - bar, w, bar], [0.0, 0.0, 0.0, 1.0]));
}

/// Nine-slice along x only (Unity `Image.Type.Sliced` with top/bottom borders of 0).
fn slice_h(src: &RgbaImage, left: u32, right: u32, out_w: u32) -> RgbaImage {
    let (sw, sh) = src.dimensions();
    let mut out = RgbaImage::new(out_w, sh);
    let centre_src = (sw - left - right).max(1) as f32;
    let centre_dst = (out_w - left - right).max(1) as f32;
    for x in 0..out_w {
        let sx = if x < left {
            x
        } else if x >= out_w - right {
            sw - (out_w - x)
        } else {
            left + (((x - left) as f32 + 0.5) / centre_dst * centre_src - 0.5).round().clamp(0.0, centre_src - 1.0) as u32
        };
        for y in 0..sh {
            out.put_pixel(x, y, *src.get_pixel(sx, y));
        }
    }
    out
}

/// `NameBackground`: `bg_base_half_r8_wh` (16×10, border L7 R7 B0 T8) sliced into a 16×720
/// rect, rotated +90° about its top-left pivot, with `GradientAlpha` (top 0.6 → bottom 0).
/// The unrotated top becomes the left end; the unrotated x axis points up.
fn name_bar(src: &RgbaImage) -> RgbaImage {
    let [_, _, len, thick] = layout::NAME_BAR;
    let (len, thick) = (len as u32, thick as u32);
    let (sw, sh) = src.dimensions();
    let top = 8u32;
    let mut out = RgbaImage::new(len, thick);
    for d in 0..len {
        // distance from the unrotated top → source row (top border fixed, the rest stretched)
        let sy = if d < top {
            d
        } else {
            let span = (sh - top).max(1) as f32;
            top + (((d - top) as f32 + 0.5) / (len - top) as f32 * span - 0.5).round().clamp(0.0, span - 1.0) as u32
        };
        let grad = layout::NAME_BAR_ALPHA * (1.0 - (d as f32 + 0.5) / len as f32);
        for row in 0..thick {
            // screen row 0 is the bar's top = unrotated x = thick - 1
            let sx = (thick - 1 - row).min(sw - 1);
            let p = src.get_pixel(sx, sy);
            let a = (p[3] as f32 * grad).round() as u8;
            out.put_pixel(d, row, image::Rgba([p[0], p[1], p[2], a]));
        }
    }
    out
}
