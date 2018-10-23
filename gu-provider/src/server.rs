#![allow(dead_code)]

use actix::fut;
use actix::prelude::*;
use actix_web::*;
use clap::{self, Arg, ArgMatches};
use futures::prelude::*;
use gu_base::Decorator;
use gu_base::Module;
use gu_ethkey::{EthKey, EthKeyStore, SafeEthKey};
use gu_p2p::{rpc, NodeId};
use gu_persist::config::{
    ConfigManager, ConfigModule, GetConfig, HasSectionId, SetConfig, SetConfigPath,
};
use gu_persist::daemon_module::DaemonModule;
use gu_persist::error::Error as ConfigError;
use hdman::HdMan;
use mdns::Responder;
use mdns::Service;
use std::borrow::Cow;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerConfig {
    #[serde(default = "ServerConfig::default_p2p_port")]
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hub_addrs: Option<Vec<SocketAddr>>,
    #[serde(default)]
    publish_service: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: 61621,
            control_socket: None,
            hub_addrs: None,
            publish_service: true,
        }
    }
}

#[derive(PartialEq)]
pub(crate) enum ConnectMode {
    Auto,
    Config,
}

impl ServerConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    fn default_p2p_port() -> u16 {
        61621
    }
}

impl HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "provider-server-cfg";
}

pub struct ServerModule {
    config_path: Option<String>,
    hub_addrs: Option<Vec<SocketAddr>>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            config_path: None,
            hub_addrs: None,
        }
    }
}

fn get_node_id(keys: Box<SafeEthKey>) -> NodeId {
    let node_id = NodeId::from(keys.address().as_ref());
    info!("node_id={:?}", node_id);
    node_id
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
        app.arg(
            Arg::with_name("hub_addr")
                .short("a")
                .long("hub address")
                .takes_value(true)
                .value_name("IP:PORT")
                .help("IP and PORT of Hub to connect to"),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        self.config_path = matches.value_of("config-dir").map(ToString::to_string);

        if let Some(hub_addr) = matches.value_of("hub_addr") {
            info!("hub addr={:?}", &hub_addr);
            self.hub_addrs = Some(vec![hub_addr.parse().unwrap()])
        }
        true
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let daemon_module: &DaemonModule = decorator.extract().unwrap();
        if !daemon_module.run() {
            return;
        }

        let config_module: &ConfigModule = decorator.extract().unwrap();

        // TODO: introduce separate actor for key mgmt
        let keys = SafeEthKey::load_or_generate(config_module.keystore_path(), &"".into()).unwrap();

        let _ = ServerConfigurer {
            config_path: self.config_path.clone(),
            node_id: get_node_id(keys),
            hub_addrs: self.hub_addrs.clone(),
            decorator: decorator.clone(),
        }.start();

        let _ = HdMan::start(config_module);

        let sys = System::new("gu-provider");
        let _ = sys.run();
    }
}

fn p2p_server(_r: &HttpRequest) -> &'static str {
    "ok"
}

fn mdns_publisher(port: u16) -> Service {
    let responder = Responder::new().expect("Failed to run mDNS publisher");

    responder.register(
        "_unlimited._tcp".to_owned(),
        "gu-provider".to_owned(),
        port,
        &["path=/", ""],
    )
}

struct ServerConfigurer<D> {
    decorator: D,
    config_path: Option<String>,
    node_id: NodeId,
    hub_addrs: Option<Vec<SocketAddr>>,
}

impl<D: Decorator + 'static + Sync + Send> Actor for ServerConfigurer<D> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let config = ConfigManager::from_registry();

        println!("path={:?}", &self.config_path);
        if let Some(path) = &self.config_path {
            config.do_send(SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }

        let node_id = self.node_id.clone();
        let hub_addrs = self.hub_addrs.clone();

        let decorator = self.decorator.clone();
        ctx.spawn(
            config
                .send(GetConfig::new())
                .map_err(|e| ConfigError::from(e))
                .and_then(|r| r)
                .map_err(|e| println!("error ! {}", e))
                .and_then(move |c: Arc<ServerConfig>| {
                    let decorator = decorator.clone();
                    let server = server::new(move || {
                        decorator.decorate_webapp(App::new().scope("/m", rpc::mock::scope))
                    });
                    let _ = server.bind(c.p2p_addr()).unwrap().start();

                    if c.publish_service {
                        Box::leak(Box::new(mdns_publisher(c.p2p_port)));
                    }

                    if let Some(hub_addrs) = hub_addrs {
                        config.do_send(SetConfig::new(ServerConfig {
                            hub_addrs: Some(hub_addrs.clone()),
                            ..(*c).clone()
                        }));
                        connect_to_multiple_hubs(node_id, &hub_addrs);
                    } else if let Some(ref hub_addrs) = c.hub_addrs {
                        connect_to_multiple_hubs(node_id, hub_addrs);
                    }

                    Ok(())
                }).into_actor(self)
                .and_then(|_, _, ctx| fut::ok(ctx.stop())),
        );

        println!("configured");
    }
}

impl<D> Drop for ServerConfigurer<D> {
    fn drop(&mut self) {
        println!("provider server configured")
    }
}

fn connect_to_multiple_hubs(id: NodeId, hubs: &Vec<SocketAddr>) {
    for hub in hubs {
        rpc::ws::start_connection(id, *hub);
    }
}
