//! Deterministic filesystem helpers (determinism rule D-3).

use std::io;
use std::path::{Path, PathBuf};

/// Directory entries sorted by file name. `std::fs::read_dir` order is filesystem-defined.
#[allow(clippy::disallowed_methods)]
pub fn read_dir_sorted(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut entries = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(entries)
}
