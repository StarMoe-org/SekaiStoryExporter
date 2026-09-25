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

/// The bundle's `BuildModelData` MonoBehaviour: what the game itself loads. A bundle may carry
/// more than one model (e.g. JP `sub_asahi` holds `sub_troupemember_t04` and `_t05`); this names
/// the one in use.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BuildModelData {
    moc3_file_name: String,
    #[serde(default)]
    physics_file_name: String,
    #[serde(default)]
    texture_names: Vec<String>,
}

pub(crate) fn load(dir: &Path) -> Result<Model3> {
    let build = dir.join("buildmodeldata.json");
    if build.is_file() {
        let b: BuildModelData = crate::read_json(&build)?;
        let moc = b
            .moc3_file_name
            .strip_suffix(".bytes")
            .unwrap_or(&b.moc3_file_name);
        return Ok(Model3 {
            dir: dir.to_owned(),
            moc: dir.join(moc),
            textures: b.texture_names.iter().map(|t| dir.join(t)).collect(),
            physics: (!b.physics_file_name.is_empty()).then(|| dir.join(&b.physics_file_name)),
        });
    }
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
        textures: raw
            .file_references
            .textures
            .iter()
            .map(|t| join(t))
            .collect(),
        physics: raw.file_references.physics.as_deref().map(join),
    })
}
