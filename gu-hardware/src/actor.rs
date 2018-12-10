use actix::{Actor, ActorResponse, Addr, ArbiterService, Handler, Message, WrapFuture};
use futures::Future;
use hostname::get_hostname;

use disk::{DiskInfo, DiskQuery};
use gu_actix::flatten::FlattenFuture;
use gu_net::rpc::{RemotingContext, RemotingSystemService};
use inner_actor::InnerActor;
use ram::{RamInfo, RamQuery};

use super::gpuinfo::{gpu_count, GpuCount};
use num_cpus;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct HardwareQuery;

impl HardwareQuery {
    const ID: u32 = 19354;
}

#[derive(Debug, Serialize, Deserialize)]
enum OsType {
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

        ActorResponse::async(
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
