use super::{LibraryCacheFile, db_file_path, owned_ids_cached_with};
use crate::osu_db::OsuClient;
use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, UNIX_EPOCH},
};

fn set_mtime(path: &Path, secs: u64) {
    let when = UNIX_EPOCH + Duration::from_secs(secs);
    let file = fs::File::options()
        .write(true)
        .open(path)
        .expect("open db file");
    file.set_modified(when).expect("set mtime");
}

#[test]
fn db_file_path_picks_client_specific_file() {
    let dir = Path::new("/osu");
    assert!(db_file_path(OsuClient::Stable, dir).ends_with("osu!.db"));
    assert!(db_file_path(OsuClient::Lazer, dir).ends_with("client.realm"));
}

#[test]
fn cache_reads_once_then_serves_until_mtime_changes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("client.realm");
    fs::write(&db, b"db").unwrap();
    let cache = dir.path().join("library-cache.json");

    let calls = AtomicUsize::new(0);

    set_mtime(&db, 1_000);
    let ids = owned_ids_cached_with(&db, &cache, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(HashSet::from([1, 2, 3]))
    })
    .unwrap();
    assert_eq!(ids, HashSet::from([1, 2, 3]));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // mtime unchanged → cache hit; the reader must not run again, and the
    // cached ids (not the reader's) are returned.
    let ids = owned_ids_cached_with(&db, &cache, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(HashSet::from([9]))
    })
    .unwrap();
    assert_eq!(ids, HashSet::from([1, 2, 3]));
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // mtime bumped → cache invalidated; reader runs and its ids win.
    set_mtime(&db, 2_000);
    let ids = owned_ids_cached_with(&db, &cache, || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(HashSet::from([4, 5]))
    })
    .unwrap();
    assert_eq!(ids, HashSet::from([4, 5]));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn cache_missing_db_file_errors_without_reading() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("absent.realm");
    let cache = dir.path().join("library-cache.json");

    let result = owned_ids_cached_with(&db, &cache, || {
        panic!("reader must not run when the db file cannot be stat-ed");
    });

    assert!(result.is_err());
}

#[test]
fn cache_from_an_older_schema_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("osu!.db");
    fs::write(&db, b"db").unwrap();
    set_mtime(&db, 1_000);
    let cache = dir.path().join("library-cache.json");
    // What osu-db 0.3.0 left behind for a current osu!.db: a matching path and
    // mtime, but an empty library.
    let stale = LibraryCacheFile {
        schema_version: 1,
        db_path: db.to_string_lossy().into_owned(),
        mtime_ns: Duration::from_secs(1_000).as_nanos(),
        beatmapset_ids: Vec::new(),
    };
    fs::write(&cache, serde_json::to_string(&stale).unwrap()).unwrap();

    let ids = owned_ids_cached_with(&db, &cache, || Ok(HashSet::from([7]))).unwrap();
    assert_eq!(ids, HashSet::from([7]));

    // The fresh read replaced the stale entry.
    let served = owned_ids_cached_with(&db, &cache, || panic!("cache hit expected")).unwrap();
    assert_eq!(served, HashSet::from([7]));
}
