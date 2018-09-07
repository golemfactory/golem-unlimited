use actix::prelude::*;
use gu_p2p::rpc::*;
use futures::Future;
use gu_actix::flatten::FlattenFuture;
use std::collections::HashSet;
use resolve_actor::ResolveActor;
use service::ServiceInstance;
use service::Service;


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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryLan {
    #[serde(default = "QueryLan::instances")]
    instances : Vec<String>,
    #[serde(default = "QueryLan::service")]
    service : String,
}

impl QueryLan {
    const ID : u32 = 576411;
    fn instances() -> Vec<String> {
        let mut vec = Vec::new();
        vec.push("gu-hub".to_string());
        vec.push("gu-provider".to_string());
        vec
    }
    fn service() -> String {
        "_unlimited._tcp".to_string()
    }
}

impl Message for QueryLan {
    type Result = Result<HashSet<ServiceInstance>, ()>;
}

impl Handler<QueryLan> for LanInfo {
    type Result = ActorResponse<LanInfo, HashSet<ServiceInstance>, ()>;

    fn handle(&mut self, _msg: QueryLan, _ctx: &mut Self::Context)
              -> ActorResponse<LanInfo, HashSet<ServiceInstance>, ()> {
        info!("Handle lan query");

        ActorResponse::async(
            ResolveActor::from_registry().send(Service::new("gu-hub", "_unlimited._tcp"))
                .flatten_fut().map_err(|e| error!("err: {}", e))
                .into_actor(self))
    }
}

