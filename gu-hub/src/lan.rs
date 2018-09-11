use clap::{App, ArgMatches, SubCommand};
use actix_web;
use futures::future::Future;
use actix_web::HttpMessage;
use gu_base::Module;
use actix_web::server::StopServer;
use actix::Recipient;
use gu_persist::config::ConfigManager;
use actix::Addr;
use gu_persist::config;
use gu_actix::flatten::FlattenFuture;
use server::ServerConfig;
use std::sync::Arc;
use gu_base::Arg;
use std::borrow::Cow;
use actix::SystemService;
use gu_lan::rest_server::{QueryLan, LAN_ENDPOINT};
use ::server::ServerConfigurer;

fn clap_match_lan(m: &ArgMatches) {
    if let Some(m) = m.subcommand_matches("list") {
        println!("lan list");
        run_client(m);
    } else {
        println!("{}", m.usage())
    }
}

fn get_config(config_path: Option<String>) -> impl Future<Item=Arc<ServerConfig>, Error=()> {
    use futures::future;

    future::ok(unimplemented!())
}

fn run_client(m: &ArgMatches) {
    use actix;

    let config_path = m.value_of("config-dir").map(String::from);
    let query = QueryLan::single(m.value_of("instance").map(String::from));

    let sys = actix::System::new("gu-lan");
    let address = get_config(config_path)
        .and_then(|config| Ok(config.port()))
        .and_then(|port| Ok(format!("http://localhost:{}/m/{}", port, LAN_ENDPOINT)));

    let query = address.and_then(move |addr| {
        actix_web::client::post(addr)
            .header("Content-type", "application/json")
            .body(query.to_json())
            .map_err(|_| panic!("Failed to build request"))
    });

    let response = query
        .and_then(|c| c.send().map_err(|_| ()))
        .map_err(|e| error!("Network error: {:?}", e))
        .and_then(|b| b.body().map_err(|e| error!("error: {:?}", e)))
        .map_err(|e| error!("Network error: {:?}", e))
        .then(|r| Ok(println!("{:#?}", r)))
        .and_then(|_| Ok(actix::System::current().stop()));

    actix_web::actix::spawn(response);
    let _ = sys.run();
}

pub struct LanModule;

impl Module for LanModule {

    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let instance = Arg::with_name("instance")
            .short("I")
            .help("queries mDNS server about some instance")
            .default_value("gu-hub");

        app.subcommand(
            SubCommand::with_name("lan")
                .subcommand(SubCommand::with_name("list").arg(instance))
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("lan") {
            clap_match_lan(m);
            return true;
        }
        false
    }
}