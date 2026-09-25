//! # sse-assets -- asset sources and caching
//!
//! ## Responsibilities
//! - Read-only loading of SekaiStoryRipper output: `ripper-episode` indexes and
//!   `sse-motion` clips via `ripper-format` (ADR-0006)
//! - Reject unknown `format` / `version` values instead of guessing
//! - **Explicit failure with a list of missing assets**, pointing at `ripper rip <selector>`.
//!   Silent degradation is forbidden
//! - Recording the asset version from `ripper.lock.json`
//!
//! ## Not responsible for
//! - Downloading or decrypting anything (ADR-0006: sse has no CDN or key handling)
//! - Resolving motion/facial bundles; the episode index is trusted (ADR-0007)
//! - Interpreting asset contents (each consuming crate does that)
//!
//! ## Allowed dependencies
//! `sse-core` and the external `ripper-format` (serde types only).

use std::path::{Path, PathBuf};

pub use ripper_format::episode::{self, EpisodeIndex};
pub use ripper_format::motion::{self, SseMotion};

mod audio;
mod model;

pub use audio::Pcm;
pub use model::Model3;

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: invalid JSON: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{path}: unsupported format {format} v{version} (this build reads {expected})")]
    Format {
        path: PathBuf,
        format: String,
        version: u32,
        expected: &'static str,
    },
    #[error("{path}: image: {source}")]
    Image {
        path: PathBuf,
        source: image::ImageError,
    },
    #[error("{path}: wav: {source}")]
    Wav { path: PathBuf, source: hound::Error },
    #[error(
        "{} asset(s) listed in the episode index are missing from the library:\n{}\n\
         run `ripper rip {selector}` to (re)export them",
        missing.len(),
        missing.iter().map(|p| format!("  - {p}")).collect::<Vec<_>>().join("\n")
    )]
    Missing {
        selector: String,
        missing: Vec<String>,
    },
}

pub type Result<T> = std::result::Result<T, AssetError>;

/// A SekaiStoryRipper output directory (the one holding `library/` and `episodes/`).
#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of a `library/`-relative, `/`-separated path from an index.
    pub fn path(&self, library_path: &str) -> PathBuf {
        let mut p = self.root.join("library");
        for segment in library_path.split('/') {
            p.push(segment);
        }
        p
    }

    /// Path of the episode index for a selector like `unit:school-refusal-story-chapter/1`.
    pub fn episode_path(&self, selector: &str) -> Option<PathBuf> {
        let (kind, rest) = selector.split_once(':')?;
        let (key, no) = rest.rsplit_once('/')?;
        Some(
            self.root
                .join("episodes")
                .join(kind)
                .join(key)
                .join(format!("{no}.json")),
        )
    }

    pub fn load_episode(&self, index_path: &Path) -> Result<EpisodeIndex> {
        let index: EpisodeIndex = read_json(index_path)?;
        if index.format != episode::FORMAT || index.version != episode::VERSION {
            return Err(AssetError::Format {
                path: index_path.to_owned(),
                format: index.format,
                version: index.version,
                expected: "ripper-episode v1",
            });
        }
        Ok(index)
    }

    /// Checks that every file the index references exists; lists all that do not.
    pub fn verify_episode(&self, index: &EpisodeIndex) -> Result<()> {
        let mut paths: Vec<&str> = vec![index.scenario.path.as_str()];
        for c in index.characters.values() {
            for costume in &c.costumes {
                paths.extend(costume.motions.values().map(|m| m.path.as_str()));
            }
        }
        paths.extend(index.backgrounds.values().map(|b| b.path.as_str()));
        for audio in index
            .bgm
            .values()
            .chain(index.se.values())
            .chain(index.voices.values())
        {
            paths.extend(audio.files.iter().map(String::as_str));
        }
        let mut missing: Vec<String> = paths
            .into_iter()
            .filter(|p| !self.path(p).is_file())
            .map(str::to_owned)
            .collect();
        missing.sort();
        missing.dedup();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(AssetError::Missing {
                selector: index.story.selector.clone(),
                missing,
            })
        }
    }

    pub fn load_motion(&self, library_path: &str) -> Result<SseMotion> {
        let path = self.path(library_path);
        let m: SseMotion = read_json(&path)?;
        if m.format != motion::FORMAT || m.version != motion::VERSION {
            return Err(AssetError::Format {
                path,
                format: m.format,
                version: m.version,
                expected: "sse-motion v1",
            });
        }
        Ok(m)
    }

    pub fn read_json<T: serde::de::DeserializeOwned>(&self, library_path: &str) -> Result<T> {
        read_json(&self.path(library_path))
    }

    pub fn load_png(&self, library_path: &str) -> Result<image::RgbaImage> {
        load_png(&self.path(library_path))
    }

    pub fn load_wav(&self, library_path: &str) -> Result<Pcm> {
        audio::load_wav(&self.path(library_path))
    }

    /// The model bundle directory `library/<bundle>/`.
    pub fn load_model3(&self, model_bundle: &str) -> Result<Model3> {
        model::load(&self.path(model_bundle))
    }
}

pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read(path)?;
    serde_json::from_slice(&bytes).map_err(|source| AssetError::Json {
        path: path.to_owned(),
        source,
    })
}

pub fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| AssetError::Io {
        path: path.to_owned(),
        source,
    })
}

pub fn load_png(path: &Path) -> Result<image::RgbaImage> {
    let bytes = read(path)?;
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map(|i| i.to_rgba8())
        .map_err(|source| AssetError::Image {
            path: path.to_owned(),
            source,
        })
}
