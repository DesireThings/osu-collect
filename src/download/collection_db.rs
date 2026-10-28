use super::{DownloadError, SelectiveDownloadCollection};
use crate::core::collection::Collection;
use crate::utils::AppError;
use osu_db::collection::{Collection as DbCollection, CollectionList};
use std::{collections::HashSet, io, path::Path, path::PathBuf};
use tracing::{debug, error, warn};

const COLLECTION_DB_CREATED: &str = "collection.db created successfully";
const COLLECTION_DB_FILENAME: &str = "collection.db";
const OSU_NAME_CFG_FILENAME: &str = "osu!.name.cfg";
const OSU_DB_VERSION: u32 = 20150203;

/// Named set of beatmap hashes destined for one entry in `collection.db`.
pub struct CollectionEntry {
    pub name: String,
    pub beatmap_hashes: Vec<String>,
}

/// Write the given entries to `<output_path>` in osu!'s `collection.db` format.
/// Duplicates within an entry are dropped.
pub fn write_collections_db(entries: &[CollectionEntry], output_path: &Path) -> io::Result<()> {
    let collections = entries
        .iter()
        .map(|entry| {
            let mut seen = HashSet::<&str>::with_capacity(entry.beatmap_hashes.len());
            DbCollection {
                name: Some(entry.name.clone()),
                beatmap_hashes: entry
                    .beatmap_hashes
                    .iter()
                    .filter(|hash| seen.insert(hash.as_str()))
                    .cloned()
                    .map(Some)
                    .collect(),
            }
        })
        .collect();

    CollectionList {
        version: OSU_DB_VERSION,
        collections,
    }
    .to_file(output_path)
    .map_err(|err| io::Error::other(err.to_string()))
}

pub async fn write_collection_db(
    collection: Collection,
    db_collection_name: String,
    output_dir: PathBuf,
) -> Result<(), DownloadError> {
    run_blocking(move || create_collection_db(&collection, &db_collection_name, &output_dir))
        .await
        .map(|()| {
            debug!(COLLECTION_DB_CREATED);
        })
        .map_err(|err| {
            let message = format!("failed to create collection.db: {err}");
            error!(error = %err, "failed to create collection.db");
            DownloadError::internal(message)
        })
}

pub async fn write_selective_collection_db(
    collection: Collection,
    collections: Vec<SelectiveDownloadCollection>,
    verified: HashSet<u32>,
    output_dir: PathBuf,
) -> Result<(), DownloadError> {
    run_blocking(move || {
        create_selective_collection_db(&collection, &collections, &verified, &output_dir)
    })
    .await
    .map(|()| {
        debug!(COLLECTION_DB_CREATED);
    })
    .map_err(|err| {
        let message = format!("failed to create collection.db: {err}");
        error!(error = %err, "failed to create selective collection.db");
        DownloadError::internal(message)
    })
}

/// Remove `collection.db` and `osu!.name.cfg` from a stopped run's folder when
/// they are its only entries. A collection run writes them before downloading
/// anything, so a run stopped before any map landed would otherwise leave a
/// folder of just these two, which `try_remove_empty_output_dir` then keeps.
pub async fn remove_lone_collection_db(output_dir: &Path) {
    let Ok(mut entries) = tokio::fs::read_dir(output_dir).await else {
        return;
    };
    let mut ours = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        if name != COLLECTION_DB_FILENAME && name != OSU_NAME_CFG_FILENAME {
            return;
        }
        ours.push(entry.path());
    }
    for path in ours {
        if let Err(err) = tokio::fs::remove_file(&path).await {
            warn!(error = %err, path = %path.display(), "failed to remove file from stopped run");
        }
    }
}

async fn run_blocking<F, T>(f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| {
            AppError::other_dynamic(format!("spawn_blocking panicked: {err}").into_boxed_str())
        })
        .and_then(|r| r)
}

pub fn create_collection_db(
    collection: &Collection,
    collection_name: &str,
    output_dir: &Path,
) -> Result<(), AppError> {
    write_entries(
        &[CollectionEntry {
            name: collection_name.to_string(),
            beatmap_hashes: collection
                .beatmapsets
                .iter()
                .flat_map(|beatmapset| beatmapset.beatmaps.iter().map(|b| b.checksum.clone()))
                .collect(),
        }],
        output_dir,
    )
}

pub fn create_selective_collection_db(
    collection: &Collection,
    collections: &[SelectiveDownloadCollection],
    newly_downloaded: &HashSet<u32>,
    output_dir: &Path,
) -> Result<(), AppError> {
    let entries = collections
        .iter()
        .filter_map(|selected| {
            let hashes: Vec<String> = collection
                .beatmapsets
                .iter()
                .filter(|beatmapset| {
                    selected.beatmapset_ids.contains(&beatmapset.id)
                        && newly_downloaded.contains(&beatmapset.id)
                })
                .flat_map(|beatmapset| beatmapset.beatmaps.iter().map(|b| b.checksum.clone()))
                .collect();
            (!hashes.is_empty()).then(|| CollectionEntry {
                name: selected.name.clone(),
                beatmap_hashes: hashes,
            })
        })
        .collect::<Vec<_>>();

    if entries.is_empty() {
        return Ok(());
    }
    write_entries(&entries, output_dir)
}

pub(crate) fn write_entries(
    entries: &[CollectionEntry],
    output_dir: &Path,
) -> Result<(), AppError> {
    let db_path = output_dir.join(COLLECTION_DB_FILENAME);
    write_collections_db(entries, &db_path).map_err(|e| {
        AppError::other_dynamic(format!("failed to write collection.db: {e}").into_boxed_str())
    })?;

    let cfg_path = output_dir.join(OSU_NAME_CFG_FILENAME);
    std::fs::write(&cfg_path, "").map_err(|e| {
        AppError::other_dynamic(format!("failed to write osu!.name.cfg: {e}").into_boxed_str())
    })?;

    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/core_db_writer.rs"]
mod tests;
