use crate::config::{constants::APP_SUBDIR, env_override};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};
use tracing::{debug, warn};

pub const STATE_FILE: &str = "collection_state.toml";
pub const STATE_ENV_PATH: &str = "OSU_SHELF_STATE";
const LEGACY_STATE_ENV_PATH: &str = "OSU_COLLECT_STATE";
const SCHEMA_VERSION: u32 = 1;
static SAVE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CollectionStateFile {
    pub schema_version: u32,
    #[serde(default)]
    pub collections: HashMap<u32, CollectionRecord>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CollectionRecord {
    pub last_seen_beatmapsets: Vec<u32>,
    #[serde(default)]
    pub last_installed_beatmapsets: Vec<u32>,
    /// local DB snapshot at time of last scan; enables manually-added detection
    #[serde(default)]
    pub snapshot_local_beatmapset_ids: Vec<u32>,
    pub last_scan_unix_secs: u64,
}

impl CollectionStateFile {
    pub fn last_seen_remote(&self, collection_id: u32) -> &[u32] {
        self.collections
            .get(&collection_id)
            .map(|r| r.last_seen_beatmapsets.as_slice())
            .unwrap_or_default()
    }

    pub fn last_installed_at_scan(&self, collection_id: u32) -> &[u32] {
        self.collections
            .get(&collection_id)
            .map(|r| r.last_installed_beatmapsets.as_slice())
            .unwrap_or_default()
    }

    pub fn snapshot_local_at_scan(&self, collection_id: u32) -> &[u32] {
        self.collections
            .get(&collection_id)
            .map(|r| r.snapshot_local_beatmapset_ids.as_slice())
            .unwrap_or_default()
    }

    pub fn update(
        &mut self,
        collection_id: u32,
        beatmapset_ids: Vec<u32>,
        installed_ids: Vec<u32>,
        local_snapshot_ids: Vec<u32>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.collections.insert(
            collection_id,
            CollectionRecord {
                last_seen_beatmapsets: beatmapset_ids,
                last_installed_beatmapsets: installed_ids,
                snapshot_local_beatmapset_ids: local_snapshot_ids,
                last_scan_unix_secs: now,
            },
        );
    }
}

pub fn state_path() -> Option<PathBuf> {
    if let Some(path) = env_override(STATE_ENV_PATH, LEGACY_STATE_ENV_PATH) {
        return Some(path);
    }
    dirs::data_local_dir().map(|d| d.join(APP_SUBDIR).join(STATE_FILE))
}

pub fn load(path: &Path) -> CollectionStateFile {
    match fs::read_to_string(path) {
        Err(_) => {
            debug!(path = %path.display(), "no state file found, starting fresh");
            CollectionStateFile {
                schema_version: SCHEMA_VERSION,
                ..Default::default()
            }
        }
        Ok(contents) => match toml::from_str::<CollectionStateFile>(&contents) {
            Ok(mut state) => {
                state.schema_version = SCHEMA_VERSION;
                state
            }
            Err(err) => {
                warn!(path = %path.display(), error = %err, "failed to parse state file, starting fresh");
                CollectionStateFile {
                    schema_version: SCHEMA_VERSION,
                    ..Default::default()
                }
            }
        },
    }
}

pub fn save(state: &CollectionStateFile, path: &Path) {
    let Ok(_guard) = SAVE_LOCK.lock() else {
        warn!(path = %path.display(), "failed to lock collection state save");
        return;
    };

    let contents = match toml::to_string_pretty(state) {
        Ok(s) => s,
        Err(err) => {
            warn!(error = %err, "failed to serialize collection state");
            return;
        }
    };

    if let Err(err) = super::write_atomic(path, "toml.tmp", &contents) {
        warn!(path = %path.display(), error = %err, "failed to save collection state");
    } else {
        debug!(path = %path.display(), "saved collection state");
    }
}

#[cfg(test)]
#[path = "../../tests/unit/collection_state.rs"]
mod tests;
