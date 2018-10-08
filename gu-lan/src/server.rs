use actix::prelude::*;
use actor::{MdnsActor, OneShot};
use futures::Future;
use gu_actix::flatten::FlattenFuture;
use gu_p2p::rpc::*;
use serde_json;
use service::{ServiceDescription, ServiceInstance, ServicesDescription};
use std::collections::HashSet;

/// Actix-web actor for mDNS service discovery
pub struct LanServer;

impl Actor for LanServer {
    type Context = RemotingContext<Self>;

    fn started(&mut self, _ctx: &mut <Self as Actor>::Context) {
        debug!("started!");
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LanQuery {
    /// Eg. 'gu-provider' 'gu-hub'
    #[serde(default = "LanQuery::instances")]
    pub(crate) instances: Vec<String>,
    /// Eg. '_unlimited._tcp'
    #[serde(default = "LanQuery::service")]
    pub(crate) service: String,
}

impl LanQuery {
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
        LanQuery {
            instances: vec![s],
            service: Self::service(),
        }
    }

    pub fn new(vec: Vec<String>) -> Self {
        LanQuery {
            instances: vec,
            service: Self::service(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Deserialization error")
    }
}

impl Message for LanQuery {
    type Result = Result<HashSet<ServiceInstance>, ()>;
}

impl Handler<LanQuery> for LanServer {
    type Result = ActorResponse<LanServer, HashSet<ServiceInstance>, ()>;

    fn handle(
        &mut self,
        msg: LanQuery,
        _ctx: &mut Self::Context,
    ) -> ActorResponse<LanServer, HashSet<ServiceInstance>, ()> {
        info!("Handle lan query");
        let mut vec = Vec::new();
        for instance in msg.instances {
            vec.push(ServiceDescription::new(instance, msg.service.clone()))
        }
        let services_desc = ServicesDescription::new(vec);

        ActorResponse::async({
            MdnsActor::<OneShot>::from_registry()
                .send(services_desc)
                .flatten_fut()
                .map_err(|e| error!("err: {}", e))
                .into_actor(self)
        })
    }
}
