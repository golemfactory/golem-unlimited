use actix::prelude::*;
use actix_web::Json;
use gu_base::Module;
use gu_model::{BuildInfo, HubInfo, Map, Version};
use gu_net::NodeId;
use std::cell::{Ref, RefCell};
use std::sync::{Arc, RwLock};

pub struct InfoModule {
    ref_node_id: RwLock<Option<NodeId>>,
}

fn build_info() -> BuildInfo {
    BuildInfo {
        ts: env!("VERGEN_BUILD_TIMESTAMP").parse().unwrap(),
        target: env!("VERGEN_TARGET_TRIPLE").to_string(),
        commit_hash: env!("VERGEN_SHA").to_string(),
    }
}

impl InfoModule {
    pub fn set_node_id(&self, node_id: NodeId) {
        self.ref_node_id.write().unwrap().replace(node_id);
    }

    fn create_info(&self) -> HubInfo {
        let node_id = { self.ref_node_id.read().unwrap().clone().unwrap() };

        let hub_info = HubInfo {
            node_id,
            version: env!("VERGEN_SEMVER").parse().unwrap(),
            build: build_info(),
            caps: Map::default(),
        };

        return hub_info;
    }
}

impl Module for InfoModule {
    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        let info = self.create_info();

        app.resource("/info", move |r| {
            r.get().with(move |_: ()| Json(info.clone()))
        })
    }
}

pub fn module() -> impl Module {
    InfoModule {
        ref_node_id: RwLock::new(None),
    }
}
