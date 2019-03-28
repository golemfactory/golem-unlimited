#![allow(dead_code)]

use crate::error::{Error, Result};
use actix::Message;
use nix::sys::statvfs::statvfs;
use std::path::{Path, PathBuf};

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

pub(crate) fn storage_info<T: AsRef<Path>>(path: T) -> Result<StorageInfo> {
    let stat = statvfs(path.as_ref()).map_err(|e| Error::Nix(e))?;

    Ok(StorageInfo {
        id: stat.filesystem_id(),
        available: stat.blocks_available() * stat.block_size(),
        total: stat.blocks() * stat.block_size(),
    })
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
