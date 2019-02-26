/***
Command line tool for manage gu-hub instance

**/
use actix::prelude::*;
use failure::Fallible;
use futures::{future, prelude::*};
use gu_client::r#async::*;
use gu_model::peers::PeerInfo;
use gu_net::NodeId;
use std::collections::BTreeSet;
use structopt::*;

#[derive(StructOpt)]
enum ClientArgs {
    #[structopt(name = "prov")]
    Providers {
        /// provider id
        #[structopt(name = "NODE_ID", parse(try_from_str))]
        provider_id: NodeId,

        #[structopt(subcommand)]
        command: Option<Providers>,
    },

    /// Lists providers connected to hub.
    #[structopt(name = "prov-list")]
    ListProviders,
}

#[derive(StructOpt)]
enum Providers {
    #[structopt(name = "drop-deployment")]
    DropDeployment {
        /// deployment id
        #[structopt(short = "d")]
        deployment_id: Option<String>,
        #[structopt(short = "t")]
        tag: Vec<String>,
    },
}

fn show_peers<Peers: IntoIterator<Item = PeerInfo>>(peers: Peers) {
    use prettytable::{cell, format, row, Table};

    let mut table = Table::new();
    //table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

    table.add_row(row!["Id", "Name"]);
    for it in peers {
        table.add_row(row![it.node_id, it.node_name.unwrap_or(it.peer_addr)]);
    }

    table.printstd();
}

fn join_str<AsStr: AsRef<str>, Items: Iterator<Item = AsStr>>(items: Items) -> String {
    let mut buf = String::new();

    for it in items {
        if buf.len() > 0 {
            buf.push_str(", ")
        }
        buf.push_str(it.as_ref())
    }

    buf
}

fn show_peer(provider: ProviderRef) -> Box<dyn Future<Item = (), Error = gu_client::error::Error>> {
    Box::new(provider.deployments().and_then(|deployments| {
        use prettytable::{cell, format, row, Table};
        let mut table = Table::new();

        table.set_titles(row!["Id", "Name", "Tags", "Note"]);
        for deployment in deployments {
            table.add_row(row![
                deployment.id(),
                deployment.name(),
                join_str(deployment.tags()),
                deployment.note().unwrap_or("")
            ]);
        }
        Ok(table.printstd())
    }))
}

fn drop_deployment(
    driver: &HubConnection,
    provider_id: NodeId,
    deployment_id: Option<String>,
    tag: Vec<String>,
) -> Box<dyn Future<Item = (), Error = gu_client::error::Error>> {
    let peer = driver.peer(provider_id);
    let tags: BTreeSet<String> = tag.into_iter().collect();

    Box::new(peer.deployments().and_then(move |deployments| {
        future::join_all(
            deployments
                .into_iter()
                .filter(move |deployment| {
                    deployment_id
                        .as_ref()
                        .map(|id| deployment.id() == id)
                        .unwrap_or(true)
                })
                .filter(move |deployment| {
                    tags.is_empty() || deployment.tags().any(|s| tags.contains(s.as_ref()))
                })
                .map(|deployment| {
                    let id = deployment.id().to_owned();
                    let name = deployment.name().to_owned();

                    deployment.delete().and_then(move |_| Ok(eprintln!("deployment id={}, name={} dropped", id, name)))
                }),
        )
        .and_then(|_| Ok(()))
    }))
}

fn main() -> Fallible<()> {
    let mut sys = System::new("gu-client");

    sys.block_on(future::lazy(|| {
        let args = ClientArgs::from_args();
        let driver = HubConnection::default();

        match args {
            ClientArgs::ListProviders => {
                Box::new(driver.list_peers().and_then(|p| Ok(show_peers(p))))
            }
            ClientArgs::Providers {
                provider_id,
                command,
            } => match command {
                Some(Providers::DropDeployment { deployment_id, tag }) => {
                    drop_deployment(&driver, provider_id, deployment_id, tag)
                }
                None => show_peer(driver.peer(provider_id))
            },
            _ => unimplemented!(),
        }
    }))?;

    Ok(())
}
