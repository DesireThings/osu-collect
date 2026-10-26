//! One osu!stable-shaped folder that lazer imports in a single pass, built from
//! any number of download runs.
//!
//! lazer's "import from previous version" only reads extracted `Songs/<set>/`
//! directories (it never opens `.osz` files) and imports `collection.db` after
//! the beatmaps, merging collections by name. So every run's `.osz` files are
//! unpacked into one shared `Songs/`, and every run's `collection.db` is merged
//! into one. Run folders are only read, never changed.

use super::collection_db::{CollectionEntry, write_entries};
use osu_db::collection::CollectionList;
use osu_downloader::{OutputEntry, classify_output_entry};
use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

/// Folder name the import folder gets inside the download directory by default.
pub const IMPORT_FOLDER_NAME: &str = "lazer-import";
const SONGS_DIR: &str = "Songs";
const COLLECTION_DB_FILENAME: &str = "collection.db";
/// Sets are extracted here first and renamed into `Songs/` once complete, so an
/// interrupted unpack never leaves a half-written set where lazer would import it.
const PARTIAL_DIR: &str = ".partial";

/// What one [`build_import_folder`] call did.
#[derive(Debug, Default, serde::Serialize)]
pub struct ImportFolderReport {
    /// Run folders that held at least one `.osz` or a `collection.db`.
    pub runs: usize,
    /// Sets unpacked into `Songs/` by this call.
    pub unpacked: usize,
    /// Sets already in `Songs/` from an earlier call (a set shared by several
    /// collections is also only unpacked once).
    pub already_unpacked: usize,
    /// Archives that could not be unpacked, with the reason.
    pub failed: Vec<(PathBuf, String)>,
    /// Collections in the written `collection.db`.
    pub collections: usize,
    /// Beatmap hashes across those collections, after de-duplication.
    pub beatmap_hashes: usize,
}

/// Build (or extend) the import folder at `out_dir` from every run folder
/// directly inside `download_dir`. Safe to re-run: sets already in `Songs/` are
/// skipped and an existing `collection.db` in `out_dir` is merged, not replaced.
pub fn build_import_folder(download_dir: &Path, out_dir: &Path) -> io::Result<ImportFolderReport> {
    let songs_dir = out_dir.join(SONGS_DIR);
    let partial_dir = out_dir.join(PARTIAL_DIR);
    fs::create_dir_all(&songs_dir)?;
    let out_canonical = fs::canonicalize(out_dir)?;

    let mut report = ImportFolderReport::default();
    let mut entries = read_collection_entries(&out_dir.join(COLLECTION_DB_FILENAME));

    for run_dir in sorted_subdirs(download_dir)? {
        if fs::canonicalize(&run_dir).is_ok_and(|p| p == out_canonical) {
            continue;
        }

        let mut is_run = false;
        for archive in sorted_archives(&run_dir)? {
            is_run = true;
            match unpack_set(&archive, &songs_dir, &partial_dir) {
                Ok(true) => report.unpacked += 1,
                Ok(false) => report.already_unpacked += 1,
                Err(reason) => {
                    warn!(archive = %archive.display(), %reason, "could not unpack beatmapset");
                    report.failed.push((archive, reason));
                }
            }
        }

        let db_path = run_dir.join(COLLECTION_DB_FILENAME);
        if db_path.is_file() {
            is_run = true;
            for entry in read_collection_entries(&db_path) {
                merge_entry(&mut entries, entry);
            }
        }

        if is_run {
            report.runs += 1;
        }
    }

    let _ = fs::remove_dir_all(&partial_dir);

    // Dedupe here as well so the report counts what `write_entries` writes.
    for entry in &mut entries {
        let mut seen = HashSet::with_capacity(entry.beatmap_hashes.len());
        entry
            .beatmap_hashes
            .retain(|hash| seen.insert(hash.clone()));
    }
    report.collections = entries.len();
    report.beatmap_hashes = entries.iter().map(|e| e.beatmap_hashes.len()).sum();

    if !entries.is_empty() {
        write_entries(&entries, out_dir).map_err(|e| io::Error::other(e.to_string()))?;
    }

    info!(
        runs = report.runs,
        unpacked = report.unpacked,
        already_unpacked = report.already_unpacked,
        failed = report.failed.len(),
        collections = report.collections,
        "built lazer import folder"
    );
    Ok(report)
}

/// Fold `entry` into `entries`, joining it with a same-named collection the way
/// lazer's importer does.
fn merge_entry(entries: &mut Vec<CollectionEntry>, entry: CollectionEntry) {
    match entries.iter_mut().find(|e| e.name == entry.name) {
        Some(existing) => existing.beatmap_hashes.extend(entry.beatmap_hashes),
        None => entries.push(entry),
    }
}

/// Collections in a `collection.db`. Missing or unreadable reads as empty: an
/// unreadable run database is logged and skipped rather than failing the build.
fn read_collection_entries(path: &Path) -> Vec<CollectionEntry> {
    if !path.is_file() {
        return Vec::new();
    }
    match CollectionList::from_file(path) {
        Ok(list) => list
            .collections
            .into_iter()
            .filter_map(|c| {
                Some(CollectionEntry {
                    name: c.name?,
                    beatmap_hashes: c.beatmap_hashes.into_iter().flatten().collect(),
                })
            })
            .collect(),
        Err(err) => {
            warn!(path = %path.display(), error = %err, "skipping unreadable collection.db");
            Vec::new()
        }
    }
}

fn sorted_subdirs(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn sorted_archives(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut archives = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && matches!(
                classify_output_entry(&entry.file_name()),
                OutputEntry::Archive { .. }
            )
        {
            archives.push(entry.path());
        }
    }
    archives.sort();
    Ok(archives)
}

/// Unpack one `.osz` into `Songs/<archive stem>/`. `Ok(false)` when that folder
/// already exists. Rejects archives with entries that would escape the set
/// folder and archives with no `.osu` file (lazer would skip the folder).
fn unpack_set(archive: &Path, songs_dir: &Path, partial_dir: &Path) -> Result<bool, String> {
    let stem = archive
        .file_stem()
        .ok_or_else(|| "archive has no file name".to_string())?;
    let dest = songs_dir.join(stem);
    if dest.is_dir() {
        return Ok(false);
    }

    let staging = partial_dir.join(stem);
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;

    let result = extract_into(archive, &staging).and_then(|osu_files| {
        if osu_files == 0 {
            return Err("archive has no .osu files".to_string());
        }
        fs::rename(&staging, &dest).map_err(|e| e.to_string())
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result.map(|()| true)
}

/// Extract every entry of `archive` under `dest`; returns the `.osu` count.
fn extract_into(archive: &Path, dest: &Path) -> Result<usize, String> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut osu_files = 0;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let relative = entry.enclosed_name().ok_or_else(|| {
            format!(
                "unsafe path in archive: {}",
                String::from_utf8_lossy(entry.name_raw())
            )
        })?;
        let path = dest.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&path).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;

        if relative
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("osu"))
        {
            osu_files += 1;
        }
    }

    Ok(osu_files)
}

#[cfg(test)]
#[path = "../../tests/unit/lazer_import.rs"]
mod tests;
