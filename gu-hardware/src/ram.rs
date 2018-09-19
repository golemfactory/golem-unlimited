use actix::Message;
use error::Result;
use sysinfo::SystemExt;

#[derive(Debug, Serialize, Deserialize)]
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

pub(crate) fn ram_info(sys: &impl SystemExt) -> RamInfo {
    RamInfo {
        free: sys.get_free_memory(),
        used: sys.get_used_memory(),
        total: sys.get_total_memory(),
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RamQuery;

impl Message for RamQuery {
    type Result = Result<RamInfo>;
}
