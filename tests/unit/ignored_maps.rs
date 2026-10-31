use crate::app::ignored_maps::{
    IgnoredMapsFile, ignored_maps_path_in, load, reconcile_installed, record_ignored, save,
};
use std::collections::HashSet;

#[test]
fn load_missing_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();

    let ignored = load(&dir.path().join("missing.json"));

    assert_eq!(ignored.beatmapset_ids, Vec::<u32>::new());
}

#[test]
fn save_and_load_sorts_and_dedupes_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ignored.json");
    let ignored = IgnoredMapsFile {
        schema_version: 0,
        beatmapset_ids: vec![30, 10, 30, 20],
    };

    save(&ignored, &path);
    let loaded = load(&path);

    assert_eq!(loaded.schema_version, 1);
    assert_eq!(loaded.beatmapset_ids, vec![10, 20, 30]);
}

#[test]
fn record_ignored_merges_existing_ids() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ignored.json");
    record_ignored(&path, [3, 1]);
    record_ignored(&path, [2, 3]);

    let loaded = load(&path);

    assert_eq!(loaded.beatmapset_ids, vec![1, 2, 3]);
}

#[test]
fn reconcile_installed_clears_installed_and_returns_remaining() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ignored.json");
    // The user marked 1, 2, 3 installed.
    record_ignored(&path, [1, 2, 3]);

    // A later scan finds 2 genuinely installed (and an unrelated 9); 2 is
    // un-ignored, 1 and 3 stay hidden and are returned for this scan.
    let remaining = reconcile_installed(&path, &HashSet::from([2, 9]));

    assert_eq!(remaining, HashSet::from([1, 3]));
    assert_eq!(load(&path).beatmapset_ids, vec![1, 3]);
}

#[test]
fn reconcile_installed_keeps_all_when_none_installed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ignored.json");
    record_ignored(&path, [5, 6]);

    let remaining = reconcile_installed(&path, &HashSet::from([7, 8]));

    assert_eq!(remaining, HashSet::from([5, 6]));
    assert_eq!(load(&path).beatmapset_ids, vec![5, 6]);
}

#[test]
fn ignored_maps_path_in_uses_osu_shelf_data_file() {
    let path = ignored_maps_path_in("/tmp/data".into());

    assert!(path.ends_with("osu-shelf/ignored-beatmapsets.json"));
}
