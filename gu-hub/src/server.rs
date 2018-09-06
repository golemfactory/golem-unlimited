use actix::fut;
use actix::prelude::*;
use futures::future;
use futures::prelude::*;
use tokio;

use gu_persist::config;

use actix_web::server::HttpServer;
use actix_web::server::StopServer;
use actix_web::*;
use clap::{self, ArgMatches, SubCommand};
use gu_actix::*;
use std::borrow::Cow;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tokio_uds::UnixListener;

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ServerConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    p2p_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
}

const DEFAULT_P2P_PORT: u16 = 61622;

impl ServerConfig {
    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port.unwrap_or(DEFAULT_P2P_PORT))
    }
}

impl config::HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "server-cfg";
}

pub fn clap_declare<'a, 'b>() -> clap::App<'a, 'b> {
    SubCommand::with_name("server")
}

pub fn clap_match(m: &ArgMatches) {
    let config_path = match m.value_of("config-dir") {
        Some(v) => Some(v.to_string()),
        None => None,
    };

    if let Some(m) = m.subcommand_matches("server") {
        println!("server");
        run_server(config_path.to_owned());
    }
}

fn run_server(config_path: Option<String>) {
    use actix;

    let sys = actix::System::new("gu-hub");

    let config = ServerConfigurer(None, config_path).start();
    /*
    let listener = UnixListener::bind("/tmp/gu.socket").expect("bind failed");
    server::new(|| {
        App::new()
            // enable logger
            .middleware(middleware::Logger::default())
            .resource("/index.html", |r| r.f(|_| "Hello world!"))
    }).start_incoming(listener.incoming(), false);
*/
    println!("[[sys");
    let _ = sys.run();
    println!("sys]]");
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
                .flatten_fut()
                .map_err(|e| println!("error ! {}", e))
                .and_then(|c: Arc<ServerConfig>| {
                    let server = server::new(move || App::new().handler("/p2p", p2p_server));
                    let _ = server.bind(c.p2p_addr()).unwrap().start();
                    Ok(())
                })
                .into_actor(self)
                .and_then(|_, _, ctx| fut::ok(ctx.stop())),
        );
    }
}

impl Drop for ServerConfigurer {
    fn drop(&mut self) {
        info!("server configured")
    }
}
