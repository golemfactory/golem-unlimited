use clap::{App, ArgMatches, SubCommand};
use actix_web;
use futures::future::Future;
use actix_web::HttpMessage;
use futures::future;


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
        lan_query();
    } else {
        println!("{}", m.usage())
    }
}

// TODO: fix hardcoding and endless, useless waiting
fn lan_query() {
    use actix;
    let sys = actix::System::new("gu-lan");

    let response = actix_web::client::post("http://localhost:61622/m/576411")
        .header("Content-type", "application/json")
        .body("{}")
        .and_then(|a| {
            println!("{:?}", a);
            Ok(a)
        }).expect("failed to create request")
        .send()
        .map_err(|e| error!(">>>>>>>>>>.. error: {}", e));

    let fut =
        response.and_then(|a| a.body()
            .and_then(|a| Ok(println!("{:?}", a)))
            .map_err(|_| ()));

    actix_web::actix::spawn(fut);
    let _ = sys.run();
}

use gu_base::Module;

pub struct LanModule;

impl Module for LanModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(
            SubCommand::with_name("lan")
                .subcommand(SubCommand::with_name("list"))
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