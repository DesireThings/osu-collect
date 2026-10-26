//! Tauri backend: each command is a thin call into `osu_collect::engine`,
//! with errors flattened to strings the frontend can show as-is.

use osu_collect::engine::{
    self, CollectionPreview, DownloadStatus, Downloads, ImportFolderReport, Installs, Settings,
};
use serde::Serialize;
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};

/// Event every download status change is pushed to the window under.
const DOWNLOAD_STATUS_EVENT: &str = "download-status";

#[tauri::command]
fn detect_installs() -> Installs {
    engine::detect_installs()
}

#[tauri::command]
fn load_settings() -> Settings {
    engine::load_settings()
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    engine::save_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
async fn preview_collection(input: String) -> Result<CollectionPreview, String> {
    engine::preview_collection(&input)
        .await
        .map_err(|e| e.to_string())
}

/// Async so it runs on Tauri's Tokio runtime: queueing can start the pipeline,
/// which spawns tasks.
#[tauri::command]
async fn start_download(input: String, downloads: State<'_, Downloads>) -> Result<u64, String> {
    downloads.enqueue(&input).map_err(|e| e.to_string())
}

#[tauri::command]
fn cancel_download(id: u64, downloads: State<'_, Downloads>) {
    downloads.cancel(id);
}

#[tauri::command]
fn list_downloads(downloads: State<'_, Downloads>) -> Vec<DownloadStatus> {
    downloads.list()
}

/// Result of building the lazer import folder, plus where it is.
#[derive(Serialize)]
struct LazerImportResult {
    folder: PathBuf,
    report: ImportFolderReport,
}

#[tauri::command]
async fn build_lazer_import(download_dir: PathBuf) -> Result<LazerImportResult, String> {
    if !download_dir.is_dir() {
        return Err(format!(
            "the download folder doesn't exist: {}",
            download_dir.display()
        ));
    }
    let folder = download_dir.join(engine::IMPORT_FOLDER_NAME);
    let report = tokio::task::spawn_blocking({
        let folder = folder.clone();
        move || engine::build_import_folder(&download_dir, &folder)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    Ok(LazerImportResult { folder, report })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(Downloads::new(Arc::new(move |status: &DownloadStatus| {
                let _ = handle.emit(DOWNLOAD_STATUS_EVENT, status);
            })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_installs,
            load_settings,
            save_settings,
            preview_collection,
            build_lazer_import,
            start_download,
            cancel_download,
            list_downloads,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the osu-collect window");
}
