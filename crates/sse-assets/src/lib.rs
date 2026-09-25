//! # sse-assets -- asset sources and caching
//!
//! ## Responsibilities
//! - Read-only loading of SekaiStoryRipper output: `ripper-episode` indexes and
//!   `sse-motion` clips via `ripper-format` (ADR-0006)
//! - Reject unknown `format` / `version` values instead of guessing
//! - **Explicit failure with a list of missing assets**, pointing at `ripper rip <selector>`.
//!   Silent degradation is forbidden
//! - Recording the asset version from `ripper.lock.json`
//! - Mirroring a library kept in S3 into a local cache (`remote`, ADR-0015)
//!
//! ## Not responsible for
//! - Game CDNs, manifests or decryption (ADR-0006: sse has no CDN or key handling)
//! - Resolving motion/facial bundles; the episode index is trusted (ADR-0007)
//! - Interpreting asset contents (each consuming crate does that)
//!
//! ## Allowed dependencies
//! `sse-core`, the external `ripper-format` (serde types only), and for `remote` an HTTP client
//! (reqwest + rustls/ring) with `rusty-s3` request signing.

use std::path::{Path, PathBuf};

pub use ripper_format::episode::{self, EpisodeIndex};
pub use ripper_format::motion::{self, SseMotion};

mod audio;
mod model;
pub mod remote;

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
    #[error("{0}")]
    Remote(String),
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

/// A SekaiStoryRipper output directory (the one holding `library/` and `episodes/`), or a local
/// mirror of one kept in S3.
#[derive(Clone)]
pub struct Library {
    root: PathBuf,
    mirror: Option<std::sync::Arc<remote::Mirror>>,
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.mirror {
            Some(m) => write!(f, "Library({} via {})", m.location(), self.root.display()),
            None => write!(f, "Library({})", self.root.display()),
        }
    }
}

/// Where remote libraries are mirrored: `SSE_CACHE_DIR`, else the platform cache directory.
pub fn default_cache_dir() -> PathBuf {
    let var = |name: &str| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = var("SSE_CACHE_DIR") {
        dir
    } else if let Some(dir) = var("XDG_CACHE_HOME") {
        dir.join("sse")
    } else if let Some(dir) = var("LOCALAPPDATA") {
        dir.join("sse").join("cache")
    } else if let Some(home) = var("HOME") {
        home.join(".cache").join("sse")
    } else {
        PathBuf::from(".sse-cache")
    }
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            mirror: None,
        }
    }

    /// A directory, or `s3://bucket/prefix` mirrored under `cache` (ADR-0015).
    pub fn open_location(location: &str, cache: &Path) -> Result<Self> {
        let Some(s3) = remote::S3Location::parse(location) else {
            return Ok(Self::open(location));
        };
        let s3 = s3?;
        let mut root = cache.join("s3").join(&s3.bucket);
        root.extend(s3.prefix.split('/').filter(|p| !p.is_empty()));
        let mirror = remote::Mirror::new(remote::S3::new(s3)?, root.clone());
        mirror.refresh("ripper.lock.json")?;
        Ok(Self {
            root,
            mirror: Some(std::sync::Arc::new(mirror)),
        })
    }

    /// The S3 location this library mirrors, if any.
    pub fn remote(&self) -> Option<&remote::S3Location> {
        self.mirror.as_ref().map(|m| m.location())
    }

    /// For a remote library, fetches the selector's episode index (always, it may have been
    /// re-ripped). A no-op for a local one.
    pub fn fetch_episode_index(&self, selector: &str) -> Result<()> {
        let (Some(mirror), Some((kind, rest))) = (&self.mirror, selector.split_once(':')) else {
            return Ok(());
        };
        let Some((key, no)) = rest.rsplit_once('/') else {
            return Ok(());
        };
        mirror.refresh(&format!("episodes/{kind}/{key}/{no}.json"))?;
        Ok(())
    }

    /// For a remote library, mirrors every file `index` needs. A no-op for a local one.
    pub fn sync(&self, index: &EpisodeIndex) -> Result<()> {
        match &self.mirror {
            Some(mirror) => mirror.sync_episode(index),
            None => Ok(()),
        }
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
