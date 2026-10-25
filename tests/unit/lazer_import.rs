use super::{IMPORT_FOLDER_NAME, build_import_folder};
use crate::download::collection_db::{CollectionEntry, write_entries};
use osu_db::collection::CollectionList;
use std::{fs, io::Write, path::Path};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

fn write_osz(path: &Path, files: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, contents) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

/// A run folder as a download leaves it: one `<id> name.osz` per set plus a
/// `collection.db` naming the given collections.
fn write_run(download_dir: &Path, run: &str, sets: &[u32], collections: &[(&str, &[&str])]) {
    let run_dir = download_dir.join(run);
    fs::create_dir_all(&run_dir).unwrap();
    for id in sets {
        write_osz(
            &run_dir.join(format!("{id} Artist - Title.osz")),
            &[
                (&format!("diff{id}.osu"), "osu file format v14"),
                ("bg.jpg", "jpg"),
            ],
        );
    }
    let entries: Vec<_> = collections
        .iter()
        .map(|(name, hashes)| CollectionEntry {
            name: (*name).to_string(),
            beatmap_hashes: hashes.iter().map(|h| (*h).to_string()).collect(),
        })
        .collect();
    if !entries.is_empty() {
        write_entries(&entries, &run_dir).unwrap();
    }
}

fn read_db(out_dir: &Path) -> Vec<(String, Vec<String>)> {
    CollectionList::from_file(out_dir.join("collection.db"))
        .unwrap()
        .collections
        .into_iter()
        .map(|c| {
            (
                c.name.unwrap(),
                c.beatmap_hashes.into_iter().flatten().collect(),
            )
        })
        .collect()
}

#[test]
fn merges_runs_into_one_folder() {
    let download = tempdir().unwrap();
    write_run(download.path(), "a-1", &[1, 2], &[("a-1", &["h1", "h2"])]);
    write_run(download.path(), "b-2", &[2, 3], &[("b-2", &["h2", "h3"])]);
    let out = tempdir().unwrap();

    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.runs, 2);
    assert_eq!(report.unpacked, 3);
    assert_eq!(report.already_unpacked, 1, "set 2 is in both runs");
    assert!(report.failed.is_empty());
    for id in [1, 2, 3] {
        let set = out
            .path()
            .join("Songs")
            .join(format!("{id} Artist - Title"));
        assert!(set.join(format!("diff{id}.osu")).is_file());
        assert!(set.join("bg.jpg").is_file());
    }
    assert_eq!(
        read_db(out.path()),
        vec![
            ("a-1".to_string(), vec!["h1".to_string(), "h2".to_string()]),
            ("b-2".to_string(), vec!["h2".to_string(), "h3".to_string()]),
        ]
    );
    assert!(out.path().join("osu!.name.cfg").is_file());
    assert!(!out.path().join(".partial").exists());
}

#[test]
fn rerun_is_idempotent() {
    let download = tempdir().unwrap();
    write_run(download.path(), "a-1", &[1, 2], &[("a-1", &["h1", "h2"])]);
    let out = tempdir().unwrap();

    build_import_folder(download.path(), out.path()).unwrap();
    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.unpacked, 0);
    assert_eq!(report.already_unpacked, 2);
    assert_eq!(report.beatmap_hashes, 2);
    assert_eq!(
        read_db(out.path()),
        vec![("a-1".to_string(), vec!["h1".to_string(), "h2".to_string()])]
    );
}

#[test]
fn later_runs_extend_an_existing_import_folder() {
    let download = tempdir().unwrap();
    write_run(download.path(), "a-1", &[1], &[("a-1", &["h1"])]);
    let out = tempdir().unwrap();
    build_import_folder(download.path(), out.path()).unwrap();

    // The first run's folder is gone (e.g. deleted after downloading) but its
    // collection is already in the import folder's collection.db.
    fs::remove_dir_all(download.path().join("a-1")).unwrap();
    write_run(download.path(), "b-2", &[2], &[("b-2", &["h2"])]);
    build_import_folder(download.path(), out.path()).unwrap();

    let names: Vec<_> = read_db(out.path()).into_iter().map(|(n, _)| n).collect();
    assert_eq!(names, ["a-1", "b-2"]);
}

#[test]
fn same_name_collections_merge() {
    let download = tempdir().unwrap();
    write_run(download.path(), "run-1", &[], &[("shared", &["h1"])]);
    write_run(download.path(), "run-2", &[], &[("shared", &["h1", "h2"])]);
    let out = tempdir().unwrap();

    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.collections, 1);
    assert_eq!(
        read_db(out.path()),
        vec![(
            "shared".to_string(),
            vec!["h1".to_string(), "h2".to_string()]
        )]
    );
}

#[test]
fn rejects_entries_escaping_the_set_folder() {
    let download = tempdir().unwrap();
    let run = download.path().join("run");
    fs::create_dir_all(&run).unwrap();
    write_osz(
        &run.join("7 Evil.osz"),
        &[("diff.osu", "osu"), ("../../escaped.osu", "osu")],
    );
    let out = tempdir().unwrap();

    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.failed.len(), 1);
    assert!(report.failed[0].1.contains("unsafe path"));
    assert!(!out.path().join("Songs/7 Evil").exists());
    assert!(!out.path().join("escaped.osu").exists());
    assert!(!out.path().join("Songs/escaped.osu").exists());
    assert!(!out.path().join(".partial").exists());
}

#[test]
fn rejects_archive_without_beatmaps() {
    let download = tempdir().unwrap();
    let run = download.path().join("run");
    fs::create_dir_all(&run).unwrap();
    write_osz(&run.join("8 Assets only.osz"), &[("Assets/bg.png", "png")]);
    let out = tempdir().unwrap();

    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.unpacked, 0);
    assert_eq!(report.failed.len(), 1);
    assert!(!out.path().join("Songs/8 Assets only").exists());
}

#[test]
fn rejects_corrupt_archive() {
    let download = tempdir().unwrap();
    let run = download.path().join("run");
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("9 Broken.osz"), b"not a zip").unwrap();
    let out = tempdir().unwrap();

    let report = build_import_folder(download.path(), out.path()).unwrap();

    assert_eq!(report.failed.len(), 1);
    assert!(!out.path().join("Songs/9 Broken").exists());
}

#[test]
fn skips_import_folder_and_unrelated_folders() {
    let download = tempdir().unwrap();
    write_run(download.path(), "a-1", &[1], &[("a-1", &["h1"])]);
    let notes = download.path().join("notes");
    fs::create_dir_all(&notes).unwrap();
    fs::write(notes.join("readme.txt"), "hi").unwrap();
    let out = download.path().join(IMPORT_FOLDER_NAME);

    build_import_folder(download.path(), &out).unwrap();
    let report = build_import_folder(download.path(), &out).unwrap();

    assert_eq!(report.runs, 1, "only a-1 is a run folder");
    assert_eq!(report.collections, 1);
}
