use crate::{
    app::{
        runtime,
        snapshots::{self, CollectionSnapshot, CollectionSnapshotFile},
    },
    osu_db::{LocalBeatmap, LocalBeatmapset, LocalCollection, Md5, OsuClient, checksum},
};
use tempfile::tempdir;

fn md5(hex: &str) -> Md5 {
    checksum::parse_hex(hex).expect("valid 32-char hex in test")
}

// Short hex aliases for readability: each is a deterministic 32-char hex.
const HASH_A: &str = "0a000000000000000000000000000000";
const HASH_B: &str = "0b000000000000000000000000000000";
const HASH_1: &str = "00000000000000000000000000000001";
const HASH_2: &str = "00000000000000000000000000000002";

#[test]
fn diff_empty_baseline_returns_no_changes() {
    let current = CollectionSnapshot {
        stable_hashes: vec!["a".to_string()],
        lazer_ids: vec![1],
    };

    let diff = snapshots::diff_snapshot(None, &current);

    assert!(diff.manually_deleted.is_empty());
    assert!(diff.manually_added.is_empty());
}

#[test]
fn diff_full_overlap_returns_no_changes() {
    let previous = CollectionSnapshot {
        stable_hashes: vec!["a".to_string(), "b".to_string()],
        lazer_ids: vec![1, 2],
    };
    let current = previous.clone();

    let diff = snapshots::diff_snapshot(Some(&previous), &current);

    assert!(diff.manually_deleted.is_empty());
    assert!(diff.manually_added.is_empty());
}

#[test]
fn diff_partial_deletion_marks_missing_previous_values() {
    let previous = CollectionSnapshot {
        stable_hashes: vec!["a".to_string(), "b".to_string()],
        lazer_ids: vec![1, 2],
    };
    let current = CollectionSnapshot {
        stable_hashes: vec!["a".to_string()],
        lazer_ids: vec![1],
    };

    let diff = snapshots::diff_snapshot(Some(&previous), &current);

    assert_eq!(diff.manually_deleted.stable_hashes, ["b"]);
    assert_eq!(diff.manually_deleted.lazer_ids, [2]);
    assert!(diff.manually_added.is_empty());
}

#[test]
fn diff_partial_addition_marks_new_current_values() {
    let previous = CollectionSnapshot {
        stable_hashes: vec!["a".to_string()],
        lazer_ids: vec![1],
    };
    let current = CollectionSnapshot {
        stable_hashes: vec!["a".to_string(), "b".to_string()],
        lazer_ids: vec![1, 2],
    };

    let diff = snapshots::diff_snapshot(Some(&previous), &current);

    assert!(diff.manually_deleted.is_empty());
    assert_eq!(diff.manually_added.stable_hashes, ["b"]);
    assert_eq!(diff.manually_added.lazer_ids, [2]);
}

#[test]
fn diff_mixed_marks_additions_and_deletions() {
    let previous = CollectionSnapshot {
        stable_hashes: vec!["a".to_string(), "b".to_string()],
        lazer_ids: vec![1, 2],
    };
    let current = CollectionSnapshot {
        stable_hashes: vec!["b".to_string(), "c".to_string()],
        lazer_ids: vec![2, 3],
    };

    let diff = snapshots::diff_snapshot(Some(&previous), &current);

    assert_eq!(diff.manually_deleted.stable_hashes, ["a"]);
    assert_eq!(diff.manually_deleted.lazer_ids, [1]);
    assert_eq!(diff.manually_added.stable_hashes, ["c"]);
    assert_eq!(diff.manually_added.lazer_ids, [3]);
}

#[test]
fn snapshot_save_and_load_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("collection-42.json");
    let snapshot = CollectionSnapshotFile::new(
        42,
        "collection - 42".to_string(),
        CollectionSnapshot {
            stable_hashes: vec!["hash".to_string()],
            lazer_ids: vec![100],
        },
    );

    snapshots::save(&snapshot, &path);
    let loaded = snapshots::load(&path).unwrap();

    assert_eq!(loaded.collection_id, "42");
    assert_eq!(loaded.name, "collection - 42");
    assert_eq!(loaded.version, 1);
    assert_eq!(loaded.snapshot.stable_hashes, ["hash"]);
    assert_eq!(loaded.snapshot.lazer_ids, [100]);
    assert!(loaded.last_run_at.contains('T'));
}

#[test]
fn snapshot_load_corrupt_file_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("collection-1.json");
    std::fs::write(&path, b"not json").unwrap();

    assert!(snapshots::load(&path).is_none());
}

#[test]
fn snapshot_load_future_version_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("collection-1.json");
    std::fs::write(
        &path,
        r#"{"collection_id":"1","name":"future","last_run_at":"2026-05-13T00:00:00Z","snapshot":{},"version":2}"#,
    )
    .unwrap();

    assert!(snapshots::load(&path).is_none());
}

#[test]
fn snapshot_dir_in_adds_expected_suffix() {
    let base = std::path::PathBuf::from("/tmp/osu-data");

    let path = snapshots::snapshot_dir_in(base);

    assert!(path.ends_with("osu-shelf/snapshots"));
}

#[test]
fn current_stable_snapshot_uses_collection_hashes() {
    let collections = vec![LocalCollection {
        name: "cool - 42".to_string(),
        beatmap_checksums: [md5(HASH_B), md5(HASH_A), md5(HASH_A)].into(),
    }];

    let snapshots =
        snapshots::current_snapshots(OsuClient::Stable, &collections, &[], |_| Some(42));

    // stable_hashes are persisted as hex strings, sorted and deduped
    assert_eq!(snapshots[&42].snapshot.stable_hashes, [HASH_A, HASH_B]);
    assert!(snapshots[&42].snapshot.lazer_ids.is_empty());
}

#[test]
fn current_lazer_snapshot_maps_hashes_to_beatmapset_ids() {
    let collections = vec![LocalCollection {
        name: "cool - 42".to_string(),
        beatmap_checksums: [md5(HASH_2), md5(HASH_1)].into(),
    }];
    let beatmapsets = vec![
        LocalBeatmapset {
            id: 20,
            beatmaps: [LocalBeatmap {
                checksum: md5(HASH_2),
            }]
            .into(),
        },
        LocalBeatmapset {
            id: 10,
            beatmaps: [LocalBeatmap {
                checksum: md5(HASH_1),
            }]
            .into(),
        },
    ];

    let snapshots =
        snapshots::current_snapshots(OsuClient::Lazer, &collections, &beatmapsets, |_| Some(42));

    assert!(snapshots[&42].snapshot.stable_hashes.is_empty());
    assert_eq!(snapshots[&42].snapshot.lazer_ids, [10, 20]);
}

#[test]
fn deleted_snapshot_deselects_matching_stable_beatmap_hash() {
    let diff = snapshots::SnapshotDiff {
        manually_deleted: CollectionSnapshot {
            stable_hashes: vec!["deleted".to_string()],
            lazer_ids: Vec::new(),
        },
        manually_added: CollectionSnapshot::default(),
    };
    let mut current = std::collections::HashMap::new();
    current.insert(
        42,
        CollectionSnapshotFile::new(
            42,
            "collection - 42".to_string(),
            CollectionSnapshot::default(),
        ),
    );
    let dir = tempdir().unwrap();
    let previous = CollectionSnapshotFile::new(
        42,
        "collection - 42".to_string(),
        CollectionSnapshot {
            stable_hashes: vec!["deleted".to_string()],
            lazer_ids: Vec::new(),
        },
    );
    snapshots::save(&previous, &snapshots::snapshot_path(dir.path(), 42));

    let diffs = runtime::snapshot_diffs_for_scan(dir.path(), &[42], &current);

    assert_eq!(diffs[&42], diff);
}
