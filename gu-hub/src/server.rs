use actix::fut;
use actix::prelude::*;
use futures::prelude::*;

use gu_persist::config;

use actix_web;
use actix_web::server::StopServer;
use clap::{App, ArgMatches, SubCommand};
use gu_actix::*;
use std::borrow::Cow;
use std::net::ToSocketAddrs;
use std::sync::Arc;

use gu_base::Module;
use gu_p2p::rpc;
use mdns::Responder;
use gu_p2p::NodeId;
use mdns::Service;
use gu_p2p::rpc::start_actor;
use gu_lan::rest_server;
use gu_p2p::rpc::mock;
use gu_persist::config::ConfigManager;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ServerConfig {
    #[serde(default = "ServerConfig::default_p2p_port")]
    p2p_port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    control_socket: Option<String>,
    #[serde(default = "ServerConfig::publish_service")]
    publish_service: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            p2p_port: Self::default_p2p_port(),
            control_socket: None,
            publish_service: Self::publish_service(),
        }
    }
}

impl ServerConfig {
    fn default_p2p_port() -> u16 {
        61622
    }
    fn publish_service() -> bool {
        true
    }

    fn p2p_addr(&self) -> impl ToSocketAddrs {
        ("0.0.0.0", self.p2p_port)
    }

    pub fn port(&self) -> u16 {
        self.p2p_port
    }
}

impl config::HasSectionId for ServerConfig {
    const SECTION_ID: &'static str = "server-cfg";
}

pub struct ServerModule;

impl Module for ServerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("server")
            .about("hub server managment"))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        let config_path = match matches.value_of("config-dir") {
            Some(v) => Some(v.to_string()),
            None => None,
        };

        if let Some(_m) = matches.subcommand_matches("server") {
            run_server(config_path.to_owned());
            return true;
        }
        false
    }
}

fn p2p_server<S>(_r: &actix_web::HttpRequest<S>) -> &'static str {
    "ok"
}

fn run_publisher(run: bool, port: u16) {
    if run {
        let responder = Responder::new().expect("Failed to run publisher");

        let svc = Box::new(responder.register(
            "_unlimited._tcp".to_owned(),
            "gu-hub".to_owned(),
            port,
            &["path=/", ""],
        ));

        let _svc : &'static mut Service = Box::leak(svc);
    }
}

fn prepare_lan_server(run: bool) {
    if run {
        start_actor(rest_server::LanInfo());
    }
}

fn chat_route(req: &actix_web::HttpRequest<NodeId>) -> Result<actix_web::HttpResponse, actix_web::Error> {
    rpc::ws::route(req, req.state().clone())
}


fn hub_configuration(c: Arc<ServerConfig>, node_id : NodeId) -> Result<(),()> {
    let server = actix_web::server::new(move || {
        actix_web::App::with_state(node_id.clone())
            .handler("/p2p", p2p_server)
            .scope("/m", mock::scope)
            .resource("/ws/", |r| r.route().f(chat_route))
    });
    let _ = server.bind(c.p2p_addr()).unwrap().start();
    prepare_lan_server(c.publish_service);
    run_publisher(c.publish_service, c.p2p_port);

    Ok(())
}

/// IDEA: Code below should be common wit gu-provider
struct ServerConfigurer {
    recipent : Option<Recipient<StopServer>>,
    path : Option<String>,
}

impl ServerConfigurer {
    fn new(recipent : Option<Recipient<StopServer>>, path : Option<String>) -> Self {
        Self { recipent, path }
    }

    fn config(&self) -> Addr<ConfigManager> {
        let config = config::ConfigManager::from_registry();
        println!("path={:?}", &self.path);

        if let Some(path) = &self.path {
            config.do_send(config::SetConfigPath::FsPath(Cow::Owned(path.clone())));
        }
        config
    }
}
impl Actor for ServerConfigurer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        use rand::*;

        let node_id : NodeId = thread_rng().gen();

        ctx.spawn(self.config()
            .send(config::GetConfig::new())
            .flatten_fut()
            .map_err(|e| println!("error ! {}", e))
            .and_then(move |config| hub_configuration(config, node_id))
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

fn run_server(config_path: Option<String>) {
    use actix;

    let sys = actix::System::new("gu-hub");

    let _config = ServerConfigurer::new(None, config_path).start();

    let _ = sys.run();
}

