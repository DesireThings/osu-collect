use crate::{
    config::constants::{APP_SUBDIR, DIRECTORY_LOCK_FILE},
    download::error::DownloadError,
};
use dashmap::DashSet;
use fs2::FileExt;
use std::{
    collections::hash_map::DefaultHasher,
    fs::{File as StdFile, OpenOptions},
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tracing::warn;

#[derive(Clone)]
pub struct ActiveDownloadRegistry {
    active: Arc<DashSet<PathBuf>>,
}

impl ActiveDownloadRegistry {
    pub fn new() -> Self {
        Self {
            active: Arc::new(DashSet::new()),
        }
    }

    pub fn try_insert(&self, path: &Path) -> bool {
        self.active.insert(lock_key(path))
    }

    pub fn remove(&self, path: &Path) {
        self.active.remove(&lock_key(path));
    }
}

impl Default for ActiveDownloadRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn lock_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn lock_file_path(path: &Path) -> Result<PathBuf, DownloadError> {
    let key = lock_key(path);
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let file_name = format!("{DIRECTORY_LOCK_FILE}-{:016x}", hasher.finish());
    let lock_dir = std::env::temp_dir().join(APP_SUBDIR);
    std::fs::create_dir_all(&lock_dir)?;
    Ok(lock_dir.join(file_name))
}

pub struct DownloadLockGuard {
    path: PathBuf,
    file: Option<StdFile>,
    registry: ActiveDownloadRegistry,
}

impl DownloadLockGuard {
    pub fn acquire(path: &Path, registry: &ActiveDownloadRegistry) -> Result<Self, DownloadError> {
        if !registry.try_insert(path) {
            return Err(DownloadError::ConcurrentDownload(
                path.to_string_lossy().into_owned(),
            ));
        }

        match Self::lock_directory(path) {
            Ok(file) => Ok(Self {
                path: path.to_path_buf(),
                file: Some(file),
                registry: registry.clone(),
            }),
            Err(err) => {
                registry.remove(path);
                Err(err)
            }
        }
    }

    fn lock_directory(path: &Path) -> Result<StdFile, DownloadError> {
        let lock_file_path = lock_file_path(path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_file_path)
            .or_else(|err| {
                if err.kind() == io::ErrorKind::AlreadyExists {
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&lock_file_path)
                } else {
                    Err(err)
                }
            })
            .map_err(DownloadError::from)?;

        if let Err(err) = file.try_lock_exclusive() {
            let kind = err.kind();
            drop(file);
            if kind == io::ErrorKind::WouldBlock {
                return Err(DownloadError::ConcurrentDownload(
                    path.to_string_lossy().into_owned(),
                ));
            }
            return Err(DownloadError::Io(err));
        }

        Ok(file)
    }
}

impl Drop for DownloadLockGuard {
    fn drop(&mut self) {
        if let Some(file) = self.file.take()
            && let Err(err) = file.unlock()
        {
            warn!(
                directory = %self.path.display(),
                error = %err,
                "Failed to release directory lock"
            );
        }

        self.registry.remove(&self.path);
    }
}

#[cfg(test)]
#[path = "../../tests/unit/lock.rs"]
mod tests;
