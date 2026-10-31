use crate::app::failed_maps::{
    FailedMapsFile, failed_maps_path_in, load, reconcile, record_failures, remove_available, save,
};
use std::collections::HashSet;

#[test]
fn load_missing_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();

    let failed_maps = load(&dir.path().join("missing.json"));

    assert_eq!(failed_maps.beatmapset_ids, Vec::<u32>::new());
}

#[test]
fn save_and_load_sorts_and_dedupes_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.json");
    let failed_maps = FailedMapsFile {
        schema_version: 0,
        beatmapset_ids: vec![30, 10, 30, 20],
    };

    save(&failed_maps, &path);
    let loaded = load(&path);

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.beatmapset_ids, vec![10, 20, 30]);
}

#[test]
fn record_failures_merges_existing_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.json");
    record_failures(&path, [3, 1]);
    record_failures(&path, [2, 3]);

    let loaded = load(&path);

    assert_eq!(loaded.beatmapset_ids, vec![1, 2, 3]);
}

#[test]
fn remove_available_keeps_unavailable_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.json");
    record_failures(&path, [1, 2, 3]);

    remove_available(&path, &HashSet::from([2]));
    let loaded = load(&path);

    assert_eq!(loaded.beatmapset_ids, vec![1, 3]);
}

#[test]
fn reconcile_clears_resolved_and_records_failures() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.json");
    // A prior run left 1, 2, 3 failed.
    record_failures(&path, [1, 2, 3]);

    // A re-download resolves 1 and 2 (now on disk) and 4 fails fresh; 3 is
    // untouched this run. The successful re-download must clear 1 and 2.
    reconcile(&path, &HashSet::from([1, 2]), [4]);
    let loaded = load(&path);

    assert_eq!(loaded.beatmapset_ids, vec![3, 4]);
}

#[test]
fn reconcile_contested_id_stays_failed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("failed.json");
    record_failures(&path, [5]);

    // Same id resolved and failed in one pass: remove runs first, add re-inserts,
    // so a contested id stays failed (defensive — the two sets are disjoint in
    // practice).
    reconcile(&path, &HashSet::from([5]), [5]);
    let loaded = load(&path);

    assert_eq!(loaded.beatmapset_ids, vec![5]);
}

#[test]
fn failed_maps_path_in_uses_osu_shelf_data_file() {
    let path = failed_maps_path_in("/tmp/data".into());

    assert!(path.ends_with("osu-shelf/failed-beatmapsets.json"));
}
