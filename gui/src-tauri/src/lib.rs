//! Tauri backend: each command is a thin call into `osu_collect::engine`,
//! with errors flattened to strings the frontend can show as-is.

use osu_collect::engine::{self, CollectionPreview, ImportFolderReport, Installs, Settings};
use serde::Serialize;
use std::path::PathBuf;

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
        .invoke_handler(tauri::generate_handler![
            detect_installs,
            load_settings,
            save_settings,
            preview_collection,
            build_lazer_import,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the osu!collect window");
}
