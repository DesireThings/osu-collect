//! Background build of the lazer import folder (`l` on the Downloads tab).
//!
//! Unpacking every run's `.osz` can take a while on a big download directory,
//! so the build runs on a blocking thread and reports back as one event; the
//! result toast stays until dismissed because it carries the folder to pick in
//! lazer's setup wizard.

use crate::{
    app::{App, Toast},
    download::lazer_import::{IMPORT_FOLDER_NAME, ImportFolderReport, build_import_folder},
};
use std::path::{Path, PathBuf};
use tokio::{sync::mpsc, task::JoinHandle};
use tracing::warn;

/// Outcome of one import-folder build.
#[derive(Debug)]
pub struct LazerImportEvent {
    pub out_dir: PathBuf,
    pub result: Result<ImportFolderReport, String>,
}

/// Start building `<download_dir>/lazer-import` unless a build is still
/// running. Returns `false` when one was already in flight.
pub fn schedule_lazer_import(
    handle: &mut Option<JoinHandle<()>>,
    download_dir: PathBuf,
    tx: &mpsc::UnboundedSender<LazerImportEvent>,
) -> bool {
    if handle.as_ref().is_some_and(|h| !h.is_finished()) {
        return false;
    }

    let tx = tx.clone();
    *handle = Some(tokio::spawn(async move {
        let out_dir = download_dir.join(IMPORT_FOLDER_NAME);
        let result = tokio::task::spawn_blocking({
            let out_dir = out_dir.clone();
            move || build_import_folder(&download_dir, &out_dir)
        })
        .await
        .map_err(|e| format!("task panicked: {e}"))
        .and_then(|r| r.map_err(|e| e.to_string()));
        let _ = tx.send(LazerImportEvent { out_dir, result });
    }));
    true
}

pub fn handle_lazer_import_event(event: LazerImportEvent, app: &mut App) {
    match event.result {
        Ok(report) => app.push_toast(lazer_import_toast(&report, &event.out_dir)),
        Err(err) => {
            warn!(out_dir = %event.out_dir.display(), error = %err, "lazer import folder build failed");
            app.toast_err(format!("lazer import folder failed: {err}"));
        }
    }
}

/// The result toast: what went in, and where to point lazer's import screen.
pub fn lazer_import_toast(report: &ImportFolderReport, out_dir: &Path) -> Toast {
    if report.runs == 0 {
        let download_dir = out_dir.parent().unwrap_or(out_dir);
        return Toast::warning("no downloads to import")
            .with_detail(format!("no download folders in {}", download_dir.display()));
    }

    let sets = report.unpacked + report.already_unpacked;
    let mut title = format!(
        "lazer import ready: {sets} sets, {} collections",
        report.collections
    );
    if !report.failed.is_empty() {
        title.push_str(&format!(" · {} skipped", report.failed.len()));
    }
    // The path leads: toasts wrap to two short lines and cut the tail, and the
    // folder is the one thing the user has to find again in lazer.
    Toast::success(title)
        .with_detail(format!(
            "{} · pick it in setup wizard → Import",
            out_dir.display()
        ))
        .until_dismissed()
}

#[cfg(test)]
#[path = "../../../tests/unit/runtime_lazer_import.rs"]
mod tests;
