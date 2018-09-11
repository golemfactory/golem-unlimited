use actix::prelude::*;
use gu_p2p::rpc::*;
use futures::Future;
use gu_actix::flatten::FlattenFuture;
use std::collections::HashSet;
use resolve_actor::ResolveActor;
use service::ServiceInstance;
use service::Service;
use serde_json;


pub const LAN_ENDPOINT : u32 = 576411;

/// Actix-web actor for mDNS service discovery
pub struct LanInfo();

impl Actor for LanInfo {
    type Context = RemotingContext<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        println!("started!");
        ctx.bind::<QueryLan>(QueryLan::ID);
        ctx.register();
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryLan {
    #[serde(default = "QueryLan::instances")]
    pub(crate) instances : Vec<String>,
    #[serde(default = "QueryLan::service")]
    pub(crate) service : String,
}

impl QueryLan {
    const ID : u32 = LAN_ENDPOINT;
    fn instances() -> Vec<String> {
        let mut vec = Vec::new();
        vec.push("gu-hub".to_string());
        vec.push("gu-provider".to_string());
        vec
    }
    fn service() -> String {
        "_unlimited._tcp".to_string()
    }

    fn first(&self) -> String {
        self.instances.first().unwrap_or(&"gu-provider".to_string()).to_owned()
    }

    pub fn single(s: Option<String>) -> Self {
        let vec = match s {
            Some(a) => vec![a],
            None => Self::instances(),
        };

        QueryLan {
            instances : vec,
            service : Self::service(),
        }
    }

    pub fn to_json(&self) -> String {
        let a = serde_json::to_string(self).expect("Deserialization error");
        println!("{:?}", a);
        a
    }
}

impl Message for QueryLan {
    type Result = Result<HashSet<ServiceInstance>, ()>;
}

impl Handler<QueryLan> for LanInfo {
    type Result = ActorResponse<LanInfo, HashSet<ServiceInstance>, ()>;

    fn handle(&mut self, msg: QueryLan, _ctx: &mut Self::Context)
              -> ActorResponse<LanInfo, HashSet<ServiceInstance>, ()> {
        info!("Handle lan query");

        ActorResponse::async({
            ResolveActor::from_registry().send(Service::new(msg.first(), "_unlimited._tcp"))
                .flatten_fut().map_err(|e| error!("err: {}", e))
                .into_actor(self)
        })
    }
}

