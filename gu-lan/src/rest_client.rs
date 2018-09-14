use actix::{Arbiter, System};
use clap::{App, Arg, ArgMatches, SubCommand};
use futures::Future;
use gu_base::{cli, Module};
use gu_p2p::rpc::start_actor;
use server::{self, QueryLan};
use prettytable::Table;
use std::collections::HashSet;
use service::ServiceInstance;

fn print_instances_table(instances: &HashSet<ServiceInstance>) {
    let mut table = Table::new();
    table.set_titles(row!["Service type", "Host name", "Addresses", "Ports", "Description"]);
    for instance in instances {
        table.add_row(row![instance.name, instance.host, format!("{:?}", instance.addrs),
        format!("{:?}", instance.ports), instance.txt.join(", "),]);
    }

    table.set_format(*cli::FORMAT_BASIC);
    table.printstd()
}

fn run_client(m: &ArgMatches) {
    use actix;

    let sys = actix::System::new("gu-lan");

    let instance = m.value_of("instance").expect("default value not set");
    let query = QueryLan::single(instance.to_string());
    let addr = start_actor(server::LanInfo());

    Arbiter::spawn(
        addr.send(query)
            .map_err(|e| error!("error! {}", e))
            .and_then(|r| r)
            .and_then(|r| Ok(print_instances_table(&r)))
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
            .default_value("gu-hub");

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
        debug!("lan list");
        run_client(m);
    } else {
        println!("{}", m.usage())
    }
}
