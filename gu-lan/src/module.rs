//! Command line module for one-shot service discovery

use actix::{Arbiter, System};
use actor::MdnsActor;
use actor::OneShot;
use clap::{App, Arg, ArgMatches, SubCommand};
use futures::Future;
use gu_base::Decorator;
use gu_base::{cli, Module};
use service::ServiceInstance;
use service::ServicesDescription;
use std::{collections::HashSet, net::Ipv4Addr};

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

fn run_client(instances: &String) {
    use actix;
    use actix::SystemService;

    let sys = actix::System::new("gu-lan");
    let instances = instances.split(',').map(|s| s.to_string().into()).collect();

    let mdns_actor = MdnsActor::<OneShot>::from_registry();
    let query = ServicesDescription::new(instances);

    Arbiter::spawn(
        mdns_actor
            .send(query)
            .map_err(|e| error!("error! {}", e))
            .and_then(|r| r.map_err(|e| error!("error! {}", e)))
            .and_then(|r| Ok(format_instances_table(&r)))
            .map_err(|e| error!("error! {:?}", e))
            .then(|_| Ok(System::current().stop())),
    );

    let _ = sys.run();
}

enum LanCommand {
    None,
    List(String),
}

pub struct LanModule {
    command: LanCommand,
}

impl LanModule {
    pub fn module() -> LanModule {
        Self {
            command: LanCommand::None,
        }
    }
}

impl Module for LanModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let instance = Arg::with_name("instance")
            .short("I")
            .help("queries mDNS server about some instance")
            .default_value("gu-hub,gu-provider");

        app.subcommand(
            SubCommand::with_name("lan")
                .subcommand(
                    SubCommand::with_name("list")
                        .about("Lists available instances")
                        .arg(instance),
                ).about("Lan services"),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("lan") {
            self.command = match m.subcommand() {
                ("list", Some(m)) => {
                    let instance = m
                        .value_of("instance")
                        .expect("Lack of required `instance` argument")
                        .to_string();
                    LanCommand::List(instance)
                }
                _ => return false,
            };
            true
        } else {
            false
        }
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self.command {
            LanCommand::List(ref s) => run_client(s),
            _ => (),
        }
    }
}
