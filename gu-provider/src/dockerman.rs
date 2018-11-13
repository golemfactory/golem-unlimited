//! Docker mode implementation

use super::envman;
use actix::prelude::*;
use async_docker::{new_docker, ContainerOptions, DockerApi};
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_envman_api::*;
use gu_net::rpc::peer::PeerSessionInfo;
use std::collections::BTreeMap;

// Actor.
#[derive(Default)]
struct DockerMan {
    docker_api: Option<Box<DockerApi>>,
    sessions: BTreeMap<String, DockerSessionInfo>,
}

struct DockerSessionInfo {}

impl Actor for DockerMan {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        match new_docker(None) {
            Ok(docker_api) => {
                self.docker_api = Some(docker_api);
                envman::register("docker", ctx.address())
            }
            Err(e) => {
                error!("docker start failed: {}", e);
                ctx.stop()
            }
        }
    }
}

impl envman::EnvManService for DockerMan {}

impl Handler<CreateSession> for DockerMan {
    type Result = ActorResponse<DockerMan, String, Error>;

    fn handle(
        &mut self,
        msg: CreateSession,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<CreateSession>>::Result {
        match self.docker_api {
            Some(ref api) => {
                let Image { url, hash } = msg.image;

                //let docker_image = api.image(url.as_ref().into());
                let opts = ContainerOptions::builder(url.as_ref())
                    .name(msg.name.as_ref())
                    .build();
                ActorResponse::async(
                    api.containers()
                        .create(&opts)
                        .and_then(|c| Ok(c.Id))
                        .map_err(|e| Error::IoError(format!("{}", e)))
                        .into_actor(self),
                )
            }
            None => ActorResponse::reply(Err(Error::UnknownEnv(msg.env_type))),
        }
    }
}

impl Handler<SessionUpdate> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<String>, Vec<String>>;

    fn handle(
        &mut self,
        msg: SessionUpdate,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<SessionUpdate>>::Result {
        unimplemented!()
    }
}

impl Handler<GetSessions> for DockerMan {
    type Result = ActorResponse<DockerMan, Vec<PeerSessionInfo>, ()>;

    fn handle(
        &mut self,
        msg: GetSessions,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<GetSessions>>::Result {
        ActorResponse::reply(Ok(Vec::new()))
    }
}

impl Handler<DestroySession> for DockerMan {
    type Result = ActorResponse<DockerMan, String, Error>;

    fn handle(
        &mut self,
        msg: DestroySession,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<DestroySession>>::Result {
        unimplemented!()
    }
}
