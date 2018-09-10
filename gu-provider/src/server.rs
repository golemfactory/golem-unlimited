use actix::fut;
use actix::prelude::*;
use futures::future;
use futures::prelude::*;
use tokio;

use gu_persist::config;

use actix_web::server::HttpServer;
use actix_web::server::StopServer;
use actix_web::*;
use clap::{self, ArgMatches, SubCommand, Arg};
use std::borrow::Cow;
use std::net::{self, ToSocketAddrs};
use std::sync::Arc;

use gu_p2p::rpc;
use gu_p2p::NodeId;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
}

const DEFAULT_P2P_PORT: u16 = 61622;

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: DEFAULT_P2P_PORT,
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

pub fn clap_declare<'a, 'b>() -> clap::App<'a, 'b> {
    SubCommand::with_name("server")
        .subcommand(
            SubCommand::with_name("connect")
            .arg(Arg::with_name("peer_addr"))
        )
}

pub fn clap_match(m: &ArgMatches) {
    let config_path = match m.value_of("config-dir") {
        Some(v) => Some(v.to_string()),
        None => None,
    };

    let mut peer_addr = None;

    if let Some(m) = m.subcommand_matches("server") {
        println!("server");
        if let Some(mc) = m.subcommand_matches("connect") {
            let param = mc.value_of("peer_addr");
            info!("addr={:?}", &param);
            if let Some(a) = param {
                peer_addr = Some(a.parse().unwrap())
            }
        }
        run_server(config_path.to_owned(), peer_addr);
    }
}

fn run_server(config_path: Option<String>, peer_addr : Option<net::SocketAddr>) {
    use actix;
    use env_logger;
    use rand::*;

    let node_id : NodeId = thread_rng().gen();

    let sys = actix::System::new("gu-provider");

    let _ = super::hdman::start();

    let _ = match peer_addr {
        Some(a) => Some(rpc::ws::start_connection(node_id, a)),
        None => None
    };

    let config = ServerConfigurer(None, config_path).start();

    let _ = sys.run();
}

fn p2p_server(r: &HttpRequest) -> &'static str {
    "ok"
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
        println!("drop")
    }
}
