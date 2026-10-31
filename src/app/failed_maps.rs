use crate::config::{constants::APP_SUBDIR, env_override};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
use tracing::{debug, warn};

pub const FAILED_MAPS_FILE: &str = "failed-beatmapsets.json";
pub const FAILED_MAPS_ENV_PATH: &str = "OSU_SHELF_FAILED_MAPS";
const LEGACY_FAILED_MAPS_ENV_PATH: &str = "OSU_COLLECT_FAILED_MAPS";
const SCHEMA_VERSION: u32 = 1;
static SAVE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailedMapsFile {
    pub schema_version: u32,
    #[serde(default)]
    pub beatmapset_ids: Vec<u32>,
}

impl FailedMapsFile {
    pub fn ids(&self) -> HashSet<u32> {
        self.beatmapset_ids.iter().copied().collect()
    }

    pub fn insert_all(&mut self, ids: impl IntoIterator<Item = u32>) {
        self.beatmapset_ids.extend(ids);
        normalize(&mut self.beatmapset_ids);
    }

    pub fn remove_all(&mut self, ids: &HashSet<u32>) {
        self.beatmapset_ids.retain(|id| !ids.contains(id));
    }
}

pub fn failed_maps_path() -> Option<PathBuf> {
    if let Some(path) = env_override(FAILED_MAPS_ENV_PATH, LEGACY_FAILED_MAPS_ENV_PATH) {
        return Some(path);
    }

    platform_data_dir().map(failed_maps_path_in)
}

pub fn failed_maps_path_in(base: PathBuf) -> PathBuf {
    base.join(APP_SUBDIR).join(FAILED_MAPS_FILE)
}

#[cfg(windows)]
fn platform_data_dir() -> Option<PathBuf> {
    dirs::data_dir()
}

#[cfg(not(windows))]
fn platform_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
}

pub fn load(path: &Path) -> FailedMapsFile {
    match fs::read_to_string(path) {
        Err(_) => {
            debug!(path = %path.display(), "no failed maps file found");
            FailedMapsFile {
                schema_version: SCHEMA_VERSION,
                ..Default::default()
            }
        }
        Ok(contents) => match serde_json::from_str::<FailedMapsFile>(&contents) {
            Ok(mut failed_maps) => {
                failed_maps.schema_version = SCHEMA_VERSION;
                normalize(&mut failed_maps.beatmapset_ids);
                failed_maps
            }
            Err(err) => {
                warn!(path = %path.display(), error = %err, "failed to parse failed maps file");
                FailedMapsFile {
                    schema_version: SCHEMA_VERSION,
                    ..Default::default()
                }
            }
        },
    }
}

pub fn save(failed_maps: &FailedMapsFile, path: &Path) {
    let Ok(_guard) = SAVE_LOCK.lock() else {
        warn!(path = %path.display(), "failed to lock failed maps save");
        return;
    };

    let mut failed_maps = failed_maps.clone();
    failed_maps.schema_version = SCHEMA_VERSION;
    normalize(&mut failed_maps.beatmapset_ids);

    let contents = match serde_json::to_string_pretty(&failed_maps) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(error = %err, "failed to serialize failed maps");
            return;
        }
    };

    if let Err(err) = super::write_atomic(path, "json.tmp", &contents) {
        warn!(path = %path.display(), error = %err, "failed to save failed maps");
    } else {
        debug!(path = %path.display(), "saved failed maps");
    }
}

pub fn record_failures(path: &Path, ids: impl IntoIterator<Item = u32>) {
    let mut failed_maps = load(path);
    failed_maps.insert_all(ids);
    save(&failed_maps, path);
}

pub fn remove_available(path: &Path, available: &HashSet<u32>) {
    let mut failed_maps = load(path);
    failed_maps.remove_all(available);
    save(&failed_maps, path);
}

/// Reconcile the persisted file with one download run's outcome in a single
/// load → modify → save: drop every id that is now verifiably on disk
/// (`resolved`) and add this run's `failures`. Doing both in one pass is what
/// lets a successful re-download clear a previously-failed map — a separate
/// add and remove could race and lose each other's update. `resolved` and
/// `failures` are disjoint, but remove runs first so a contested id stays failed.
pub fn reconcile(path: &Path, resolved: &HashSet<u32>, failures: impl IntoIterator<Item = u32>) {
    let mut failed_maps = load(path);
    failed_maps.remove_all(resolved);
    failed_maps.insert_all(failures);
    save(&failed_maps, path);
}

fn normalize(ids: &mut Vec<u32>) {
    ids.sort_unstable();
    ids.dedup();
}

#[cfg(test)]
#[path = "../../tests/unit/failed_maps.rs"]
mod tests;
