//! # sse-text -- text layout and rasterisation
//!
//! Rasterises the client's source font directly (ADR-0011). Everything runs on the CPU in pure Rust, so the output is
//! identical on every platform.
//!
//! ## What is reproduced
//! - The client's FOT-RodinNTLG Pro DB (body) / EB (name) source fonts (Source Han Sans SC on
//!   CN); em size = TMP `fontSize`
//! - Line advance `(ascent − descent)·s + (lineHeight − pointSize)·s + lineSpacing·size·0.01`
//!   with FaceInfo pointSize 35 / lineHeight 70 / ascent 30.8 / descent −4.2, `lineSpacing` −80
//! - Auto-size between min and max (1 pt steps) + truncate
//! - The outline layer (`WordsOutline` / `NameOutline`, material `SDF_Base_Scenario_Outline`,
//!   shader `Sekai/TextMeshPro/Mobile/Distance Field` with `OUTLINE_ON`): the glyph grown by
//!   `(_FaceDilate + _OutlineWidth) · ratioA / 2` SDF units, alpha-tested and filled with
//!   `(0.2667, 0.2667, 0.4, 0.6)`, drawn under the white face (see [`outline_reach`])
//!
//! ## Approximations
//! - Coverage rasterisation instead of SDF (edges differ slightly from TMP); the outline is the
//!   Euclidean dilation of the coverage mask rather than an SDF contour
//! - Rich-text tags are stripped, not interpreted
//! - Line breaking: CJK breaks anywhere, Latin at spaces, simple kinsoku rules
//!
//! ## Allowed dependencies
//! `sse-core`.

use ab_glyph::{Font as _, FontVec, PxScale, ScaleFont as _};

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("{0}: {1}")]
    Font(String, String),
}

pub struct Font {
    font: FontVec,
}

impl Font {
    pub fn load(path: &std::path::Path) -> Result<Self, TextError> {
        let bytes = std::fs::read(path)
            .map_err(|e| TextError::Font(path.display().to_string(), e.to_string()))?;
        let font = FontVec::try_from_vec(bytes)
            .map_err(|e| TextError::Font(path.display().to_string(), e.to_string()))?;
        Ok(Self { font })
    }

    /// TMP's preferred width of a single line (`CalculatePreferredValues`): the advances at
    /// em size `size`, each character but the last followed by `spacing × size × 0.01`.
    pub fn preferred_width(&self, text: &str, size: f32, spacing: f32) -> f32 {
        let sf = self.font.as_scaled(self.em(size));
        let n = text.chars().count();
        let advances: f32 = text
            .chars()
            .map(|c| sf.h_advance(self.font.glyph_id(c)))
            .sum();
        advances + spacing * size * 0.01 * n.saturating_sub(1) as f32
    }

    /// `ab_glyph`'s `PxScale` is the pixel height of ascent − descent, while TMP's font size
    /// is the em. Converts an em size in pixels to the matching `PxScale`.
    fn em(&self, em_px: f32) -> PxScale {
        let upem = self.font.units_per_em().unwrap_or(1000.0);
        PxScale::from(em_px * self.font.height_unscaled() / upem)
    }
}

/// TMP FaceInfo of the game's font assets.
const POINT_SIZE: f32 = 35.0;
const LINE_HEIGHT: f32 = 70.0;
const ASCENT: f32 = 30.8;
const DESCENT: f32 = -4.2;
/// The font assets' `_GradientScale` (atlas padding 5 + 1). The atlas stores 0.5 at the glyph
/// edge and changes by one SDF unit over `2 × GRADIENT_SCALE` atlas pixels.
const GRADIENT_SCALE: f32 = 6.0;

/// `SDF_Base_Scenario_Outline`: `_FaceDilate`, `_OutlineWidth`, `_OutlineSoftness`, `_WeightBold`.
const OUTLINE_FACE_DILATE: f32 = 0.5;
const OUTLINE_WIDTH: f32 = 0.5;
const OUTLINE_SOFTNESS: f32 = 0.0;
const WEIGHT_BOLD: f32 = 0.75;

/// How far the outline layer reaches beyond the glyph edge, in atlas pixels (4.21).
///
/// TMP `UpdateShaderRatios`: `ratioA = (GS − 1) / (GS · max(1, weightBold/4 + faceDilate +
/// outlineWidth + softness))`. The shader's vertex stage puts `bias = (0.5 − faceDilate · ratioA
/// / 2) · scale − 0.5` and `outline = outlineWidth · ratioA · scale`; its fragment covers
/// `saturate(d − (bias − outline / 2))` and discards below 0.1, so the filled shape ends
/// `(faceDilate + outlineWidth) · ratioA / 2` SDF units outside the glyph edge.
pub fn outline_reach() -> f32 {
    let t = (WEIGHT_BOLD / 4.0 + OUTLINE_FACE_DILATE + OUTLINE_WIDTH + OUTLINE_SOFTNESS).max(1.0);
    let ratio_a = (GRADIENT_SCALE - 1.0) / (GRADIENT_SCALE * t);
    (OUTLINE_FACE_DILATE + OUTLINE_WIDTH) * ratio_a / 2.0 * (2.0 * GRADIENT_SCALE)
}

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// TMP `fontSize` (UI units per em).
    pub size: f32,
    pub min_size: f32,
    pub auto_size: bool,
    /// TMP `lineSpacing`.
    pub line_spacing: f32,
    /// Straight-alpha face colour.
    pub color: [f32; 4],
    pub outline: Option<[f32; 4]>,
    /// TMP `UNDERLAY_ON`: colour and downward offset in em (softness 0, dilate 0).
    pub underlay: Option<([f32; 4], f32)>,
    /// TMP `characterSpacing` (em / 100): every advance gains `spacing × size × 0.01`
    /// (`currentEmScale`, orthographic UI); line widths drop the last glyph's share.
    pub char_spacing: f32,
}

impl Style {
    pub fn line_advance(&self, size: f32) -> f32 {
        let s = size / POINT_SIZE;
        (ASCENT - DESCENT) * s
            + (LINE_HEIGHT - (ASCENT - DESCENT)) * s
            + self.line_spacing * size * 0.01
    }
}

/// Premultiplied RGBA f32 canvas.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub px: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            px: vec![[0.0; 4]; width * height],
        }
    }

    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        let d = &mut self.px[y * self.width + x];
        let k = 1.0 - src[3];
        for i in 0..4 {
            d[i] = src[i] + d[i] * k;
        }
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        self.px
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))
            .collect()
    }
}

/// Removes rich-text tags; returns the visible text and, for every visible char, the
/// UTF-16 offset just past it in the original string.
fn strip_tags(raw: &str) -> Vec<(char, u32)> {
    let mut out = Vec::new();
    let mut units = 0u32;
    let mut in_tag = false;
    for c in raw.chars() {
        units += c.len_utf16() as u32;
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            _ => out.push((c, units)),
        }
    }
    out
}

const NO_LINE_START: &str = "。，、．！？）」』】〉》…‥：；ー～!?,.)]}";
const NO_LINE_END: &str = "（「『【〈《([{";

struct Placed {
    c: char,
    /// Index in the tag-stripped character list, line feeds included (TMP `characterInfo`).
    idx: usize,
    x: f32,
    line: usize,
    units_end: u32,
}

fn layout(font: &Font, chars: &[(char, u32)], size: f32, width: f32, spacing: f32) -> Vec<Placed> {
    let sf = font.font.as_scaled(font.em(size));
    let gap = spacing * size * 0.01;
    let mut out: Vec<Placed> = Vec::new();
    let mut line = 0;
    let mut x = 0.0_f32;
    let mut i = 0;
    while i < chars.len() {
        let (c, u) = chars[i];
        if c == '\n' {
            line += 1;
            x = 0.0;
            i += 1;
            continue;
        }
        let adv = sf.h_advance(font.font.glyph_id(c));
        if x + adv > width && x > 0.0 {
            // break before this char; pull one char down if this one may not start a line
            let mut carry = 0;
            if NO_LINE_START.contains(c) {
                carry = 1;
            } else if c.is_ascii_alphanumeric() {
                // keep a Latin word together
                let mut k = out.len();
                while k > 0 && out[k - 1].line == line && out[k - 1].c.is_ascii_alphanumeric() {
                    k -= 1;
                }
                if k > 0 && out[k - 1].line == line {
                    carry = out.len() - k;
                }
            }
            if let Some(last) = out.last()
                && last.line == line
                && NO_LINE_END.contains(last.c)
            {
                carry = carry.max(1);
            }
            line += 1;
            x = 0.0;
            let start = out.len() - carry.min(out.len());
            for p in &mut out[start..] {
                // carried characters keep their index
                p.line = line;
                p.x = x;
                x += sf.h_advance(font.font.glyph_id(p.c)) + gap;
            }
        }
        out.push(Placed {
            c,
            idx: i,
            x,
            line,
            units_end: u,
        });
        x += adv + gap;
        i += 1;
    }
    out
}

/// A text box in canvas pixels.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// UI units → canvas pixels.
    pub scale: f32,
    /// Horizontal alignment: 0 = left, 0.5 = centre.
    pub align: f32,
    /// Vertical alignment: 0 = top, 0.5 = middle.
    pub valign: f32,
}

/// Draws `raw` with the first `visible_units` UTF-16 units visible (`maxVisible` semantics
/// of `Substring(0, n)`). `alpha` multiplies everything.
pub fn draw(
    canvas: &mut Canvas,
    font: &Font,
    raw: &str,
    visible_units: u32,
    frame: Frame,
    style: &Style,
    alpha: f32,
) {
    draw_faded(
        canvas,
        font,
        raw,
        visible_units,
        frame,
        style,
        alpha,
        &|_| 1.0,
    );
}

/// [`draw`] with a per-character alpha, indexed like TMP's `characterInfo` (rich-text tags
/// removed, line feeds counted).
#[allow(clippy::too_many_arguments)]
pub fn draw_faded(
    canvas: &mut Canvas,
    font: &Font,
    raw: &str,
    visible_units: u32,
    frame: Frame,
    style: &Style,
    alpha: f32,
    char_alpha: &dyn Fn(usize) -> f32,
) {
    let chars = strip_tags(raw);
    // auto-size: shrink until the lines fit the box height
    let mut size = style.size;
    let placed = loop {
        let placed = layout(font, &chars, size, frame.width, style.char_spacing);
        let lines = placed.last().map_or(1, |p| p.line + 1) as f32;
        let height =
            (ASCENT - DESCENT) * size / POINT_SIZE + (lines - 1.0) * style.line_advance(size);
        if !style.auto_size || height <= frame.height || size <= style.min_size {
            break placed;
        }
        size -= 1.0;
    };
    let px_size = size * frame.scale;
    let advance = style.line_advance(size) * frame.scale;
    let ascent = ASCENT * size / POINT_SIZE * frame.scale;
    let lines = placed.last().map_or(1, |p| p.line + 1);
    let block_h =
        (ASCENT - DESCENT) * size / POINT_SIZE * frame.scale + (lines - 1) as f32 * advance;
    let top = frame.y + (frame.height * frame.scale - block_h) * frame.valign;
    let sf = font.font.as_scaled(font.em(px_size));
    let mut line_w = vec![0.0_f32; lines];
    for p in &placed {
        let w = (p.x + sf.h_advance(font.font.glyph_id(p.c)) / frame.scale) * frame.scale;
        line_w[p.line] = line_w[p.line].max(w);
    }
    let visible: Vec<&Placed> = placed
        .iter()
        .filter(|p| p.units_end <= visible_units)
        .collect();
    let dilate = outline_reach() * size / POINT_SIZE * frame.scale;
    // pass 0: underlay, 1: outline, 2: face
    for pass in 0..3 {
        let (color, dy) = match pass {
            0 => match style.underlay {
                Some((c, em)) => (c, em * px_size),
                None => continue,
            },
            1 => match style.outline {
                Some(c) => (c, 0.0),
                None => continue,
            },
            _ => (style.color, 0.0),
        };
        for p in &visible {
            let alpha = alpha * char_alpha(p.idx);
            if alpha <= 0.0 {
                continue;
            }
            let left = frame.x + (frame.width * frame.scale - line_w[p.line]) * frame.align;
            let gx = left + p.x * frame.scale;
            let gy = top + ascent + p.line as f32 * advance + dy;
            let glyph = font
                .font
                .glyph_id(p.c)
                .with_scale_and_position(font.em(px_size), ab_glyph::point(gx, gy));
            let Some(outline) = font.font.outline_glyph(glyph) else {
                continue;
            };
            let b = outline.px_bounds();
            let (bw, bh) = (b.width() as usize, b.height() as usize);
            let mut cov = vec![0.0_f32; bw * bh];
            outline.draw(|x, y, c| {
                let (x, y) = (x as usize, y as usize);
                if x < bw && y < bh {
                    cov[y * bw + x] = c;
                }
            });
            if pass == 1 {
                draw_outline(canvas, &cov, bw, bh, b.min.x, b.min.y, dilate, color, alpha);
            } else {
                for y in 0..bh {
                    for x in 0..bw {
                        let c = cov[y * bw + x] * color[3] * alpha;
                        put(
                            canvas,
                            b.min.x as i64 + x as i64,
                            b.min.y as i64 + y as i64,
                            color,
                            c,
                        );
                    }
                }
            }
        }
    }
}

fn put(canvas: &mut Canvas, x: i64, y: i64, color: [f32; 4], a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= canvas.width as i64 || y >= canvas.height as i64 {
        return;
    }
    canvas.blend(
        x as usize,
        y as usize,
        [color[0] * a, color[1] * a, color[2] * a, a],
    );
}

/// Fills every pixel within `r` (Euclidean) of a pixel whose coverage passes the shader's
/// alpha test (`>= 0.1`), with `color` at `color.a × alpha`.
#[allow(clippy::too_many_arguments)]
fn draw_outline(
    canvas: &mut Canvas,
    cov: &[f32],
    bw: usize,
    bh: usize,
    ox: f32,
    oy: f32,
    r: f32,
    color: [f32; 4],
    alpha: f32,
) {
    let ri = r.ceil() as usize;
    let (w, h) = (bw + 2 * ri, bh + 2 * ri);
    let seed = |x: usize, y: usize| {
        x >= ri && y >= ri && x - ri < bw && y - ri < bh && cov[(y - ri) * bw + (x - ri)] >= 0.1
    };
    let dist = squared_distance(w, h, seed);
    let r2 = r * r;
    for y in 0..h {
        for x in 0..w {
            if dist[y * w + x] <= r2 {
                put(
                    canvas,
                    ox as i64 - ri as i64 + x as i64,
                    oy as i64 - ri as i64 + y as i64,
                    color,
                    color[3] * alpha,
                );
            }
        }
    }
}

/// Exact squared Euclidean distance from every pixel to the nearest seed pixel
/// (Felzenszwalb & Huttenlocher: a 1D lower-envelope pass over columns, then rows).
fn squared_distance(w: usize, h: usize, seed: impl Fn(usize, usize) -> bool) -> Vec<f32> {
    const INF: f32 = 1e20;
    let mut d: Vec<f32> = (0..w * h)
        .map(|i| if seed(i % w, i / w) { 0.0 } else { INF })
        .collect();
    let n = w.max(h);
    let (mut f, mut out) = (vec![0.0_f32; n], vec![0.0_f32; n]);
    let (mut v, mut z) = (vec![0_usize; n], vec![0.0_f32; n + 1]);
    for x in 0..w {
        for y in 0..h {
            f[y] = d[y * w + x];
        }
        edt_1d(&f[..h], &mut out[..h], &mut v, &mut z);
        for y in 0..h {
            d[y * w + x] = out[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&d[y * w..(y + 1) * w]);
        edt_1d(&f[..w], &mut out[..w], &mut v, &mut z);
        d[y * w..(y + 1) * w].copy_from_slice(&out[..w]);
    }
    d
}

fn edt_1d(f: &[f32], out: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let parabola = |q: usize| f[q] + (q * q) as f32;
    let mut k = 0;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..f.len() {
        let mut s;
        loop {
            let p = v[k];
            s = (parabola(q) - parabola(p)) / (2.0 * (q - p) as f32);
            if s > z[k] {
                break;
            }
            // z[0] is −∞, so this never underflows
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let dq = q as f32 - v[k] as f32;
        *o = dq * dq + f[v[k]];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squared_distance_matches_brute_force() {
        let (w, h) = (23, 17);
        let seeds = [(3, 4), (15, 2), (20, 14), (9, 9), (10, 9)];
        let d = squared_distance(w, h, |x, y| seeds.contains(&(x, y)));
        for y in 0..h {
            for x in 0..w {
                let best = seeds
                    .iter()
                    .map(|&(sx, sy)| {
                        let (dx, dy) = (x as f32 - sx as f32, y as f32 - sy as f32);
                        dx * dx + dy * dy
                    })
                    .fold(f32::INFINITY, f32::min);
                assert_eq!(d[y * w + x], best, "({x}, {y})");
            }
        }
    }

    #[test]
    fn outline_reach_follows_the_material() {
        // ratioA = 5 / (6 · 1.1875); (0.5 + 0.5) · ratioA / 2 SDF units × 12 px
        assert!(
            (outline_reach() - 4.2105).abs() < 1e-3,
            "{}",
            outline_reach()
        );
    }
}
