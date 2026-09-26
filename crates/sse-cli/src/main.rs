//! # sse-cli -- command-line entry point
//!
//! ## Responsibilities
//! - Subcommand orchestration: inspect / timeline / export
//! - The shell around the export configuration (ADR-0004: output form is a struct;
//!   the CLI is only a thin layer over it)

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sse_assets::Library;
use sse_core::TimeBase;

#[derive(Parser)]
#[command(name = "sse", version, about = "SekaiStoryExporter")]
struct Cli {
    /// SekaiStoryRipper output directory (holding `library/` and `episodes/`), or
    /// s3://bucket/prefix to read it from S3 (credentials from AWS_ACCESS_KEY_ID /
    /// AWS_SECRET_ACCESS_KEY, endpoint from AWS_ENDPOINT_URL).
    #[arg(long, global = true, env = "SSE_LIBRARY", default_value = ".")]
    library: String,
    /// Where an S3 library is mirrored and S3 outputs are staged (default: the platform cache
    /// directory, e.g. ~/.cache/sse).
    #[arg(long, global = true, env = "SSE_CACHE_DIR")]
    cache_dir: Option<PathBuf>,
    /// Replacement for {{playerName}} (ADR-0008).
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
        /// Encode at this size, e.g. 1920x1080, after rendering at --width/--height (e.g. render
        /// 3840x2160 to match a 4K device capture).
        #[arg(long, value_parser = parse_size)]
        output_size: Option<(u32, u32)>,
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
    /// The UI kit exported from your own client by `tools/ui-kit/extract.py` (see README).
    #[arg(long, env = "SSE_UI_DIR")]
    ui: PathBuf,
    /// Which client's fonts to use: cn or jp. Default: the `region` the library was ripped from
    /// (`ripper.lock.json`), else cn.
    #[arg(long, value_parser = ["cn", "jp"])]
    game: Option<String>,
}

impl OutputArgs {
    /// Both clients' talk windows use `FOT-RodinNTLGPro-DB SDF_Base` for the words and `-EB` for
    /// the name, with the same FaceInfo and layout. Only the source font behind them differs: CN
    /// ships Source Han Sans SC Medium/Bold under the Rodin names, JP the real FOT-RodinNTLG Pro
    /// DB/EB. `tools/ui-kit/extract.py` exports whichever the client has; a CN kit without them
    /// falls back to Source Han Sans SC files.
    fn ui_assets(&self, lib: &Library) -> sse_render::UiAssets {
        let opt = |name: &str| {
            let p = self.ui.join(name);
            p.is_file().then_some(p)
        };
        let game = self
            .game
            .clone()
            .or_else(|| lib.info().map(|i| i.region.clone()))
            .unwrap_or_else(|| "cn".into());
        let rodin = ("FOT-RodinNTLGPro-DB.otf", "FOT-RodinNTLGPro-EB.otf");
        let (body, name) = match game.as_str() {
            "jp" => rodin,
            _ if opt(rodin.0).is_some() => rodin,
            _ => ("SourceHanSansSC-Medium.otf", "SourceHanSansSC-Bold.otf"),
        };
        sse_render::UiAssets {
            dialog: opt("Dialogue_Background.png"),
            font_body: self.ui.join(body),
            font_name: self.ui.join(name),
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

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text.split_once('x').ok_or("expected WIDTHxHEIGHT")?;
    Ok((
        w.parse().map_err(|_| "bad width")?,
        h.parse().map_err(|_| "bad height")?,
    ))
}

/// An output path that may be `s3://bucket/key`: written to `local`, then uploaded by
/// [`Output::publish`] (ADR-0015).
struct Output {
    local: PathBuf,
    remote: Option<(sse_assets::remote::S3, String)>,
}

impl Output {
    fn new(path: &std::path::Path, cache: &std::path::Path) -> Result<Self> {
        let text = path.to_string_lossy();
        let Some(location) = sse_assets::remote::S3Location::parse(&text) else {
            return Ok(Self {
                local: path.to_owned(),
                remote: None,
            });
        };
        let location = location?;
        let key = location.prefix.clone();
        anyhow::ensure!(!key.is_empty(), "{text}: missing object key");
        let mut local = cache.join("outputs").join(&location.bucket);
        local.extend(key.split('/'));
        std::fs::create_dir_all(local.parent().expect("has parent"))?;
        let bucket = sse_assets::remote::S3Location {
            bucket: location.bucket,
            prefix: String::new(),
        };
        Ok(Self {
            local,
            remote: Some((sse_assets::remote::S3::new(bucket)?, key)),
        })
    }

    /// Uploads the output (and `sibling`, e.g. the report, next to it) when it is remote;
    /// returns where it ended up.
    fn publish(&self, sibling: Option<&std::path::Path>) -> Result<String> {
        let Some((s3, key)) = &self.remote else {
            return Ok(self.local.display().to_string());
        };
        s3.put_file(
            key,
            &self.local,
            sse_assets::remote::content_type(&self.local),
        )?;
        if let Some(sibling) = sibling {
            let name = sibling.file_name().expect("file").to_string_lossy();
            let sibling_key = match key.rsplit_once('/') {
                Some((dir, _)) => format!("{dir}/{name}"),
                None => name.into_owned(),
            };
            s3.put_file(
                &sibling_key,
                sibling,
                sse_assets::remote::content_type(sibling),
            )?;
        }
        Ok(format!("s3://{}/{key}", s3.location().bucket))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cache = cli
        .cache_dir
        .clone()
        .unwrap_or_else(sse_assets::default_cache_dir);
    let lib = Library::open_location(&cli.library, &cache)?;
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
        Command::Render {
            selector,
            frame,
            output,
            out,
        } => {
            let table = bake(&lib, selector, &opts, out.config())?;
            let mut r = sse_render::Renderer::new(&lib, &table, out.config(), out.ui_assets(&lib))?;
            let f = table.frames.get(*frame as usize).with_context(|| {
                format!("frame {frame} out of range (0..{})", table.frames.len())
            })?;
            let rgba = r.render(f)?;
            let target = Output::new(output, &cache)?;
            sse_export::write_png(&target.local, out.width, out.height, rgba)?;
            println!("wrote {}", target.publish(None)?);
        }
        Command::Export {
            selector,
            output,
            from,
            to,
            crf,
            ffmpeg,
            output_size,
            out,
        } => {
            let table = bake(&lib, selector, &opts, out.config())?;
            let target = Output::new(output, &cache)?;
            let mut r = sse_render::Renderer::new(&lib, &table, out.config(), out.ui_assets(&lib))?;
            r.set_ffmpeg(ffmpeg.clone());
            let n = table.frames.len() as u32;
            let range = from.unwrap_or(0).min(n)..to.unwrap_or(n).min(n);
            let vopts = sse_export::VideoOptions {
                output: target.local.clone(),
                width: out.width,
                height: out.height,
                ffmpeg: ffmpeg.clone(),
                crf: *crf,
                output_size: *output_size,
                range,
            };
            let step = table.fps * 10;
            sse_export::export_video(&lib, &table, &mut r, &vopts, |done, total| {
                if done % step == 0 || done == total {
                    eprintln!("  {done}/{total} frames");
                }
            })?;
            let report = target.local.with_extension("report.txt");
            let mut notes = table.notes.clone();
            notes.extend(sse_render::Renderer::notes());
            notes.extend(r.asset_notes());
            std::fs::write(&report, notes.join("\n") + "\n")?;
            let written = target.publish(Some(&report))?;
            println!(
                "wrote {written} (notes next to it: {})",
                report.file_name().expect("file").to_string_lossy()
            );
        }
        Command::Bake { selector, frame } => {
            let table = bake(
                &lib,
                selector,
                &opts,
                sse_render::RenderConfig {
                    width: 1920,
                    height: 1080,
                },
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

fn load(
    lib: &Library,
    selector: &str,
    opts: &sse_scenario::ParseOptions,
) -> Result<sse_ir::Episode> {
    let path = lib
        .episode_path(selector)
        .with_context(|| format!("bad selector {selector}"))?;
    lib.fetch_episode_index(selector)?;
    let index = lib.load_episode(&path)?;
    lib.sync(&index)?;
    lib.verify_episode(&index)?;
    Ok(sse_scenario::parse_episode(lib, &index, opts)?)
}
