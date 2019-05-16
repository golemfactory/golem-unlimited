use std::{borrow::Cow, net::ToSocketAddrs, sync::Arc};

use actix::prelude::*;
use actix_web;
use clap::{App, ArgMatches};
use futures::Future;
use log::error;
use serde::{Deserialize, Serialize};

use ethkey::prelude::*;
use gu_actix::*;
use gu_base::{
    daemon_lib::{DaemonCommand, DaemonHandler},
    Decorator, Module,
};
use gu_lan::MdnsPublisher;
use gu_net::{
    rpc::{self, mock},
    NodeId,
};
use gu_persist::{
    config::{self, ConfigManager, ConfigModule},
    http::{ServerClient, ServerConfig},
};

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
    daemon_command: DaemonCommand,
    config_path: Option<String>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            daemon_command: DaemonCommand::None,
            config_path: None,
        }
    }
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(DaemonHandler::subcommand())
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        self.config_path = match matches.value_of("config-dir") {
            Some(v) => Some(v.to_string()),
            None => None,
        };

        self.daemon_command = DaemonHandler::consume(matches);
        self.daemon_command != DaemonCommand::None
    }

    fn run<D: Decorator + 'static + Sync + Send>(&self, decorator: D) {
        let config_module: &ConfigModule = decorator.extract().unwrap();

        if !DaemonHandler::hub(self.daemon_command, config_module.work_dir()).run() {
            return;
        }

        let sys = actix::System::new("gu-hub");

        let _config = ServerConfigurer::new(decorator.clone(), self.config_path.clone()).start();

        let _ = sys.run();
    }
}

fn mdns_publisher(port: u16, node_id: NodeId) -> std::io::Result<MdnsPublisher> {
    let _ = mdns::Responder::new()?;

    let mut publisher = MdnsPublisher::init_publisher(port, node_id.to_string(), true);
    publisher.start();
    Ok(publisher)
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

    fn hub_configuration(&mut self, c: Arc<HubConfig>) -> Result<(), String> {
        let config_module: &ConfigModule = self.decorator.extract().unwrap();
        let key = EthAccount::load_or_generate(config_module.keystore_path(), "")
            .expect("should load or generate eth key");

        let decorator = self.decorator.clone();
        let node_id = NodeId::from(key.address().as_ref());

        match self.decorator.extract::<super::hub_info::InfoModule>() {
            Some(v) => {
                v.set_node_id(node_id);
            }
            None => {}
        }

        let server = actix_web::server::new(move || {
            decorator.decorate_webapp(
                actix_web::App::with_state(node_id)
                    .middleware(actix_web::middleware::Logger::default())
                    .handler(
                        "/app",
                        actix_web::fs::StaticFiles::new("webapp")
                            .expect("cannot provide static files"),
                    )
                    .scope("/m", mock::scope)
                    .resource("/ws/", |r| r.route().f(chat_route))
                    .resource("/node_id/", |r| {
                        r.get().f(|req| {
                            actix_web::HttpResponse::with_body(
                                actix_web::http::StatusCode::OK,
                                format!(
                                    "{} {}",
                                    req.state().to_string(),
                                    hostname::get_hostname().unwrap_or("unknown".to_string())
                                ),
                            )
                        });
                    }),
            )
        });
        match server.bind(c.p2p_addr()) {
            Err(e) => {
                for addr in c.p2p_addr().to_socket_addrs().unwrap() {
                    return Err(format!("P2P socket binding for {} err: {}", addr, e));
                }
            }
            Ok(server) => {
                server.start();
                ()
            }
        };

        if c.publish_service {
            match mdns_publisher(c.p2p_port, node_id) {
                // we use Box::leak to prevent publisher from being dropped
                Ok(publisher) => {
                    Box::leak(Box::new(publisher));
                }
                Err(e) => error!("Failed to run mDNS publisher: {}", e),
            }
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
                .map_err(|e| error!("Cannot load config file of Hub: {}", e))
                .into_actor(self)
                .and_then(move |config, act, ctx| {
                    let _ = act.hub_configuration(config).map_err(|e| {
                        error!("Hub configuration error {:?}", e);
                        System::current().stop()
                    });
                    fut::ok(ctx.stop())
                }),
        );
    }
}
