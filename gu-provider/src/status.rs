use actix::prelude::*;
use actix_web;
use actix_web::{App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse, Json, Responder};
use futures::{future, prelude::*};
use gu_base::Module;
use std::collections::BTreeMap;

pub fn module() -> impl Module {
    StatusModule
}

struct StatusModule;

impl Module for StatusModule {
    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        println!("status!");
        app.handler("/status", status_handler)
    }
}

#[derive(Serialize)]
struct StatusBody {
    envs: BTreeMap<String, EnvStatus>,
}

fn status_handler<S: 'static>(_r: &HttpRequest<S>) -> impl Responder {
    StatusManager::from_registry()
        .send(ListEnvStatus)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|envs| match envs {
            Ok(envs) => Ok(HttpResponse::Ok().json(StatusBody { envs })),
            Err(e) => Err(actix_web::error::ErrorInternalServerError(format!(
                "err: {}",
                e
            ))),
        }).responder()
}

#[derive(Serialize, Deserialize)]
pub enum EnvStatus {
    Ready,
    Working,
    Paused,
    Disabled,
}

pub struct GetEnvStatus;

impl Message for GetEnvStatus {
    type Result = EnvStatus;
}

// status manager:

struct ListEnvStatus;

impl Message for ListEnvStatus {
    type Result = Result<BTreeMap<String, EnvStatus>, String>;
}

#[derive(Message)]
pub struct AddProvider(&'static str, Recipient<GetEnvStatus>);

impl AddProvider {
    #[inline]
    pub fn new(name: &'static str, handler: Recipient<GetEnvStatus>) -> AddProvider {
        AddProvider(name, handler)
    }
}

#[derive(Default)]
pub struct StatusManager {
    providers: BTreeMap<&'static str, Recipient<GetEnvStatus>>,
}

impl Actor for StatusManager {
    type Context = Context<Self>;
}

impl Handler<AddProvider> for StatusManager {
    type Result = ();

    fn handle(
        &mut self,
        msg: AddProvider,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<AddProvider>>::Result {
        self.providers.insert(msg.0, msg.1);
    }
}

impl Handler<ListEnvStatus> for StatusManager {
    type Result = ActorResponse<StatusManager, BTreeMap<String, EnvStatus>, String>;

    fn handle(&mut self, msg: ListEnvStatus, ctx: &mut Self::Context) -> Self::Result {
        ActorResponse::async(
            future::join_all(self.providers.clone().into_iter().map(
                move |(env_name, env_addr)| {
                    let name = env_name.to_string();
                    env_addr.send(GetEnvStatus).and_then(move |s| Ok((name, s)))
                },
            )).and_then(
                |envs| -> Result<BTreeMap<String, EnvStatus>, actix::MailboxError> {
                    Ok(envs.into_iter().collect())
                },
            ).map_err(|e| format!("{}", e))
            .into_actor(self),
        )
    }
}

impl Supervised for StatusManager {}
impl SystemService for StatusManager {}
