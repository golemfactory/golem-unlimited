use actix::prelude::*;
use futures::Future;
use gu_actix::flatten::FlattenFuture;
use gu_p2p::rpc::*;
use resolve_actor::ResolveActor;
use serde_json;
use service::ServiceDescription;
use service::ServiceInstance;
use service::ServicesDescription;
use std::collections::HashSet;

/// Actix-web actor for mDNS service discovery
pub struct LanInfo();

impl Actor for LanInfo {
    type Context = RemotingContext<Self>;

    fn started(&mut self, _ctx: &mut <Self as Actor>::Context) {
        debug!("started!");
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryLan {
    /// Eg. 'gu-provider' 'gu-hub'
    #[serde(default = "QueryLan::instances")]
    pub(crate) instances: Vec<String>,
    /// Eg. '_unlimited._tcp'
    #[serde(default = "QueryLan::service")]
    pub(crate) service: String,
}

impl QueryLan {
    fn instances() -> Vec<String> {
        let mut vec = Vec::new();
        vec.push("gu-hub".to_string());
        vec.push("gu-provider".to_string());
        vec
    }

    fn service() -> String {
        "_unlimited._tcp".to_string()
    }

    pub fn single(s: String) -> Self {
        QueryLan {
            instances: vec![s],
            service: Self::service(),
        }
    }

    pub fn new(vec: Vec<String>) -> Self {
        QueryLan {
            instances: vec,
            service: Self::service(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Deserialization error")
    }
}

impl Message for QueryLan {
    type Result = Result<HashSet<ServiceInstance>, ()>;
}

impl Handler<QueryLan> for LanInfo {
    type Result = ActorResponse<LanInfo, HashSet<ServiceInstance>, ()>;

    fn handle(
        &mut self,
        msg: QueryLan,
        _ctx: &mut Self::Context,
    ) -> ActorResponse<LanInfo, HashSet<ServiceInstance>, ()> {
        info!("Handle lan query");
        let mut vec = Vec::new();
        for instance in msg.instances {
            vec.push(ServiceDescription::new(instance, msg.service.clone()))
        }
        let services_desc = ServicesDescription::new(vec);

        ActorResponse::async({
            ResolveActor::from_registry()
                .send(services_desc)
                .flatten_fut()
                .map_err(|e| error!("err: {}", e))
                .into_actor(self)
        })
    }
}
