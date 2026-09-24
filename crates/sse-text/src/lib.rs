//! # sse-text -- text layout and rasterisation
//!
//! First version (decision Round 8: rasterise the game font directly; the TMP SDF path of
//! Q11 replaces it later). Everything runs on the CPU in pure Rust, so the output is
//! identical on every platform.
//!
//! ## What is reproduced (`docs/reverse/versions/cn-6.4.0/text.md`)
//! - Source Han Sans SC Medium (body) / Bold (name); em size = TMP `fontSize`
//! - Line advance `(ascent − descent)·s + (lineHeight − pointSize)·s + lineSpacing·size·0.01`
//!   with FaceInfo pointSize 35 / lineHeight 70 / ascent 30.8 / descent −4.2, `lineSpacing` −80
//! - Auto-size between min and max (1 pt steps) + truncate
//! - The outline layer: the glyph dilated and alpha-tested, filled with
//!   `(0.2667, 0.2667, 0.4, 0.6)`, drawn under the white face
//!
//! ## Approximations
//! - Coverage rasterisation instead of SDF (edges differ slightly from TMP)
//! - `_FaceDilate 0.5` approximated as a fixed dilation radius
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
}

impl Style {
    pub fn line_advance(&self, size: f32) -> f32 {
        let s = size / POINT_SIZE;
        (ASCENT - DESCENT) * s + (LINE_HEIGHT - (ASCENT - DESCENT)) * s + self.line_spacing * size * 0.01
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

fn layout(font: &Font, chars: &[(char, u32)], size: f32, width: f32) -> Vec<Placed> {
    let sf = font.font.as_scaled(font.em(size));
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
                && last.line == line && NO_LINE_END.contains(last.c) {
                    carry = carry.max(1);
                }
            line += 1;
            x = 0.0;
            let start = out.len() - carry.min(out.len());
            for p in &mut out[start..] {
                // carried characters keep their index
                p.line = line;
                p.x = x;
                x += sf.h_advance(font.font.glyph_id(p.c));
            }
        }
        out.push(Placed { c, idx: i, x, line, units_end: u });
        x += adv;
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
    draw_faded(canvas, font, raw, visible_units, frame, style, alpha, &|_| 1.0);
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
        let placed = layout(font, &chars, size, frame.width);
        let lines = placed.last().map_or(1, |p| p.line + 1) as f32;
        let height = (ASCENT - DESCENT) * size / POINT_SIZE + (lines - 1.0) * style.line_advance(size);
        if !style.auto_size || height <= frame.height || size <= style.min_size {
            break placed;
        }
        size -= 1.0;
    };
    let px_size = size * frame.scale;
    let advance = style.line_advance(size) * frame.scale;
    let ascent = ASCENT * size / POINT_SIZE * frame.scale;
    let lines = placed.last().map_or(1, |p| p.line + 1);
    let block_h = (ASCENT - DESCENT) * size / POINT_SIZE * frame.scale + (lines - 1) as f32 * advance;
    let top = frame.y + (frame.height * frame.scale - block_h) * frame.valign;
    let sf = font.font.as_scaled(font.em(px_size));
    let mut line_w = vec![0.0_f32; lines];
    for p in &placed {
        let w = (p.x + sf.h_advance(font.font.glyph_id(p.c)) / frame.scale) * frame.scale;
        line_w[p.line] = line_w[p.line].max(w);
    }
    let visible: Vec<&Placed> = placed.iter().filter(|p| p.units_end <= visible_units).collect();
    let dilate = 1.7 * size / 40.0 * frame.scale;
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
            let Some(outline) = font.font.outline_glyph(glyph) else { continue };
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
                        put(canvas, b.min.x as i64 + x as i64, b.min.y as i64 + y as i64, color, c);
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
    canvas.blend(x as usize, y as usize, [color[0] * a, color[1] * a, color[2] * a, a]);
}

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
    let ri = r.ceil() as i64;
    let (w, h) = (bw as i64 + 2 * ri, bh as i64 + 2 * ri);
    for y in 0..h {
        for x in 0..w {
            let mut m = 0.0_f32;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if ((dx * dx + dy * dy) as f32) > r * r {
                        continue;
                    }
                    let (sx, sy) = (x - ri + dx, y - ri + dy);
                    if sx >= 0 && sy >= 0 && sx < bw as i64 && sy < bh as i64 {
                        m = m.max(cov[sy as usize * bw + sx as usize]);
                    }
                }
            }
            // alpha test (`col.a < 0.1 → discard`), then a solid fill
            if m >= 0.1 {
                put(canvas, ox as i64 - ri + x, oy as i64 - ri + y, color, color[3] * alpha);
            }
        }
    }
}
