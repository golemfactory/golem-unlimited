use actix::fut;
use actix::prelude::*;
use actix_web::*;
use clap::{self, Arg, ArgMatches, SubCommand};
use futures::prelude::*;
use gu_base::Decorator;
use gu_base::Module;
use gu_ethkey::{EthKey, EthKeyStore, SafeEthKey};
use gu_p2p::{rpc, NodeId};
use gu_persist::config::{
    ConfigManager, ConfigModule, Error as ConfigError, GetConfig, HasSectionId, SetConfig,
    SetConfigPath,
};
use hdman::HdMan;
use mdns::Responder;
use std::borrow::Cow;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hub_addr: Option<SocketAddr>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: 61621,
            control_socket: None,
            hub_addr: None,
        }
    }
}

impl ServerConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }
}

impl HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "provider-server-cfg";
}

pub struct ServerModule {
    active: bool,
    config_path: Option<String>,
    hub_addr: Option<SocketAddr>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            active: false,
            config_path: None,
            hub_addr: None,
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
        app.subcommand(
            SubCommand::with_name("server")
                .about("provider server management")
                .subcommand(SubCommand::with_name("connect").arg(Arg::with_name("hub_addr"))),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        self.config_path = matches.value_of("config-dir").map(ToString::to_string);

        if let Some(m) = matches.subcommand_matches("server") {
            println!("server");
            self.active = true;
            if let Some(mc) = m.subcommand_matches("connect") {
                let param = mc.value_of("hub_addr");
                info!("hub addr={:?}", &param);
                if let Some(addr) = param {
                    self.hub_addr = Some(addr.parse().unwrap())
                }
            }
            return true;
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        use actix;

        if !self.active {
            return;
        }

        let config_module: &ConfigModule = decorator.extract().unwrap();
        // TODO: introduce separate actor for key mgmt
        let keys = SafeEthKey::load_or_generate(config_module.keystore_path(), &"".into()).unwrap();

        let _ = ServerConfigurer {
            config_path: self.config_path.clone(),
            node_id: get_node_id(keys),
            hub_addr: self.hub_addr,
            decorator: decorator.clone(),
        }.start();

        let _ = HdMan::start(config_module);

        let sys = actix::System::new("gu-provider");
        let _ = sys.run();
    }
}

fn p2p_server(_r: &HttpRequest) -> &'static str {
    "ok"
}

fn run_mdns_publisher(port: u16) {
    let responder = Responder::new().expect("Failed to run mDNS publisher");

    let svc = Box::new(responder.register(
        "_unlimited._tcp".to_owned(),
        "gu-provider".to_owned(),
        port,
        &["path=/", ""],
    ));

    let _ = Box::leak(svc);
}

struct ServerConfigurer<D> {
    decorator: D,
    config_path: Option<String>,
    node_id: NodeId,
    hub_addr: Option<SocketAddr>,
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
        let hub_addr = self.hub_addr.clone();

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

                    run_mdns_publisher(c.p2p_port);

                    if let Some(hub_addr) = hub_addr {
                        config.do_send(SetConfig::new(ServerConfig {
                            hub_addr: Some(hub_addr),
                            ..(*c).clone()
                        }));
                        rpc::ws::start_connection(node_id, hub_addr);
                    } else if let Some(hub_addr) = c.hub_addr {
                        rpc::ws::start_connection(node_id, hub_addr);
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
