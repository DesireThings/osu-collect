use super::runtime::owned_beatmapset_ids;
use crate::{
    config::{constants::APP_SUBDIR, env_override},
    osu_db::OsuClient,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::UNIX_EPOCH,
};
use tracing::{debug, warn};

pub const LIBRARY_CACHE_FILE: &str = "library-cache.json";
pub const LIBRARY_CACHE_ENV_PATH: &str = "OSU_SHELF_LIBRARY_CACHE";
const LEGACY_LIBRARY_CACHE_ENV_PATH: &str = "OSU_COLLECT_LIBRARY_CACHE";
/// Bump to drop every saved entry. 2: entries from osu-db 0.3.0, which read a
/// stable `osu!.db` written since 20250107 as an empty library.
pub(crate) const SCHEMA_VERSION: u32 = 2;
static SAVE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Memoized owned-beatmapset-id set for one osu! client database, keyed by the
/// db file path plus its mtime. A single-entry cache: a different path or a
/// changed mtime invalidates it and forces a fresh read.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryCacheFile {
    pub schema_version: u32,
    pub db_path: String,
    pub mtime_ns: u128,
    #[serde(default)]
    pub beatmapset_ids: Vec<u32>,
}

/// The database file that backs `read_local_database` for `client`: `osu!.db`
/// for stable, `client.realm` for lazer. Joined onto the install directory; no
/// IO. Reused by the config-tab hint so it always names the file actually read.
pub fn db_file_path(client: OsuClient, install_dir: &Path) -> PathBuf {
    let file = match client {
        OsuClient::Stable => "osu!.db",
        OsuClient::Lazer => "client.realm",
    };
    install_dir.join(file)
}

pub fn library_cache_path() -> Option<PathBuf> {
    if let Some(path) = env_override(LIBRARY_CACHE_ENV_PATH, LEGACY_LIBRARY_CACHE_ENV_PATH) {
        return Some(path);
    }

    platform_data_dir().map(library_cache_path_in)
}

pub fn library_cache_path_in(base: PathBuf) -> PathBuf {
    base.join(APP_SUBDIR).join(LIBRARY_CACHE_FILE)
}

#[cfg(windows)]
fn platform_data_dir() -> Option<PathBuf> {
    dirs::data_dir()
}

#[cfg(not(windows))]
fn platform_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir()
}

/// Owned beatmapset ids for the configured client, served from the on-disk
/// cache when the db file's path + mtime match the cached entry, otherwise read
/// fresh (and the cache rewritten). Best-effort: a missing cache path falls
/// back to a direct read with no memoization.
pub fn owned_ids_cached(client: OsuClient, install_dir: PathBuf) -> Result<HashSet<u32>, String> {
    let db_path = db_file_path(client, &install_dir);
    let read = move || owned_beatmapset_ids(client, install_dir);
    match library_cache_path() {
        Some(cache_path) => owned_ids_cached_with(&db_path, &cache_path, read),
        None => read(),
    }
}

/// Cache core, isolated from the client/path resolution so it can be tested with
/// an arbitrary db file and a stubbed reader.
pub(crate) fn owned_ids_cached_with<F>(
    db_path: &Path,
    cache_path: &Path,
    read: F,
) -> Result<HashSet<u32>, String>
where
    F: FnOnce() -> Result<HashSet<u32>, String>,
{
    let mtime = mtime_ns(db_path)?;
    let db_path_str = db_path.to_string_lossy();

    if let Some(cached) = load(cache_path)
        && cached.schema_version == SCHEMA_VERSION
        && cached.db_path == db_path_str
        && cached.mtime_ns == mtime
    {
        debug!(path = %db_path.display(), "library cache hit");
        return Ok(cached.beatmapset_ids.into_iter().collect());
    }

    let ids = read()?;
    save(
        &LibraryCacheFile {
            schema_version: SCHEMA_VERSION,
            db_path: db_path_str.into_owned(),
            mtime_ns: mtime,
            beatmapset_ids: ids.iter().copied().collect(),
        },
        cache_path,
    );
    Ok(ids)
}

fn mtime_ns(db_path: &Path) -> Result<u128, String> {
    let modified = fs::metadata(db_path)
        .and_then(|meta| meta.modified())
        .map_err(|err| format!("cannot stat {}: {err}", db_path.display()))?;
    modified
        .duration_since(UNIX_EPOCH)
        .map(|dur| dur.as_nanos())
        .map_err(|err| format!("db mtime predates unix epoch: {err}"))
}

fn load(path: &Path) -> Option<LibraryCacheFile> {
    let contents = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<LibraryCacheFile>(&contents) {
        Ok(cache) => Some(cache),
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to parse library cache");
            None
        }
    }
}

fn save(cache: &LibraryCacheFile, path: &Path) {
    let Ok(_guard) = SAVE_LOCK.lock() else {
        warn!(path = %path.display(), "failed to lock library cache save");
        return;
    };

    let contents = match serde_json::to_string_pretty(cache) {
        Ok(contents) => contents,
        Err(err) => {
            warn!(error = %err, "failed to serialize library cache");
            return;
        }
    };

    if let Err(err) = super::write_atomic(path, "json.tmp", &contents) {
        warn!(path = %path.display(), error = %err, "failed to save library cache");
    } else {
        debug!(path = %path.display(), "saved library cache");
    }
}

#[cfg(test)]
#[path = "../../tests/unit/library_cache.rs"]
mod tests;
