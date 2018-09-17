use sysinfo::{self, SystemExt};
use actix::Message;
use error::Result;

#[derive(Debug)]
pub struct RamInfo {
    free: u64,
    used: u64,
    total: u64,
}

impl RamInfo {
    pub fn free(&self) -> u64 {
        self.free
    }

    pub fn used(&self) -> u64 {
        self.used
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

pub(crate) fn ram_info() -> RamInfo {
    let system = sysinfo::System::new();

    RamInfo {
        free: system.get_free_memory(),
        used: system.get_used_memory(),
        total: system.get_total_memory(),
    }
}

#[derive(Debug, Default)]
pub struct RamQuery {}

impl RamQuery {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Message for RamQuery {
    type Result = Result<RamInfo>;
}