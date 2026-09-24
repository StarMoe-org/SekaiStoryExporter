//! # sse-cli -- command-line entry point
//!
//! ## Responsibilities
//! - Subcommand orchestration: inspect / timeline / export
//! - The shell around the export configuration (decision Q15: output form is a struct;
//!   the CLI is only a thin layer over it)

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sse_assets::Library;
use sse_core::TimeBase;

#[derive(Parser)]
#[command(name = "sse", version, about = "SekaiStoryExporter")]
struct Cli {
    /// SekaiStoryRipper output directory (holding `library/` and `episodes/`).
    #[arg(long, global = true, env = "SSE_LIBRARY", default_value = ".")]
    library: PathBuf,
    /// Replacement for {{playerName}} (decision Q39).
    #[arg(long, global = true, default_value = "「世界」的居民")]
    player_name: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the IR of an episode as JSON.
    Inspect { selector: String },
    /// Print the compiled timeline of an episode as JSON.
    Timeline {
        selector: String,
        /// Simulate at this (possibly fractional) frame rate instead of the story's 60 fps.
        /// For comparing the pacing model with a capture of a slowed-down game; frame
        /// numbers in the output are then simulation frames at this rate.
        #[arg(long)]
        sim_fps: Option<f32>,
    },
    /// Run Pass 1 and print a summary (or one frame's state with --frame).
    Bake {
        selector: String,
        #[arg(long)]
        frame: Option<usize>,
    },
    /// Render one frame to a PNG.
    Render {
        selector: String,
        #[arg(long)]
        frame: u32,
        #[arg(long, short)]
        output: PathBuf,
        #[command(flatten)]
        out: OutputArgs,
    },
    /// Render the episode to an MP4 (auto-play).
    Export {
        selector: String,
        #[arg(long, short)]
        output: PathBuf,
        /// Only export frames [from, to).
        #[arg(long)]
        from: Option<u32>,
        #[arg(long)]
        to: Option<u32>,
        #[arg(long, default_value_t = 18)]
        crf: u32,
        #[arg(long, default_value = "ffmpeg")]
        ffmpeg: PathBuf,
        #[command(flatten)]
        out: OutputArgs,
    },
}

#[derive(clap::Args)]
struct OutputArgs {
    #[arg(long, default_value_t = 1920)]
    width: u32,
    #[arg(long, default_value_t = 1080)]
    height: u32,
    /// Directory with the user-supplied UI overlays and fonts (see README).
    #[arg(long, env = "SSE_UI_DIR")]
    ui: PathBuf,
}

impl OutputArgs {
    fn ui_assets(&self) -> sse_render::UiAssets {
        let opt = |name: &str| {
            let p = self.ui.join(name);
            p.is_file().then_some(p)
        };
        sse_render::UiAssets {
            dialog: opt("Dialogue_Background.png"),
            telop: opt("SceneText_Background.png"),
            place_info: opt("SceneText_TopLeft.png"),
            font_body: self.ui.join("SourceHanSansSC-Medium.otf"),
            font_name: self.ui.join("SourceHanSansSC-Bold.otf"),
            sprites: self.ui.clone(),
        }
    }

    fn config(&self) -> sse_render::RenderConfig {
        sse_render::RenderConfig {
            width: self.width,
            height: self.height,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let lib = Library::open(&cli.library);
    let opts = sse_scenario::ParseOptions {
        player_name: cli.player_name.clone(),
    };
    match &cli.command {
        Command::Inspect { selector } => {
            let ep = load(&lib, selector, &opts)?;
            println!("{}", serde_json::to_string_pretty(&ep)?);
        }
        Command::Timeline { selector, sim_fps } => {
            let ep = load(&lib, selector, &opts)?;
            let tb = match sim_fps {
                Some(f) => TimeBase::with_delta(f.round() as u32, 1.0 / f),
                None => TimeBase::story(),
            };
            let tl = sse_timeline::compile(&lib, &ep, tb)?;
            println!("{}", serde_json::to_string_pretty(&tl)?);
        }
        Command::Render { selector, frame, output, out } => {
            let table = bake(&lib, selector, &opts, out.config())?;
            let mut r = sse_render::Renderer::new(&lib, &table, out.config(), out.ui_assets())?;
            let f = table
                .frames
                .get(*frame as usize)
                .with_context(|| format!("frame {frame} out of range (0..{})", table.frames.len()))?;
            let rgba = r.render(f)?;
            sse_export::write_png(output, out.width, out.height, rgba)?;
            println!("wrote {}", output.display());
        }
        Command::Export { selector, output, from, to, crf, ffmpeg, out } => {
            let table = bake(&lib, selector, &opts, out.config())?;
            let mut r = sse_render::Renderer::new(&lib, &table, out.config(), out.ui_assets())?;
            r.set_ffmpeg(ffmpeg.clone());
            let n = table.frames.len() as u32;
            let range = from.unwrap_or(0).min(n)..to.unwrap_or(n).min(n);
            let vopts = sse_export::VideoOptions {
                output: output.clone(),
                width: out.width,
                height: out.height,
                ffmpeg: ffmpeg.clone(),
                crf: *crf,
                range,
            };
            let step = table.fps * 10;
            sse_export::export_video(&lib, &table, &mut r, &vopts, |done, total| {
                if done % step == 0 || done == total {
                    eprintln!("  {done}/{total} frames");
                }
            })?;
            let report = output.with_extension("report.txt");
            let mut notes = table.notes.clone();
            notes.extend(sse_render::Renderer::notes());
            notes.extend(r.asset_notes());
            std::fs::write(&report, notes.join("\n") + "\n")?;
            println!("wrote {} (notes: {})", output.display(), report.display());
        }
        Command::Bake { selector, frame } => {
            let table = bake(
                &lib,
                selector,
                &opts,
                sse_render::RenderConfig { width: 1920, height: 1080 },
            )?;
            match frame {
                Some(f) => println!("{}", serde_json::to_string_pretty(&table.frames[*f])?),
                None => {
                    println!(
                        "frames {} ({:.1} s), models {:?}, audio cues {}",
                        table.frames.len(),
                        table.frames.len() as f64 / f64::from(table.fps),
                        table.models,
                        table.audio.len()
                    );
                    for n in &table.notes {
                        println!("note: {n}");
                    }
                }
            }
        }
    }
    Ok(())
}

fn bake(
    lib: &Library,
    selector: &str,
    opts: &sse_scenario::ParseOptions,
    cfg: sse_render::RenderConfig,
) -> Result<sse_params::ParamTable> {
    let ep = load(lib, selector, opts)?;
    let tl = sse_timeline::compile(lib, &ep, TimeBase::story())?;
    Ok(sse_bake::bake(
        lib,
        &ep,
        &tl,
        &sse_bake::BakeOptions {
            content_size: cfg.content_size(),
            seed: 0,
        },
    )?)
}

fn load(lib: &Library, selector: &str, opts: &sse_scenario::ParseOptions) -> Result<sse_ir::Episode> {
    let path = lib
        .episode_path(selector)
        .with_context(|| format!("bad selector {selector}"))?;
    let index = lib.load_episode(&path)?;
    lib.verify_episode(&index)?;
    Ok(sse_scenario::parse_episode(lib, &index, opts)?)
}
