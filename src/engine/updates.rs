//! Update scans: which downloaded collections have new maps on osu!collector,
//! and the download of the ones the user picks.
//!
//! [`scan`] is the one implementation of the scan; the headless
//! `update-collections` command and the GUI both run it. A collection counts
//! as downloaded when its name in osu! carries its osu!collector id, as every
//! osu-collect download names it ("Tiebreakers-18951").

use super::{
    Downloads, Settings,
    download::{RunSettings, run_settings},
};
use crate::{
    app::{
        collection_state, failed_maps, ignored_maps,
        runtime::{self, FetchCompareSettings, FetchMissingResult},
        snapshots::{self, CollectionSnapshotFile, SnapshotDiff},
        update_source::{MissingBeatmapset, extract_collection_id},
    },
    config::load_config_or_default,
    download::{DownloadId, SelectiveDownloadCollection, SelectiveDownloadRequest},
    osu_db::{
        BeatmapReader, LazerReader, LocalBeatmapset, LocalCollection, OsuClient, StableReader,
    },
    utils::{AppError, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

/// Where a running scan is, for a progress line.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ScanProgress {
    ReadingLibrary,
    /// The library is read; `collections` are being checked on osu!collector.
    Checking {
        local_sets: usize,
        collections: usize,
    },
}

/// Everything one scan found.
pub struct Scan {
    pub client: OsuClient,
    pub db_path: PathBuf,
    pub local_collections: Vec<LocalCollection>,
    pub local_beatmapsets: Vec<LocalBeatmapset>,
    /// Ids of the downloaded collections found in osu!, deduped, in library order.
    pub collection_ids: Vec<u32>,
    /// Each collection's baseline as the library stands now.
    pub current_snapshots: HashMap<u32, CollectionSnapshotFile>,
    /// What changed since each collection's last baseline.
    pub snapshot_diffs: HashMap<u32, SnapshotDiff>,
    pub fetched: FetchMissingResult,
    /// Sets added to the collections by hand since the last scan.
    pub added_since_last_scan: usize,
    pub db_read: Duration,
    pub fetch: Duration,
}

/// The osu! library to scan: the configured install, else the default one.
pub fn library_path(settings: &Settings) -> Option<PathBuf> {
    settings.osu_path.clone().or_else(|| match settings.client {
        OsuClient::Lazer => LazerReader::default_path(),
        OsuClient::Stable => StableReader::default_path(),
    })
}

/// Read the osu! library at `db_path`, then check every downloaded collection
/// in it against osu!collector. Writes nothing but the ignored-maps reconcile
/// (an ignored set that is now installed stops being ignored); [`persist`]
/// saves the scan's state.
pub async fn scan(
    client: OsuClient,
    db_path: PathBuf,
    on_progress: impl Fn(ScanProgress),
) -> std::result::Result<Scan, String> {
    on_progress(ScanProgress::ReadingLibrary);
    let t_db = Instant::now();
    let (local_collections, local_beatmapsets, all_checksums) = tokio::task::spawn_blocking({
        let db_path = db_path.clone();
        move || runtime::read_local_database(client, db_path)
    })
    .await
    .map_err(|e| format!("task panicked: {e}"))
    .and_then(|r| r)?;
    let db_read = t_db.elapsed();

    let mut seen = HashSet::new();
    let collection_ids: Vec<u32> = local_collections
        .iter()
        .filter_map(|c| collection_id(&c.name))
        .filter(|id| seen.insert(*id))
        .collect();

    let current_snapshots =
        snapshots::current_snapshots(client, &local_collections, &local_beatmapsets, |name| {
            collection_id(name)
        });
    let snapshot_diffs = snapshots::snapshots_dir()
        .as_deref()
        .map(|dir| runtime::snapshot_diffs_for_scan(dir, &collection_ids, &current_snapshots))
        .unwrap_or_default();
    let added_since_last_scan = snapshot_diffs
        .values()
        .map(|diff| diff.manually_added.len())
        .sum();

    on_progress(ScanProgress::Checking {
        local_sets: local_beatmapsets.len(),
        collections: collection_ids.len(),
    });
    let t_fetch = Instant::now();
    let fetched = if collection_ids.is_empty() {
        FetchMissingResult {
            missing: Vec::new(),
            collection_seen: HashMap::new(),
            collection_removed_counts: HashMap::new(),
            collections: Vec::new(),
            hidden_failed_count: 0,
        }
    } else {
        let local_set_ids: HashSet<u32> = local_beatmapsets.iter().map(|bs| bs.id).collect();
        let ignored_beatmapset_ids = ignored_maps::ignored_maps_path()
            .map(|path| ignored_maps::reconcile_installed(&path, &local_set_ids))
            .unwrap_or_default();
        runtime::fetch_missing_beatmapsets(
            client,
            collection_ids.clone(),
            local_set_ids,
            all_checksums.into_iter().collect(),
            &local_collections,
            snapshot_diffs.clone(),
            FetchCompareSettings {
                hidden_failed_beatmapset_ids: failed_maps::failed_maps_path()
                    .as_deref()
                    .map(failed_maps::load)
                    .map(|fm| fm.ids())
                    .unwrap_or_default(),
                ignored_beatmapset_ids,
            },
        )
        .await?
    };

    Ok(Scan {
        client,
        db_path,
        local_collections,
        local_beatmapsets,
        collection_ids,
        current_snapshots,
        snapshot_diffs,
        fetched,
        added_since_last_scan,
        db_read,
        fetch: t_fetch.elapsed(),
    })
}

fn collection_id(name: &str) -> Option<u32> {
    extract_collection_id(name).and_then(|id| u32::try_from(id).ok())
}

/// Save what the scan saw: each collection's upstream and installed sets, and
/// its baseline rebuilt so held-back sets stay recorded as deleted (a map the
/// user deletes after this scan then shows as deleted on the next one).
/// Blocking. Skip it while a download runs: that run's completion writes the
/// baselines from its own, older view of the library.
pub fn persist(scan: &Scan) {
    let local_ids: HashSet<u32> = scan.local_beatmapsets.iter().map(|bs| bs.id).collect();
    if let Some(path) = collection_state::state_path() {
        let mut state = collection_state::load(&path);
        let local_snapshot: Vec<u32> = local_ids.iter().copied().collect();
        for (&collection_id, ids) in &scan.fetched.collection_seen {
            let installed: Vec<u32> = ids
                .iter()
                .copied()
                .filter(|id| local_ids.contains(id))
                .collect();
            state.update(
                collection_id,
                ids.clone(),
                installed,
                local_snapshot.clone(),
            );
        }
        collection_state::save(&state, &path);
    }
    if let Some(dir) = snapshots::snapshots_dir() {
        persist_baselines(
            &dir,
            scan.current_snapshots.clone(),
            scan.client,
            &scan.snapshot_diffs,
        );
    }
}

/// Write the per-collection baselines, rebuilt so every held-back set stays
/// recorded as deleted.
///
/// The rebuild and the write live in ONE function on purpose: a helper that only
/// returned the rebuilt map would be pinned by asserting its return value, and
/// dropping its call from the caller would then change nothing any test observes.
/// Here the test asserts the bytes on disk, so dropping the rebuild changes what
/// lands and reds.
pub(crate) fn persist_baselines(
    snapshot_dir: &std::path::Path,
    mut baselines: HashMap<u32, CollectionSnapshotFile>,
    client: OsuClient,
    snapshot_diffs: &HashMap<u32, SnapshotDiff>,
) {
    runtime::retain_held_back_in_snapshots(&mut baselines, client, snapshot_diffs);
    for (collection_id, snapshot) in baselines {
        snapshots::save(
            &snapshot,
            &snapshots::snapshot_path(snapshot_dir, collection_id),
        );
    }
}

/// One map set missing from a collection.
#[derive(Debug, Clone, Serialize)]
pub struct MissingSet {
    pub beatmapset_id: u32,
    /// The user deleted it from osu! after downloading; held back unless they
    /// pick it again.
    pub previously_deleted: bool,
}

/// One downloaded collection, as the scan left it.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCollection {
    pub id: u32,
    /// The name on osu!collector, else the name in osu!.
    pub name: String,
    /// Maps the collection holds in osu!.
    pub local_maps: usize,
    pub missing: Vec<MissingSet>,
    /// Maps in osu! the collection on osu!collector no longer has.
    pub removed_upstream: usize,
}

/// What the GUI shows after a scan.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateReport {
    pub client: OsuClient,
    pub library: PathBuf,
    /// Map sets in the osu! library.
    pub local_sets: usize,
    pub collections: Vec<UpdateCollection>,
    /// Sets left out because downloading them failed before (the TUI can
    /// recheck them).
    pub known_bad: usize,
    pub added_since_last_scan: usize,
}

impl UpdateReport {
    pub fn from_scan(scan: &Scan) -> Self {
        let upstream_names: HashMap<u32, &str> = scan
            .fetched
            .collections
            .iter()
            .map(|c| (c.id, c.name.as_str()))
            .collect();
        let collections = scan
            .collection_ids
            .iter()
            .map(|&id| {
                let local = scan
                    .local_collections
                    .iter()
                    .find(|c| collection_id(&c.name) == Some(id));
                UpdateCollection {
                    id,
                    name: upstream_names
                        .get(&id)
                        .map(|name| name.to_string())
                        .or_else(|| local.map(|c| c.name.clone()))
                        .unwrap_or_default(),
                    local_maps: local.map_or(0, |c| c.beatmap_checksums.len()),
                    missing: scan
                        .fetched
                        .missing
                        .iter()
                        .filter(|set| set.collection_id == id)
                        .map(|set| MissingSet {
                            beatmapset_id: set.id,
                            previously_deleted: set.previously_deleted,
                        })
                        .collect(),
                    removed_upstream: scan
                        .fetched
                        .collection_removed_counts
                        .get(&id)
                        .copied()
                        .unwrap_or(0),
                }
            })
            .collect();
        Self {
            client: scan.client,
            library: scan.db_path.clone(),
            local_sets: scan.local_beatmapsets.len(),
            collections,
            known_bad: scan.fetched.hidden_failed_count,
            added_since_last_scan: scan.added_since_last_scan,
        }
    }
}

/// A previously deleted set the user picked again, in one collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub struct SetRef {
    pub collection_id: u32,
    pub beatmapset_id: u32,
}

/// What to download from the last scan.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdatePick {
    pub collection_ids: Vec<u32>,
    #[serde(default)]
    pub reincluded: Vec<SetRef>,
}

/// The run for `pick`, built the way the TUI builds its update download.
fn build_update_request(
    scan: &Scan,
    pick: &UpdatePick,
    run: RunSettings,
) -> SelectiveDownloadRequest {
    let reincluded: HashSet<SetRef> = pick.reincluded.iter().copied().collect();
    let missing: Vec<MissingBeatmapset> = scan
        .fetched
        .missing
        .iter()
        .cloned()
        .map(|mut set| {
            set.included = !set.previously_deleted
                || reincluded.contains(&SetRef {
                    collection_id: set.collection_id,
                    beatmapset_id: set.id,
                });
            set
        })
        .collect();

    let collection_ids: Vec<u32> = scan
        .collection_ids
        .iter()
        .copied()
        .filter(|id| pick.collection_ids.contains(id))
        .collect();
    let picked = |collection_id: u32| {
        missing
            .iter()
            .filter(move |set| set.included && set.collection_id == collection_id)
            .map(|set| set.id)
    };
    // A set missing from two picked collections downloads once.
    let mut seen = HashSet::new();
    let beatmapset_ids: Vec<u32> = missing
        .iter()
        .filter(|set| set.included && collection_ids.contains(&set.collection_id))
        .map(|set| set.id)
        .filter(|id| seen.insert(*id))
        .collect();

    // A held-back set is absent from the library, so the current baseline would
    // tell the next scan it's no longer deleted; fold the deletions back in,
    // minus the sets picked again (or the next scan would hold them back).
    let mut current = scan.current_snapshots.clone();
    let fold_diffs =
        runtime::exclude_reincluded_sets(scan.snapshot_diffs.clone(), scan.client, &missing);
    runtime::retain_held_back_in_snapshots(&mut current, scan.client, &fold_diffs);
    let snapshots: Vec<CollectionSnapshotFile> = collection_ids
        .iter()
        .filter_map(|id| current.get(id).cloned())
        .collect();

    let collections = collection_ids
        .iter()
        .map(|&id| SelectiveDownloadCollection {
            id,
            // The name in osu!, so collection.db merges into the same
            // collection rather than starting a second one.
            name: scan
                .local_collections
                .iter()
                .find(|c| collection_id(&c.name) == Some(id))
                .map(|c| c.name.clone())
                .unwrap_or_default(),
            beatmapset_ids: picked(id).collect(),
        })
        .collect();

    SelectiveDownloadRequest {
        prefetched: scan
            .fetched
            .collections
            .iter()
            .filter(|c| collection_ids.contains(&c.id))
            .map(|c| (c.id, c.clone()))
            .collect(),
        collection_ids,
        beatmapset_ids,
        collections,
        config: run.config,
        snapshot_dir: snapshots::snapshots_dir(),
        snapshots,
        skip_already_imported: run.skip_already_imported,
        // The library the scan read, even if the settings changed since.
        osu_client: scan.client,
        osu_path: scan.db_path.to_string_lossy().into_owned(),
    }
}

/// The GUI's update state: the last scan, kept so a download can be built
/// from exactly what the user looked at.
#[derive(Default)]
pub struct Updates {
    last: Mutex<Option<Scan>>,
}

impl Updates {
    /// Scan the library in `settings` and keep the result. `persist` saves
    /// the scan's state (see [`persist`]).
    pub async fn scan(
        &self,
        settings: &Settings,
        persist_state: bool,
        on_progress: impl Fn(ScanProgress),
    ) -> Result<UpdateReport> {
        let db_path = library_path(settings).ok_or_else(|| {
            AppError::other("Couldn't find your osu! install. Pick its folder in Set up.")
        })?;
        let scan = scan(settings.client, db_path, on_progress)
            .await
            .map_err(AppError::other_dynamic)?;
        let scan = if persist_state {
            tokio::task::spawn_blocking(move || {
                persist(&scan);
                scan
            })
            .await
            .map_err(|e| AppError::other_dynamic(e.to_string()))?
        } else {
            scan
        };
        let report = UpdateReport::from_scan(&scan);
        *self.last.lock().expect("updates lock") = Some(scan);
        Ok(report)
    }

    /// Queue the download of `pick` from the last scan.
    pub fn download(&self, downloads: &Downloads, pick: &UpdatePick) -> Result<DownloadId> {
        let last = self.last.lock().expect("updates lock");
        let scan = last
            .as_ref()
            .ok_or_else(|| AppError::other("Scan for updates first."))?;
        let request = build_update_request(scan, pick, run_settings(&load_config_or_default())?);
        let label = match request.collection_ids.len() {
            1 => "Update (1 collection)".to_string(),
            n => format!("Update ({n} collections)"),
        };
        downloads.enqueue_update(request, &label)
    }
}

#[cfg(test)]
#[path = "../../tests/unit/engine_updates.rs"]
mod tests;
