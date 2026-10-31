use super::{ActiveDownloadRegistry, DownloadLockGuard};
use std::path::PathBuf;

#[test]
fn registry_insert_and_remove() {
    let registry = ActiveDownloadRegistry::new();
    let path = PathBuf::from("/tmp/test-download-registry");
    assert!(registry.try_insert(&path));
    assert!(!registry.try_insert(&path));
    registry.remove(&path);
    assert!(registry.try_insert(&path));
    registry.remove(&path);
}

#[test]
fn registry_independent_paths() {
    let registry = ActiveDownloadRegistry::new();
    let path_a = PathBuf::from("/tmp/registry-a");
    let path_b = PathBuf::from("/tmp/registry-b");
    assert!(registry.try_insert(&path_a));
    assert!(registry.try_insert(&path_b));
    registry.remove(&path_a);
    registry.remove(&path_b);
}

#[test]
fn registry_canonicalizes_paths() {
    let dir = tempfile::tempdir().unwrap();
    let direct = dir.path();
    let equivalent = dir.path().join(".");
    let registry = ActiveDownloadRegistry::new();

    assert!(registry.try_insert(direct));
    assert!(!registry.try_insert(&equivalent));
    registry.remove(&equivalent);
    assert!(registry.try_insert(direct));
}

#[test]
fn lock_file_is_not_created_in_output_directory() {
    let dir = tempfile::tempdir().unwrap();
    let registry = ActiveDownloadRegistry::new();
    let _guard = DownloadLockGuard::acquire(dir.path(), &registry).unwrap();

    assert!(!dir.path().join(".osu-shelf.lock").exists());
}
