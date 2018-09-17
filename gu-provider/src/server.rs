use actix::fut;
use actix::prelude::*;
//use futures::future;
use futures::prelude::*;
//use tokio;

use gu_persist::config;

//use actix_web::server::HttpServer;
use actix_web::server::StopServer;
use actix_web::*;
use clap::{self, Arg, ArgMatches, SubCommand};
use std::borrow::Cow;
use std::net::{self, ToSocketAddrs};
use std::sync::Arc;

use gu_base::Decorator;
use gu_base::Module;
use gu_p2p::rpc;
use gu_p2p::NodeId;
use gu_persist::config::ConfigModule;
use gu_ethkey::{EthKey, EthKeyStore, SafeEthKey};
use mdns::{Responder, Service};
use std::path::PathBuf;


#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: 61621,
            control_socket: None,
        }
    }
}

impl ServerConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }
}

impl config::HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "provider-server-cfg";
}

pub struct ServerModule {
    active: bool,
    config_path: Option<String>,
    peer_addr: Option<net::SocketAddr>,
}

impl ServerModule {
    pub fn new() -> Self {
        ServerModule {
            active: false,
            config_path: None,
            peer_addr: None,
        }
    }
}

fn get_node_id<P: Into<PathBuf>>(keystore_path: P) -> NodeId {
    let keys = SafeEthKey::load_or_generate(keystore_path, &"".into()).unwrap();
    let mut public_key_bytes: &[u8] = keys.public().as_ref();
    // TODO: NodeId 32 --> 64 ?
    let node_id = NodeId::from(&public_key_bytes[0..32]);
    info!("node_id={:?}", node_id);
    node_id
}

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
        app.subcommand(
            SubCommand::with_name("server")
                .about("provider server management")
                .subcommand(SubCommand::with_name("connect").arg(Arg::with_name("peer_addr"))),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        self.config_path = matches.value_of("config-dir").map(ToString::to_string);

        if let Some(m) = matches.subcommand_matches("server") {
            println!("server");
            self.active = true;
            if let Some(mc) = m.subcommand_matches("connect") {
                let param = mc.value_of("peer_addr");
                info!("peer addr={:?}", &param);
                if let Some(addr) = param {
                    self.peer_addr = Some(addr.parse().unwrap())
                }
            }
            return true;
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        use actix;
        //    use env_logger;
        use rand::*;

        if !self.active {
            return;
        }

        let config = ServerConfigurer(None, self.config_path.clone()).start();
        let sys = actix::System::new("gu-provider");

        let configModule: &ConfigModule = decorator.extract().unwrap();
        let _ = super::hdman::start(configModule);



        if let Some(a) = self.peer_addr {
            let _ = rpc::ws::start_connection(get_node_id(configModule.keystore_path()), a);
        }

        let _ = sys.run();
    }
}

fn p2p_server(r: &HttpRequest) -> &'static str {
    "ok"
}

fn run_mdns_publisher(run: bool, port: u16) {
    if run {
        let responder = Responder::new().expect("Failed to run mDNS publisher");

        let svc = Box::new(responder.register(
            "_unlimited._tcp".to_owned(),
            "gu-provider".to_owned(),
            port,
            &["path=/", ""],
        ));

        let _svc: &'static mut Service = Box::leak(svc);
    }
}


struct ServerConfigurer(Option<Recipient<StopServer>>, Option<String>);

impl Actor for ServerConfigurer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let config = config::ConfigManager::from_registry();

        println!("path={:?}", &self.1);
        if let Some(path) = &self.1 {
            config.do_send(config::SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }

        ctx.spawn(
            config
                .send(config::GetConfig::new())
                .map_err(|e| config::Error::from(e))
                .and_then(|r| r)
                .map_err(|e| println!("error ! {}", e))
                .and_then(|c: Arc<ServerConfig>| {
                    let server = server::new(move || {
                        App::new()
                            .handler("/p2p", p2p_server)
                            .scope("/m", rpc::mock::scope)
                    });
                    let s = server.bind(c.p2p_addr()).unwrap().start();
                    run_mdns_publisher(true, c.p2p_port);
                    Ok(())
                })
                .into_actor(self)
                .and_then(|_, _, ctx| fut::ok(ctx.stop())),
        );
        println!("configured");
    }
}

impl Drop for ServerConfigurer {
    fn drop(&mut self) {
        println!("provider server configured")
    }
}
