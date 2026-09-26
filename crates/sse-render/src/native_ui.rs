//! The scenario UI rebuilt from the game's prefabs (`TalkWindow` `resources.assets|7281`,
//! `ScreenLayerScenario` `|6451`, `ScenarioFullScreenTextDialog` `|44706`).
//!
//! The sprites come from the game client (`data.unity3d`, not the CDN bundles), so the user
//! exports them with `tools/ui-kit/extract.py` into the `--ui` directory. A missing sprite drops
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
    /// `AutoSignalText` `characterSpacing`.
    pub const AUTO_TEXT_SPACING: f32 = -4.0;
    /// `AutoSignalText` font size.
    pub const AUTO_TEXT_SIZE: f32 = 32.0;
    /// `AutoSignal`'s `HorizontalLayoutGroup` (MiddleCenter, spacing 6, controls child size):
    /// `icon` (its own group holds only `AutoSignalIcon1` in auto mode — `ActiveAutoObject`
    /// disables icon 2 — so 28 wide) + 6 + `signalText` at the text's TMP preferred width
    /// (`Font::preferred_width`; the prefab's serialized 73.82 is a stale layout result),
    /// centred in the 200-wide signal. With CN 6.4.0's EB OnDemand advances (A 22.4375,
    /// U 26.1875, T 21.875, O 26.953125 at 35 pt) the width is 85.26; JP's real Rodin EB is
    /// wider, so the row is laid out from the loaded font.
    fn auto_row_x(text_w: f32) -> f32 {
        AUTO[0] + (AUTO[2] - (28.0 + 6.0 + text_w)) * 0.5
    }
    /// `AutoSignalIcon1` (28×22, rotated −90° about its centre): centre (row x + 14, 1032).
    pub fn auto_icon(text_w: f32) -> [f32; 4] {
        [auto_row_x(text_w) + 14.0 - 11.0, 1018.0, 22.0, 28.0]
    }
    /// `AutoSignalText`, sized to its preferred width.
    pub fn auto_text(text_w: f32) -> [f32; 4] {
        [auto_row_x(text_w) + 34.0, 1016.0, text_w, 32.0]
    }
    /// `TweenAlpha` on the icon: visible for the first 0.5028 s of each 1 s loop.
    pub const AUTO_ICON_ON: f32 = 0.502_762_44;
    /// `UIPartsMenuButton`: 96×96 centred at top-right − (64, 64).
    pub const MENU: [f32; 4] = [1808.0, 16.0, 96.0, 96.0];
    /// `MenuIcon`: 52×12, centred.
    pub const MENU_ICON: [f32; 4] = [1830.0, 58.0, 52.0, 12.0];
    pub const MENU_ICON_COLOR: [f32; 4] = [0.266_666_68, 0.266_666_68, 0.4, 1.0];
    /// `AnswerChoiceDialog` (CN `resources.assets|102548`): `WindowRoot` centre-anchored at
    /// (0, −40); `Answer0` at (460, 40) and `Answer1` at (−460, 40) inside it, 520×96.
    pub const ANSWER_WINDOW_Y: f32 = -40.0;
    pub const ANSWER_POS: [[f32; 2]; 2] = [[460.0, 40.0], [-460.0, 40.0]];
    pub const ANSWER_SIZE: [f32; 2] = [520.0, 96.0];
    /// `Answer*/Text`: stretched with sizeDelta (−48, 0); FOT-RodinNTLGPro-EB 32 (auto-size
    /// up to 32), centre / middle, `base_dbl` (0.267, 0.267, 0.4).
    pub const ANSWER_TEXT_INSET: f32 = 24.0;
    pub const ANSWER_TEXT_SIZE: f32 = 32.0;
    pub const ANSWER_TEXT_COLOR: [f32; 4] = [0.266_666_68, 0.266_666_68, 0.4, 1.0];
    /// `bg_base_wh` colour of the telop band and the place-info panel.
    pub const BAND_COLOR: [f32; 4] = [0.266_666_68, 0.266_666_68, 0.4, 0.8];
    /// `ScenarioTelop/.../Text`: 1040×120 centred, 44, centre / middle.
    pub const TELOP_TEXT: [f32; 4] = [440.0, 480.0, 1040.0, 120.0];
    /// `PlaceInfo/Content/CustomText`: 520×40 at the panel centre + (10, 0), 40, left / middle.
    pub const PLACE_TEXT: [f32; 4] = [40.0, 43.0, 520.0, 40.0];
    /// `ScenarioFullScreenTextDialog/Content/Text`: stretched, size (−280, −480), pos (140, 0).
    pub const FST_TEXT: [f32; 4] = [280.0, 240.0, 1640.0, 600.0];
}

/// `ac_scenario_telop_v2_01` / `_02` (`resources.assets|1974` / `|1975`), decoded from the
/// clips' streamed curves (`story/scripts/45_clipdecode.py`) and sampled every 1/60 s.
pub mod telop_clip {
    pub const SHOW_BASE_SCALE_X: [f32; 21] = [
        0.0, 0.12296, 0.25391, 0.37795, 0.48991, 0.58842, 0.67495, 0.74949, 0.8125, 0.86549,
        0.90873, 0.94274, 0.9685, 0.98631, 0.99662, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ];
    pub const SHOW_BASE_ALPHA: [f32; 21] = [
        0.0, 0.01372, 0.05516, 0.1241, 0.21668, 0.32518, 0.4419, 0.55721, 0.66336, 0.75705,
        0.83532, 0.89725, 0.94396, 0.97589, 0.99411, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
    ];
    pub const SHOW_TEXT_ALPHA: [f32; 21] = [
        0.0, 0.0, 0.0, 0.0, 0.01159, 0.05211, 0.1365, 0.29176, 0.54934, 0.80819, 0.90005, 0.92208,
        0.94093, 0.95669, 0.96952, 0.97981, 0.98771, 0.99342, 0.99723, 0.99935, 1.0,
    ];
    pub const SHOW_TEXT_X: [f32; 21] = [
        -30.0, -30.0, -30.0, -30.0, -29.69962, -28.85422, -27.52697, -25.79509, -23.74549,
        -21.42683, -18.92424, -16.32728, -13.678, -11.0655, -8.57801, -6.25891, -4.19729, -2.47611,
        -1.14798, -0.29804, 0.0,
    ];
    /// Base and text alpha share this curve.
    pub const HIDE_ALPHA: [f32; 21] = [
        1.0, 0.99797, 0.99153, 0.97992, 0.96224, 0.93757, 0.90421, 0.86046, 0.8045, 0.73316,
        0.6446, 0.53982, 0.42369, 0.30985, 0.21207, 0.1352, 0.07928, 0.04113, 0.01686, 0.00391,
        0.00001,
    ];

    pub fn sample(table: &[f32; 21], t: f32) -> f32 {
        let x = (t * 60.0).clamp(0.0, 20.0);
        let i = (x.floor() as usize).min(19);
        let w = x - i as f32;
        table[i] * (1.0 - w) + table[i + 1] * w
    }
}

pub struct NativeUi {
    window: Option<gpu::Image>,
    name_bar: Option<gpu::Image>,
    auto_bg: Option<gpu::Image>,
    auto_icon: Option<gpu::Image>,
    menu: Option<gpu::Image>,
    menu_icon: Option<gpu::Image>,
    /// `AnswerChoiceDialog/WindowRoot/Answer*/ButtonImage`: `btn_round_h80_wh` (106×96,
    /// border L51 R51) sliced to 520 wide; optional (older UI kits lack it).
    answer: Option<gpu::Image>,
    /// White 256×1 ramps for `GradientAlpha` edges: alpha 0 → 1 and 1 → 0.
    ramp: [gpu::Image; 2],
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
        let name_bar =
            load(dir, "bg_base_half_r8_wh", &mut missing).map(|i| gpu.image(&name_bar(&i)));
        let auto_bg = load(dir, "bg_base_round_h48_wh", &mut missing)
            .map(|i| gpu.image(&slice_h(&i, 25, 25, layout::AUTO[2] as u32)));
        let auto_icon = load(dir, "icon_triangle_h22_wh", &mut missing)
            .map(|i| gpu.image(&image::imageops::rotate90(&i)));
        let menu = load(dir, "btn_circle_h80_wh", &mut missing).map(|i| gpu.image(&i));
        let menu_icon = load(dir, "icon_menu_story_wh", &mut missing).map(|i| gpu.image(&i));
        let answer = sse_assets::load_png(&dir.join("btn_round_h80_wh.png"))
            .ok()
            .map(|i| gpu.image(&slice_h(&i, 51, 51, layout::ANSWER_SIZE[0] as u32)));
        let ramp = |up: bool| {
            RgbaImage::from_fn(256, 1, |x, _| {
                let a = if up { x } else { 255 - x } as u8;
                image::Rgba([255, 255, 255, a])
            })
        };
        let ramp = [gpu.image(&ramp(true)), gpu.image(&ramp(false))];
        Self {
            window,
            name_bar,
            auto_bg,
            auto_icon,
            menu,
            menu_icon,
            answer,
            ramp,
            missing,
        }
    }

    pub fn has_window(&self) -> bool {
        self.window.is_some()
    }

    /// Talk window background, name bar and auto signal. `auto_time` is the seconds since
    /// the auto signal was enabled (drives the icon blink).
    pub fn talk(
        &self,
        out: &mut Vec<QuadDraw>,
        k: f32,
        alpha: f32,
        auto_time: f32,
        auto_text_w: f32,
    ) {
        let r = |a: [f32; 4]| [a[0] * k, a[1] * k, a[2] * k, a[3] * k];
        let with = |c: [f32; 4], a: f32| [c[0], c[1], c[2], c[3] * a];
        if let Some(w) = &self.window {
            out.push(QuadDraw::image(
                w.id,
                r(layout::WINDOW),
                with(layout::WINDOW_COLOR, alpha),
            ));
        }
        if let Some(b) = &self.name_bar {
            out.push(QuadDraw::image(
                b.id,
                r(layout::NAME_BAR),
                [1.0, 1.0, 1.0, alpha],
            ));
        }
        if let Some(b) = &self.auto_bg {
            out.push(QuadDraw::image(
                b.id,
                r(layout::AUTO),
                with(layout::AUTO_BG_COLOR, alpha),
            ));
        }
        if let Some(i) = &self.auto_icon
            && auto_time.rem_euclid(1.0) < layout::AUTO_ICON_ON
        {
            out.push(QuadDraw::image(
                i.id,
                r(layout::auto_icon(auto_text_w)),
                [1.0, 1.0, 1.0, alpha],
            ));
        }
    }

    /// `AnswerChoiceDialog` buttons at window scale `scale`; their rects (target pixels) in
    /// answer order, for the labels.
    pub fn choice(
        &self,
        out: &mut Vec<QuadDraw>,
        k: f32,
        screen: [f32; 2],
        count: usize,
        scale: f32,
    ) -> Vec<[f32; 4]> {
        let [bw, bh] = layout::ANSWER_SIZE;
        let rects: Vec<[f32; 4]> = layout::ANSWER_POS
            .iter()
            .take(count)
            .map(|&[x, y]| {
                // WindowRoot (0, −40) scaled about its centre; canvas y up
                let (cx, cy) = (x * scale, layout::ANSWER_WINDOW_Y + y * scale);
                let (w, h) = (bw * scale * k, bh * scale * k);
                [
                    screen[0] * 0.5 + cx * k - w * 0.5,
                    screen[1] * 0.5 - cy * k - h * 0.5,
                    w,
                    h,
                ]
            })
            .collect();
        if let Some(img) = &self.answer {
            for r in &rects {
                out.push(QuadDraw::image(img.id, *r, [1.0; 4]));
            }
        }
        rects
    }

    pub fn menu(&self, out: &mut Vec<QuadDraw>, k: f32, alpha: f32) {
        let r = |a: [f32; 4]| [a[0] * k, a[1] * k, a[2] * k, a[3] * k];
        if let Some(m) = &self.menu {
            out.push(QuadDraw::image(
                m.id,
                r(layout::MENU),
                [1.0, 1.0, 1.0, alpha],
            ));
        }
        if let Some(i) = &self.menu_icon {
            let c = layout::MENU_ICON_COLOR;
            out.push(QuadDraw::image(
                i.id,
                r(layout::MENU_ICON),
                [c[0], c[1], c[2], c[3] * alpha],
            ));
        }
    }
}

/// Three-piece band of `bg_base_wh` (a plain white image) in `color`: a left ramp
/// (`GradientAlpha` left 0 → right 1), a solid middle and a right ramp.
fn band(
    ramps: &[gpu::Image; 2],
    out: &mut Vec<QuadDraw>,
    k: f32,
    x: [f32; 4],
    y: f32,
    h: f32,
    color: [f32; 4],
) {
    let r = |x0: f32, x1: f32| [x0 * k, y * k, (x1 - x0) * k, h * k];
    out.push(QuadDraw::image(ramps[0].id, r(x[0], x[1]), color));
    out.push(QuadDraw::solid(r(x[1], x[2]), color));
    out.push(QuadDraw::image(ramps[1].id, r(x[2], x[3]), color));
}

impl NativeUi {
    /// `ScenarioTelop` (`resources.assets|44707`): 1040×120 centred; `Content/Base` pieces
    /// 260 / 520 / 260 scaled in x about pivot x = 0.02. Returns the text's (x offset, alpha).
    pub fn telop(
        &self,
        out: &mut Vec<QuadDraw>,
        k: f32,
        show: f32,
        hide: Option<f32>,
    ) -> (f32, f32) {
        use telop_clip::*;
        let (scale, base_a, text_a, text_x) = match hide {
            Some(t) => (1.0, sample(&HIDE_ALPHA, t), sample(&HIDE_ALPHA, t), 0.0),
            None => (
                sample(&SHOW_BASE_SCALE_X, show),
                sample(&SHOW_BASE_ALPHA, show),
                sample(&SHOW_TEXT_ALPHA, show),
                sample(&SHOW_TEXT_X, show),
            ),
        };
        let (left, width) = (440.0, 1040.0);
        let pivot = left + 0.02 * width;
        let sx = |x: f32| pivot + (x - pivot) * scale;
        let c = layout::BAND_COLOR;
        if scale > 0.0 && base_a > 0.0 {
            band(
                &self.ramp,
                out,
                k,
                [sx(440.0), sx(700.0), sx(1220.0), sx(1480.0)],
                480.0,
                120.0,
                [c[0], c[1], c[2], c[3] * base_a],
            );
        }
        (text_x, text_a)
    }

    /// `PlaceInfo` (`ScreenLayerScenario/UILayer/TopLeft2`): 580×64 at (x, 31); `Bg (3)` ramp
    /// 152 left of it, `Bg (4)` ramp 152 right of it.
    pub fn place_info(&self, out: &mut Vec<QuadDraw>, k: f32, x: f32) {
        band(
            &self.ramp,
            out,
            k,
            [x - 152.0, x, x + 580.0, x + 732.0],
            31.0,
            64.0,
            layout::BAND_COLOR,
        );
    }
}

/// `PlayCinemascope`: `Base` (black, alpha 0.5·e) and the two black bars (height 240·e).
pub fn cinemascope(out: &mut Vec<QuadDraw>, k: f32, w: f32, h: f32, e: f32) {
    if e <= 0.0 {
        return;
    }
    out.push(QuadDraw::solid(
        [0.0, 0.0, w, h],
        [0.0, 0.0, 0.0, sse_core::consts::FST_BASE_ALPHA * e],
    ));
    let bar = sse_core::consts::FST_CINEMASCOPE_HEIGHT * e * k;
    out.push(QuadDraw::solid([0.0, 0.0, w, bar], [0.0, 0.0, 0.0, 1.0]));
    out.push(QuadDraw::solid(
        [0.0, h - bar, w, bar],
        [0.0, 0.0, 0.0, 1.0],
    ));
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
            left + (((x - left) as f32 + 0.5) / centre_dst * centre_src - 0.5)
                .round()
                .clamp(0.0, centre_src - 1.0) as u32
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
            top + (((d - top) as f32 + 0.5) / (len - top) as f32 * span - 0.5)
                .round()
                .clamp(0.0, span - 1.0) as u32
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
