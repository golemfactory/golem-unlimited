#![allow(dead_code)]

use clap::{App, ArgMatches, SubCommand};
use gu_actix::*;
use std::{borrow::Cow, net::ToSocketAddrs, sync::Arc};

use actix::{
    self, fut, Actor, ActorContext, ActorFuture, Addr, AsyncContext, Context, SystemService,
    WrapFuture,
};
use actix_web;
use futures::Future;
use gu_base::{Decorator, Module};
use gu_ethkey::prelude::*;
use gu_net::{
    rpc::{self, mock},
    NodeId,
};
use gu_persist::{
    config::{self, ConfigManager, ConfigModule},
    daemon_module::DaemonModule,
    http::{ServerClient, ServerConfig},
};
use mdns::{Responder, Service};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HubConfig {
    #[serde(default = "HubConfig::default_p2p_port")]
    pub(crate) p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(default = "HubConfig::publish_service")]
    pub(crate) publish_service: bool,
}

pub(crate) type HubClient = ServerClient<HubConfig>;

impl Default for HubConfig {
    fn default() -> Self {
        HubConfig {
            p2p_port: Self::default_p2p_port(),
            control_socket: None,
            publish_service: Self::publish_service(),
        }
    }
}

impl ServerConfig for HubConfig {
    fn port(&self) -> u16 {
        self.p2p_port
    }
}

impl HubConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    fn default_p2p_port() -> u16 {
        61622
    }

    fn publish_service() -> bool {
        true
    }
}

impl config::HasSectionId for HubConfig {
    const SECTION_ID: &'static str = "server-cfg";
}

pub struct ServerModule {
    active: bool,
    config_path: Option<String>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            active: false,
            config_path: None,
        }
    }
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("server").about("Hub server management"))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        let config_path = match matches.value_of("config-dir") {
            Some(v) => Some(v.to_string()),
            None => None,
        };

        if let Some(_m) = matches.subcommand_matches("server") {
            self.active = true;
            self.config_path = config_path.to_owned();
            return true;
        }
        false
    }

    fn run<D: Decorator + 'static + Sync + Send>(&self, decorator: D) {
        let daemon: &DaemonModule = decorator.extract().unwrap();

        if !daemon.run() {
            return;
        }

        let sys = actix::System::new("gu-hub");

        let _config = ServerConfigurer::new(decorator.clone(), self.config_path.clone()).start();

        let _ = sys.run();
    }
}

fn mdns_publisher(port: u16, node_id: NodeId) -> Service {
    let responder = Responder::new().expect("Failed to run mDNS publisher");
    let node_txt_record = format!("node_id={:?}", node_id);

    responder.register(
        "_unlimited._tcp".to_owned(),
        "gu-hub".to_owned(),
        port,
        &[node_txt_record.as_ref()],
    )
}

fn chat_route(
    req: &actix_web::HttpRequest<NodeId>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    rpc::ws::route(req, req.state().clone())
}

pub(crate) struct ServerConfigurer<D: Decorator> {
    decorator: D,
    path: Option<String>,
}

impl<D: Decorator + 'static + Sync + Send> ServerConfigurer<D> {
    fn new(decorator: D, path: Option<String>) -> Self {
        Self { decorator, path }
    }

    pub fn config(&self) -> Addr<ConfigManager> {
        let config = config::ConfigManager::from_registry();
        println!("path={:?}", &self.path);

        if let Some(path) = &self.path {
            config.do_send(config::SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }
        config
    }

    fn hub_configuration(&mut self, c: Arc<HubConfig>) -> Result<(), ()> {
        let config_module: &ConfigModule = self.decorator.extract().unwrap();
        let key = SafeEthKey::load_or_generate(config_module.keystore_path(), &"".into())
            .expect("should load or generate eth key");

        let decorator = self.decorator.clone();
        let node_id = NodeId::from(key.address().as_ref());
        let server = actix_web::server::new(move || {
            decorator.decorate_webapp(
                actix_web::App::with_state(node_id)
                    .handler(
                        "/app",
                        actix_web::fs::StaticFiles::new("webapp")
                            .expect("cannot provide static files"),
                    ).scope("/m", mock::scope)
                    .resource("/ws/", |r| r.route().f(chat_route)),
            )
        });
        let _ = server.bind(c.p2p_addr()).unwrap().start();

        if c.publish_service {
            Box::leak(Box::new(mdns_publisher(c.p2p_port, node_id)));
        }

        Ok(())
    }
}

impl<D: Decorator + 'static> Actor for ServerConfigurer<D> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.spawn(
            self.config()
                .send(config::GetConfig::new())
                .flatten_fut()
                .map_err(|e| error!("error ! {}", e))
                .into_actor(self)
                .and_then(move |config, act, ctx| {
                    let _ = act
                        .hub_configuration(config)
                        .map_err(|e| error!("Hub configuration error {:?}", e));
                    fut::ok(ctx.stop())
                }),
        );
    }
}

impl<D: Decorator> Drop for ServerConfigurer<D> {
    fn drop(&mut self) {
        info!("hub server configured")
    }
}
