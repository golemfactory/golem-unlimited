use clap::{App, ArgMatches, SubCommand};
use actix_web;
use futures::future::Future;
use actix_web::HttpMessage;
use gu_persist::config;
use actix::Addr;
use gu_persist::config::ConfigManager;
use actix::Recipient;
use actix_web::server::StopServer;
use actix::SystemService;
use std::borrow::Cow;
use gu_actix::flatten::FlattenFuture;
use std::sync::Arc;
use ::server::ServerConfig;

pub fn clap_declare<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("lan").subcommand(SubCommand::with_name("list"))
}

pub fn clap_match(m: &ArgMatches) {
    if let Some(m) = m.subcommand_matches("lan") {
        clap_match_lan(m)
    }
}

fn clap_match_lan(m: &ArgMatches) {
    if let Some(_m) = m.subcommand_matches("list") {
        lan_query(m);
    } else {
        println!("{}", m.usage())
    }
}

fn lan_query(m: &ArgMatches) {
    let config_path = match m.value_of("config-dir") {
        Some(v) => Some(v.to_string()),
        None => None,
    };

    println!("lan list");
    run_client(config_path.to_owned());
}

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


impl Drop for ServerConfigurer {
    fn drop(&mut self) {
        info!("server configured")
    }
}

fn get_config(config_path: Option<String>) -> impl Future<Item=Arc<ServerConfig>, Error=()> {
    use futures::future;

    future::ok(unimplemented!())
}

fn run_client(config_path: Option<String>) {
    use actix;

    let sys = actix::System::new("gu-lan");
    let address = get_config(config_path)
        .and_then(|config| Ok(config.p2p_port))
        .and_then(|port| Ok(format!("http://localhost:61622/m/{}", port)));

    let query = address.and_then(|addr| {
        actix_web::client::post(addr)
            .header("Content-type", "application/json")
            .body("{}")
            .map_err(|e| panic!("Failed to build request"))
    });

    let response = query
        .and_then(|a| {println!("Mid: {:?}", a); Ok(a)})
        .and_then(|a| a.send().map_err(|e| ()))
        .map_err(|e| error!("Network error: {:?}", e))
        .and_then(|a| Ok(a.body().and_then(|a| Ok(println!("End: {:?}", a)))))
        .then(|_| Ok(()));

    actix_web::actix::spawn(response);
    let _ = sys.run();
}