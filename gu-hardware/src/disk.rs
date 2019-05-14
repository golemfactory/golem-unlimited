use std::path::PathBuf;

use actix::Message;
use serde::{Deserialize, Serialize};
use sysinfo::{DiskExt, DiskType, SystemExt};

use gu_persist::config::ConfigModule;

use crate::error::{Error, Result};

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskInfo {
    available: u64,
    total: u64,
    #[serde(with = "DiskTypeDef")]
    disk_type: DiskType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "DiskType")]
enum DiskTypeDef {
    /// HDD type.
    HDD,
    /// SSD type.
    SSD,
    /// Unknown type.
    Unknown(isize),
}

impl DiskInfo {
    pub fn available(&self) -> u64 {
        self.available
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn disk_type(&self) -> DiskType {
        self.disk_type
    }
}

fn disk_for_path(disks: &[impl DiskExt], path: PathBuf) -> Result<&impl DiskExt> {
    let path = path.canonicalize().map_err(|e| Error::Io(e))?;
    let mut best_match = None;
    let mut best_len = 0;

    for disk in disks {
        let mount_point = disk.get_mount_point();
        if path.starts_with(mount_point) {
            let len = mount_point.to_str().unwrap_or("").len();

            if len > best_len {
                best_len = len;
                best_match = Some(disk);
            }
        }
    }

    best_match.ok_or_else(|| Error::PathMountpointNotFound(path))
}

pub(crate) fn disk_info(sys: &impl SystemExt, path: PathBuf) -> Result<DiskInfo> {
    let disk = disk_for_path(sys.get_disks(), path)?;
    Ok(DiskInfo {
        available: disk.get_available_space(),
        total: disk.get_total_space(),
        disk_type: disk.get_type(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskQuery {
    path: PathBuf,
}

impl DiskQuery {
    pub fn new() -> Self {
        Self {
            path: ConfigModule::new().work_dir().to_path_buf(),
        }
    }

    pub fn path(self) -> PathBuf {
        self.path
    }
}

impl Message for DiskQuery {
    type Result = Result<DiskInfo>;
}
