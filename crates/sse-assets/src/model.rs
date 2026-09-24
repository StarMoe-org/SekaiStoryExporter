use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{AssetError, Result};

/// The parts of a `model3.json` sse uses, with paths resolved against the bundle directory.
#[derive(Debug, Clone)]
pub struct Model3 {
    pub dir: PathBuf,
    pub moc: PathBuf,
    pub textures: Vec<PathBuf>,
    pub physics: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Raw {
    file_references: Refs,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Refs {
    moc: String,
    #[serde(default)]
    textures: Vec<String>,
    physics: Option<String>,
}

pub(crate) fn load(dir: &Path) -> Result<Model3> {
    let entries = sse_core::fs::read_dir_sorted(dir).map_err(|source| AssetError::Io {
        path: dir.to_owned(),
        source,
    })?;
    let file = entries
        .into_iter()
        .find(|p| p.to_string_lossy().ends_with(".model3.json"))
        .ok_or_else(|| AssetError::Io {
            path: dir.to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no *.model3.json"),
        })?;
    let raw: Raw = crate::read_json(&file)?;
    let join = |p: &str| dir.join(p);
    Ok(Model3 {
        dir: dir.to_owned(),
        moc: join(&raw.file_references.moc),
        textures: raw.file_references.textures.iter().map(|t| join(t)).collect(),
        physics: raw.file_references.physics.as_deref().map(join),
    })
}
