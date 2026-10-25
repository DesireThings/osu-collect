use super::{
    LazerImportEvent, handle_lazer_import_event, lazer_import_toast, schedule_lazer_import,
};
use crate::app::{App, ToastLevel};
use crate::config::Config;
use crate::download::lazer_import::ImportFolderReport;
use std::{fs, path::Path};
use tokio::sync::mpsc;

fn report(runs: usize, sets: usize, collections: usize, failed: usize) -> ImportFolderReport {
    ImportFolderReport {
        runs,
        unpacked: sets,
        collections,
        failed: (0..failed)
            .map(|i| (format!("{i}.osz").into(), "bad".to_string()))
            .collect(),
        ..ImportFolderReport::default()
    }
}

#[test]
fn toast_names_the_folder_to_pick() {
    let toast = lazer_import_toast(&report(2, 5, 2, 0), Path::new("/dl/lazer-import"));

    assert_eq!(toast.level(), ToastLevel::Success);
    assert_eq!(toast.title(), "lazer import ready: 5 sets, 2 collections");
    assert!(toast.detail().unwrap().starts_with("/dl/lazer-import"));
}

#[test]
fn toast_mentions_skipped_archives() {
    let toast = lazer_import_toast(&report(1, 1, 1, 2), Path::new("/dl/lazer-import"));

    assert!(toast.title().ends_with("· 2 skipped"));
}

#[test]
fn toast_warns_when_there_is_nothing_to_import() {
    let toast = lazer_import_toast(&report(0, 0, 0, 0), Path::new("/dl/lazer-import"));

    assert_eq!(toast.level(), ToastLevel::Warning);
    assert!(toast.detail().unwrap().contains("/dl"));
}

#[test]
fn failed_build_surfaces_an_error_toast() {
    let mut app = App::new(Config::default());

    handle_lazer_import_event(
        LazerImportEvent {
            out_dir: "/dl/lazer-import".into(),
            result: Err("permission denied".to_string()),
        },
        &mut app,
    );

    let toast = app.toasts.iter().last().unwrap();
    assert_eq!(toast.level(), ToastLevel::Danger);
    assert!(toast.title().contains("permission denied"));
}

#[tokio::test]
async fn scheduled_build_reports_back_and_refuses_a_second_run() {
    let download = tempfile::tempdir().unwrap();
    fs::create_dir_all(download.path().join("run")).unwrap();
    crate::download::collection_db::write_entries(
        &[crate::download::collection_db::CollectionEntry {
            name: "c".to_string(),
            beatmap_hashes: vec!["h".to_string()],
        }],
        &download.path().join("run"),
    )
    .unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut handle = None;

    assert!(schedule_lazer_import(
        &mut handle,
        download.path().to_path_buf(),
        &tx
    ));
    assert!(
        !schedule_lazer_import(&mut handle, download.path().to_path_buf(), &tx),
        "a second build while the first runs is refused"
    );

    let event = rx.recv().await.unwrap();
    let report = event.result.unwrap();
    assert_eq!(report.runs, 1);
    assert_eq!(report.collections, 1);
    assert_eq!(event.out_dir, download.path().join("lazer-import"));
}
