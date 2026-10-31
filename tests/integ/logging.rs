use osu_collect::config::{LogFormat, LogLevel, LoggingConfig};
use osu_collect::utils::logging::init_logging;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

fn unique_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "osu_collect_logging_test_{}_{}",
        std::process::id(),
        nanos
    ))
}

#[tokio::test]
async fn pretty_logging_writes_human_readable_lines() {
    let dir = unique_dir();
    let config = LoggingConfig {
        enabled: true,
        level: LogLevel::Info,
        format: LogFormat::Pretty,
        file_dir: Some(dir.to_string_lossy().into()),
    };

    let guard = init_logging(&config)
        .expect("init logging")
        .expect("logging guard");
    tracing::info!("pretty-log-test-entry");
    drop(guard);

    let mut entries = fs::read_dir(&dir).await.expect("log dir exists");
    let mut log_path: Option<PathBuf> = None;
    while let Some(entry) = entries.next_entry().await.expect("read dir entry") {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("osu-shelf.log") {
            log_path = Some(entry.path());
            break;
        }
    }

    let log_path = log_path.expect("log file created");
    let contents = fs::read_to_string(&log_path)
        .await
        .expect("log file exists");
    assert!(contents.contains("pretty-log-test-entry"));
    assert!(!contents.trim_start().starts_with('{'));

    let _ = tokio::fs::remove_dir_all(&dir).await;
}
