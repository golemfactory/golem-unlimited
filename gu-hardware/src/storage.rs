use std::path::{Path, PathBuf};

use actix::Message;
#[cfg(unix)]
use nix::sys::statvfs::statvfs;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    id: u64,
    available: u64,
    total: u64,
}

impl StorageInfo {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn available(&self) -> u64 {
        self.available
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

#[cfg(unix)]
pub(crate) fn storage_info<T: AsRef<Path>>(path: T) -> Result<StorageInfo> {
    let stat = statvfs(path.as_ref()).map_err(|e| Error::Nix(e))?;

    Ok(StorageInfo {
        id: stat.filesystem_id() as u64,
        available: stat.blocks_available() as u64 * stat.block_size() as u64,
        total: stat.blocks() as u64 * stat.block_size() as u64,
    })
}

#[cfg(not(unix))]
pub(crate) fn storage_info<T: AsRef<Path>>(path: T) -> Result<StorageInfo> {
    Err(Error::StorageNotSupported)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageQuery {
    path: PathBuf,
}

impl StorageQuery {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(self) -> PathBuf {
        self.path
    }
}

impl Message for StorageQuery {
    type Result = std::result::Result<StorageInfo, String>;
}
