use actix::{Actor, ActorResponse, Addr, ArbiterService, Handler, Message, WrapFuture};
use futures::Future;
use hostname::get_hostname;
use num_cpus;
use serde::{Deserialize, Serialize};

use gu_actix::flatten::FlattenFuture;
use gu_net::rpc::{PublicMessage, RemotingContext, RemotingSystemService};

pub use crate::disk::{DiskInfo, DiskQuery};
use crate::inner_actor::InnerActor;
pub use crate::ram::{RamInfo, RamQuery};
use crate::storage::storage_info;
pub use crate::storage::{StorageInfo, StorageQuery};

use super::gpuinfo::{gpu_count, GpuCount};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HardwareQuery;

impl PublicMessage for HardwareQuery {
    const ID: u32 = 19354;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OsType {
    Windows,
    MacOs,
    Linux,
}

fn os_type() -> Option<OsType> {
    if cfg!(target_os = "windows") {
        return Some(OsType::Windows);
    } else if cfg!(target_os = "macos") {
        return Some(OsType::MacOs);
    } else if cfg!(target_os = "linux") {
        return Some(OsType::Linux);
    }
    None
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Hardware {
    #[serde(skip_serializing_if = "Option::is_none")]
    gpu: Option<GpuCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ram: Option<RamInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disk: Option<DiskInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    os: Option<OsType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hostname: Option<String>,
    num_cores: usize,
}

impl Hardware {
    pub fn num_cores(&self) -> usize {
        self.num_cores
    }

    pub fn os(&self) -> Option<&OsType> {
        self.os.as_ref()
    }
}

impl Message for HardwareQuery {
    type Result = Result<Hardware, String>;
}

#[derive(Debug, Default)]
pub struct HardwareActor {
    gpu_count: Option<GpuCount>,
    hostname: Option<String>,
}

impl Actor for HardwareActor {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.bind::<HardwareQuery>(HardwareQuery::ID);

        self.gpu_count = gpu_count()
            .or_else(|e| Err(error!("gpu detection: {}", e)))
            .ok();
        self.hostname = get_hostname()
    }
}

impl RemotingSystemService for HardwareActor {}

fn ram(
    query: RamQuery,
    inner: &Addr<InnerActor>,
) -> impl Future<Item = Option<RamInfo>, Error = String> {
    inner
        .send(query)
        .flatten_fut()
        .map_err(|e| format!("{}", e))
        .and_then(|r| Ok(Some(r)))
}

fn disk(
    query: DiskQuery,
    inner: &Addr<InnerActor>,
) -> impl Future<Item = Option<DiskInfo>, Error = String> {
    inner
        .send(query)
        .flatten_fut()
        .map_err(|e| format!("{}", e))
        .then(|r| Ok(r.ok()))
}

impl Handler<HardwareQuery> for HardwareActor {
    type Result = ActorResponse<Self, Hardware, String>;

    fn handle(
        &mut self,
        _msg: HardwareQuery,
        _ctx: &mut RemotingContext<Self>,
    ) -> <Self as Handler<HardwareQuery>>::Result {
        let inner = InnerActor::from_registry();
        let gpu = self.gpu_count.clone();
        let hostname = self.hostname.clone();

        ActorResponse::r#async(
            ram(RamQuery::default(), &inner)
                .join(disk(DiskQuery::new(), &inner))
                .and_then(move |(ram, disk)| {
                    Ok(Hardware {
                        gpu,
                        ram,
                        disk,
                        os: os_type(),
                        hostname,
                        num_cores: num_cpus::get_physical(),
                    })
                })
                .into_actor(self),
        )
    }
}

impl Handler<StorageQuery> for HardwareActor {
    type Result = ActorResponse<Self, StorageInfo, String>;

    fn handle(
        &mut self,
        msg: StorageQuery,
        _ctx: &mut RemotingContext<Self>,
    ) -> <Self as Handler<StorageQuery>>::Result {
        ActorResponse::reply(storage_info(msg.path()).map_err(|e| e.to_string()))
    }
}
