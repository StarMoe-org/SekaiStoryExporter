//! # sse-export -- encoding and muxing
//!
//! ## Responsibilities
//! - Offline audio mix of the parameter table's cues (48 kHz stereo, linear resampling,
//!   linear fades, BGM looping)
//! - Video: rendered RGBA frames piped into `ffmpeg` (H.264, yuv420p), muxed with the mix
//! - Single-frame PNG output for inspection

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sse_assets::Library;
use sse_params::{AudioKind, ParamTable, VolumeTween};
use sse_render::Renderer;

pub const MIX_RATE: u32 = 48_000;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Asset(#[from] sse_assets::AssetError),
    #[error(transparent)]
    Render(#[from] sse_render::RenderError),
    #[error("{0}")]
    Io(String),
    #[error("ffmpeg: {0}")]
    Ffmpeg(String),
}

fn io(e: impl std::fmt::Display) -> ExportError {
    ExportError::Io(e.to_string())
}

/// Mixes every cue into interleaved stereo f32 at [`MIX_RATE`].
pub fn mix(lib: &Library, table: &ParamTable, frames: u32) -> Result<Vec<f32>, ExportError> {
    let fps = f64::from(table.fps);
    let total = (f64::from(frames) / fps * f64::from(MIX_RATE)).ceil() as usize;
    let mut out = vec![0.0_f32; total * 2];
    let at = |frame: u32| (f64::from(frame) / fps * f64::from(MIX_RATE)) as usize;
    // interactive BGMs replay the same waveforms block after block
    let mut cache: std::collections::BTreeMap<String, std::sync::Arc<sse_assets::Pcm>> =
        std::collections::BTreeMap::new();
    for cue in &table.audio {
        let gain = cue.volume
            * match cue.kind {
                AudioKind::Bgm => 0.6,
                AudioKind::Se => 0.8,
                AudioKind::Voice | AudioKind::Movie => 1.0,
            };
        for file in &cue.files {
            let pcm = match cache.get(file) {
                Some(p) => p.clone(),
                None => {
                    let p = std::sync::Arc::new(lib.load_wav(file)?);
                    cache.insert(file.clone(), p.clone());
                    p
                }
            };
            let ch = usize::from(pcm.channels.max(1));
            let src_frames = pcm.frames();
            if src_frames == 0 {
                continue;
            }
            let ratio = f64::from(pcm.sample_rate) / f64::from(MIX_RATE);
            let start = at(cue.start_frame);
            let stop = cue.stop_frame.map_or(total, at).min(total);
            let src_len_out = (src_frames as f64 / ratio) as usize;
            let fade_in = at(cue.fade_in).max(1);
            let fade_out = at(cue.fade_out);
            let mut i = start;
            while i < stop {
                let local = i - start;
                if !cue.looping && local >= src_len_out {
                    break;
                }
                let pos = (local as f64 * ratio) % src_frames as f64;
                let i0 = pos as usize;
                let i1 = if i0 + 1 < src_frames {
                    i0 + 1
                } else if cue.looping {
                    0
                } else {
                    i0
                };
                let t = (pos - i0 as f64) as f32;
                let mut g = gain;
                if cue.kind == AudioKind::Bgm && !table.bgm_volume.is_empty() {
                    let frame = i as f64 * fps / f64::from(MIX_RATE);
                    g *= VolumeTween::at(&table.bgm_volume, frame);
                }
                if let Some(a) = &cue.aisac {
                    let frame = i as f64 * fps / f64::from(MIX_RATE);
                    g *= a.at(&table.bgm_vertical, frame);
                }
                if cue.fade_in > 0 && local < fade_in {
                    g *= local as f32 / fade_in as f32;
                }
                if cue.fade_out > 0 && stop - i <= fade_out {
                    g *= (stop - i) as f32 / fade_out as f32;
                }
                for c in 0..2 {
                    let sc = c.min(ch - 1);
                    let a = pcm.samples[i0 * ch + sc];
                    let b = pcm.samples[i1 * ch + sc];
                    out[i * 2 + c] += (a + (b - a) * t) * g;
                }
                i += 1;
            }
        }
    }
    for s in &mut out {
        *s = s.clamp(-1.0, 1.0);
    }
    Ok(out)
}

pub fn write_wav(path: &Path, samples: &[f32]) -> Result<(), ExportError> {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: MIX_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).map_err(io)?;
    for s in samples {
        w.write_sample((s * 32767.0).round() as i16).map_err(io)?;
    }
    w.finalize().map_err(io)
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: Vec<u8>) -> Result<(), ExportError> {
    let img =
        image::RgbaImage::from_raw(width, height, rgba).ok_or_else(|| io("bad frame size"))?;
    img.save(path).map_err(io)
}

#[derive(Debug, Clone)]
pub struct VideoOptions {
    pub output: PathBuf,
    pub width: u32,
    pub height: u32,
    pub ffmpeg: PathBuf,
    pub crf: u32,
    /// Encode at this size (ffmpeg Lanczos downscale from the render size). Rendering at the
    /// device's native resolution and scaling down reproduces a capture of a high-resolution
    /// device: resolution-dependent effects such as the camera blur keep their on-screen size.
    pub output_size: Option<(u32, u32)>,
    /// Render only frames in this range.
    pub range: std::ops::Range<u32>,
}

/// Renders `range` and encodes it with the mixed audio.
pub fn export_video(
    lib: &Library,
    table: &ParamTable,
    renderer: &mut Renderer,
    opts: &VideoOptions,
    mut progress: impl FnMut(u32, u32),
) -> Result<(), ExportError> {
    let frames = opts.range.end - opts.range.start;
    let full = mix(lib, table, opts.range.end)?;
    let skip =
        (f64::from(opts.range.start) / f64::from(table.fps) * f64::from(MIX_RATE)) as usize * 2;
    let wav = opts.output.with_extension("mix.wav");
    write_wav(&wav, &full[skip.min(full.len())..])?;

    let mut child = Command::new(&opts.ffmpeg)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
        ])
        .args(["-s", &format!("{}x{}", opts.width, opts.height)])
        .args(["-r", &table.fps.to_string(), "-i", "-"])
        .arg("-i")
        .arg(&wav)
        .args(match opts.output_size {
            Some((w, h)) => vec!["-vf".to_owned(), format!("scale={w}:{h}:flags=lanczos")],
            None => Vec::new(),
        })
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-crf",
            &opts.crf.to_string(),
        ])
        .args([
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
        ])
        .arg(&opts.output)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| ExportError::Ffmpeg(e.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ExportError::Ffmpeg("no stdin".into()))?;
    for (n, f) in opts.range.clone().enumerate() {
        let rgba = renderer.render(&table.frames[f as usize])?;
        stdin
            .write_all(&rgba)
            .map_err(|e| ExportError::Ffmpeg(e.to_string()))?;
        progress(n as u32 + 1, frames);
    }
    drop(stdin);
    let status = child
        .wait()
        .map_err(|e| ExportError::Ffmpeg(e.to_string()))?;
    let _ = std::fs::remove_file(&wav);
    if !status.success() {
        return Err(ExportError::Ffmpeg(format!("exited with {status}")));
    }
    Ok(())
}
