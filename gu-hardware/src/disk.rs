use sysinfo::{SystemExt, DiskExt, DiskType};
use actix::Message;
use error::{Result, ErrorKind};
use std::path::PathBuf;

#[derive(Debug)]
pub struct DiskInfo {
    available: u64,
    total: u64,
    disk_type: DiskType,
}

impl DiskInfo {
    pub fn available(&self) -> u64 {
        self.available
    }
}

fn disk_for_path(disks: &[impl DiskExt], path: PathBuf) -> Result<&impl DiskExt> {
    let path = path.canonicalize()?;
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

    best_match.ok_or_else(|| ErrorKind::PathMountpointNotFound(path).into())
}

pub(crate) fn disk_info(sys: &impl SystemExt, path: PathBuf) -> Result<DiskInfo>
{
    let disk = disk_for_path(sys.get_disks(), path)?;
    Ok(DiskInfo {
        available: disk.get_available_space(),
        total: disk.get_total_space(),
        disk_type: disk.get_type(),
    })
}

#[derive(Debug)]
pub struct DiskQuery {
    path: PathBuf,
}

impl DiskQuery {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path
        }
    }

    pub fn path(self) -> PathBuf {
        self.path
    }
}

impl Message for DiskQuery {
    type Result = Result<DiskInfo>;
}