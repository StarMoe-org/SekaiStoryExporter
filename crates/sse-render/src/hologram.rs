//! `Sekai/Live2D/Live2DHologram` on the character's `RawImage` (see `sse-bake`'s `hologram`).
//!
//! The shader reads `_SubTex` (`holo`, a scan-line texture) at `TEXCOORD1 × _SubTex_ST.xy`,
//! scrolled by `_Time.x × 0.1`. A UGUI `RawImage` writes `uv1 = 0` for every vertex
//! (`VertexHelper.AddVert(position, color, uv0)`), so the whole image samples the same texel
//! column `u = 0` at `v = t / 200`: one value per frame, sampled here on the CPU with the
//! texture's bilinear filter and repeat wrap. `holo.png` comes from the `--ui` kit.

use std::path::Path;

use sse_core::consts;

pub struct ScanTexture {
    /// Red channel at `u = 0` (the mean of the first and last columns, the bilinear
    /// footprint of `u = 0` under repeat wrap), by Unity row (bottom row first).
    rows: Vec<f32>,
}

impl ScanTexture {
    pub const FILE: &'static str = "holo.png";

    pub fn load(path: &Path) -> Option<Self> {
        let img = sse_assets::load_png(path).ok()?;
        let (w, h) = img.dimensions();
        if w == 0 || h == 0 {
            return None;
        }
        let rows = (0..h)
            .map(|y| {
                // PNG rows are top-down, Unity's v is bottom-up
                let row = h - 1 - y;
                let a = img.get_pixel(0, row)[0] as f32;
                let b = img.get_pixel(w - 1, row)[0] as f32;
                (a + b) * 0.5 / 255.0
            })
            .collect();
        Some(Self { rows })
    }

    /// `_SubTex.r` at shader time `time` (seconds).
    pub fn sample(&self, time: f32) -> f32 {
        let n = self.rows.len();
        let v = time * consts::HOLOGRAM_SCROLL_PER_SECOND;
        let ty = v * n as f32 - 0.5;
        let y0 = ty.floor();
        let t = ty - y0;
        let i0 = (y0 as i64).rem_euclid(n as i64) as usize;
        let i1 = (i0 + 1) % n;
        self.rows[i0] * (1.0 - t) + self.rows[i1] * t
    }
}
