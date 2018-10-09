//! Command line module for one-shot service discovery

use actix::{Arbiter, System};
use clap::{App, Arg, ArgMatches, SubCommand};
use futures::Future;
use gu_base::{cli, Module};
use service::ServiceInstance;
use std::{collections::HashSet, net::Ipv4Addr};
use actor::OneShot;
use actor::MdnsActor;
use service::ServicesDescription;

fn format_addresses(addrs_v4: &Vec<Ipv4Addr>, ports: &Vec<u16>) -> String {
    let mut res = String::new();
    let addr = addrs_v4
        .first()
        .map(|ip| format!("{:?}", ip))
        .unwrap_or("<missing ip>".to_string());

    for port in ports {
        res.push_str(addr.as_ref());
        res.push(':');
        res.push_str(&format!("{}", &port));
        res.push('\n');
    }

    res
}

pub fn format_instances_table(instances: &HashSet<ServiceInstance>) {
    cli::format_table(
        row!["Service type", "Host name", "Addresses", "Description"],
        || "No instances found",
        instances.iter().map(|instance| {
            row![
                instance.name,
                instance.host,
                format_addresses(&instance.addrs_v4, &instance.ports),
                instance.txt.join(""),
            ]
        }),
    )
}

fn run_client(m: &ArgMatches) {
    use actix;
    use actix::SystemService;

    let sys = actix::System::new("gu-lan");

    let instances = m
        .value_of("instance")
        .expect("default value not set")
        .split(',')
        .map(|s| s.to_string().into())
        .collect();

    let mdns_actor = MdnsActor::<OneShot>::from_registry();
    let query = ServicesDescription::new(instances);

    Arbiter::spawn(
        mdns_actor.send(query)
            .map_err(|e| error!("error! {}", e))
            .and_then(|r| r.map_err(|e| error!("error! {}", e)))
            .and_then(|r| Ok(format_instances_table(&r)))
            .map_err(|e| error!("error! {:?}", e))
            .then(|_| Ok(System::current().stop())),
    );

    let _ = sys.run();
}

pub struct LanModule;

impl Module for LanModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let instance = Arg::with_name("instance")
            .short("I")
            .help("queries mDNS server about some instance")
            .default_value("gu-hub,gu-provider");

        app.subcommand(
            SubCommand::with_name("lan").subcommand(SubCommand::with_name("list").arg(instance)),
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

fn clap_match_lan(m: &ArgMatches) {
    if let Some(m) = m.subcommand_matches("list") {
        run_client(m);
    } else {
        println!("{}", m.usage())
    }
}
