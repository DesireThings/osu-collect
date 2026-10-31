//! Shared path helpers: env-var overrides and the one-time copy of the
//! pre-rename `osu-collect` folders.
//!
//! The migration runs before logging is initialised, so failures are written to
//! stderr rather than the tracing subscriber.

use super::constants::{APP_SUBDIR, LEGACY_APP_SUBDIR};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

/// Tauri's identifier before and after the rename; its config/data folders live
/// beside the app's own ones and are copied too.
const LEGACY_IDENTIFIER: &str = "io.github.desirethings.osu-collect";
const IDENTIFIER: &str = "io.github.desirethings.osu-shelf";

/// Written into a legacy folder after its copy finishes, so deleting the new
/// folder later cannot silently re-import stale data.
const MARKER_FILE: &str = ".migrated-to-osu-shelf";

/// Reads `primary`, falling back to `legacy`. Empty values count as unset.
pub fn env_override(primary: &str, legacy: &str) -> Option<PathBuf> {
    [primary, legacy].into_iter().find_map(|name| {
        let value = env::var(name).ok()?;
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
    })
}

/// Copies every legacy config/data folder to its new name. Copy-only: the
/// sources stay in place and existing destination files are never overwritten.
pub fn migrate_legacy_dirs() {
    for root in roots() {
        copy_legacy_dir(&root, LEGACY_APP_SUBDIR, APP_SUBDIR);
        copy_legacy_dir(&root, LEGACY_IDENTIFIER, IDENTIFIER);
    }
}

/// Config, data and local-data roots, deduplicated where the platform maps
/// them to the same directory. The cache root is deliberately absent: a webview
/// cache is not worth copying.
fn roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = [dirs::config_dir(), dirs::data_dir(), dirs::data_local_dir()]
        .into_iter()
        .flatten()
        .collect();
    roots.sort();
    roots.dedup();
    roots
}

fn copy_legacy_dir(root: &Path, legacy: &str, current: &str) {
    let src = root.join(legacy);
    let dst = root.join(current);
    if !src.is_dir() || src.join(MARKER_FILE).exists() {
        return;
    }

    if let Err(err) = copy_tree(&src, &dst) {
        eprintln!(
            "warning: could not copy {} to {}: {err}",
            src.display(),
            dst.display()
        );
        return;
    }

    if let Err(err) = fs::write(src.join(MARKER_FILE), b"") {
        eprintln!(
            "warning: could not write migration marker in {}: {err}",
            src.display()
        );
    }
}

/// Recursively copies regular files and directories that are not already at the
/// destination. Symlinks are skipped: these stores only hold regular files.
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if file_type.is_file() && !target.exists() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/paths.rs"]
mod tests;
