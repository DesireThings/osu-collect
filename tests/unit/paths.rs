use super::{MARKER_FILE, copy_legacy_dir, env_override};
use crate::config::constants::{APP_SUBDIR, LEGACY_APP_SUBDIR};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
fn env_override_prefers_the_new_name() {
    let _env = crate::test_env::TempEnvVar::set_all([
        ("OSU_SHELF_TEST_PATHS_NEW", "/new"),
        ("OSU_SHELF_TEST_PATHS_OLD", "/old"),
    ]);

    assert_eq!(
        env_override("OSU_SHELF_TEST_PATHS_NEW", "OSU_SHELF_TEST_PATHS_OLD"),
        Some(PathBuf::from("/new"))
    );
}

#[test]
fn env_override_still_reads_the_legacy_name() {
    let _env = crate::test_env::TempEnvVar::set("OSU_SHELF_TEST_PATHS_LEGACY_ONLY", "/old");

    assert_eq!(
        env_override(
            "OSU_SHELF_TEST_PATHS_ABSENT",
            "OSU_SHELF_TEST_PATHS_LEGACY_ONLY"
        ),
        Some(PathBuf::from("/old"))
    );
}

#[test]
fn env_override_ignores_empty_values() {
    let _env = crate::test_env::TempEnvVar::set_all([
        ("OSU_SHELF_TEST_PATHS_BLANK", "   "),
        ("OSU_SHELF_TEST_PATHS_FALLBACK", "/fallback"),
    ]);

    assert_eq!(
        env_override(
            "OSU_SHELF_TEST_PATHS_BLANK",
            "OSU_SHELF_TEST_PATHS_FALLBACK"
        ),
        Some(PathBuf::from("/fallback"))
    );
}

#[test]
fn copy_legacy_dir_copies_nested_files_and_leaves_the_source() {
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join(LEGACY_APP_SUBDIR);
    write(&src.join("config.toml"), "config");
    write(&src.join("snapshots/collection-1.json"), "snapshot");

    copy_legacy_dir(root.path(), LEGACY_APP_SUBDIR, APP_SUBDIR);

    let dst = root.path().join(APP_SUBDIR);
    assert_eq!(
        fs::read_to_string(dst.join("config.toml")).unwrap(),
        "config"
    );
    assert_eq!(
        fs::read_to_string(dst.join("snapshots/collection-1.json")).unwrap(),
        "snapshot"
    );
    assert!(src.join("config.toml").exists());
    assert!(src.join(MARKER_FILE).exists());
}

#[test]
fn copy_legacy_dir_never_overwrites_newer_files() {
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join(LEGACY_APP_SUBDIR);
    let dst = root.path().join(APP_SUBDIR);
    write(&src.join("config.toml"), "old");
    write(&dst.join("config.toml"), "new");

    copy_legacy_dir(root.path(), LEGACY_APP_SUBDIR, APP_SUBDIR);

    assert_eq!(fs::read_to_string(dst.join("config.toml")).unwrap(), "new");
}

#[test]
fn marker_stops_a_second_copy_after_the_new_folder_is_removed() {
    let root = tempfile::tempdir().unwrap();
    let src = root.path().join(LEGACY_APP_SUBDIR);
    write(&src.join("config.toml"), "config");

    copy_legacy_dir(root.path(), LEGACY_APP_SUBDIR, APP_SUBDIR);
    fs::remove_dir_all(root.path().join(APP_SUBDIR)).unwrap();
    copy_legacy_dir(root.path(), LEGACY_APP_SUBDIR, APP_SUBDIR);

    assert!(!root.path().join(APP_SUBDIR).exists());
}

#[test]
fn copy_legacy_dir_is_a_noop_without_a_source() {
    let root = tempfile::tempdir().unwrap();

    copy_legacy_dir(root.path(), LEGACY_APP_SUBDIR, APP_SUBDIR);

    assert!(!root.path().join(APP_SUBDIR).exists());
    assert!(!root.path().join(LEGACY_APP_SUBDIR).exists());
}
