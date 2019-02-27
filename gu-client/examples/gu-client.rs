/***
Command line tool for manage gu-hub instance

**/
use actix::prelude::*;
use failure::Fallible;
use futures::{future, prelude::*};
use gu_client::r#async::*;
use gu_model::peers::PeerInfo;
use gu_model::session::HubExistingSession;
use gu_model::session::HubSessionSpec;
use gu_model::session::SessionDetails;
use gu_net::NodeId;
use std::collections::BTreeSet;
use std::path::PathBuf;
use structopt::*;

#[derive(StructOpt, Debug)]
enum ClientArgs {
    /// Lists providers connected to hub.
    #[structopt(name = "prov-list")]
    ListProviders,

    #[structopt(name = "prov")]
    Providers {
        /// provider id
        #[structopt(name = "NODE_ID", parse(try_from_str))]
        provider_id: NodeId,

        #[structopt(subcommand)]
        command: Option<Providers>,
    },

    /// lists hub sessions.
    #[structopt(name = "sess-list")]
    ListSessions,

    #[structopt(name = "sess")]
    Sessions {
        /// HUB session id.
        #[structopt(name = "SESSION_ID", parse(try_from_str))]
        session_id: u64,
        #[structopt(subcommand)]
        command: Option<Sessions>,
    },

    #[structopt(name = "render")]
    Render(Render),
}

#[derive(StructOpt, Debug)]
struct Render {
    #[structopt(long, short)]
    name: Option<String>,
    #[structopt(long, short)]
    frame: Vec<usize>,
    #[structopt(long)]
    docker: bool,

    #[structopt(name = "RESOURCE", parse(from_os_str))]
    resource: Vec<PathBuf>,
}

#[derive(StructOpt, Debug)]
enum Providers {
    /// drops deployment by id or tag
    #[structopt(name = "drop")]
    DropDeployment {
        /// deployment id
        #[structopt(short = "d", group = "select")]
        deployment_id: Option<String>,
        #[structopt(short = "t", group = "select")]
        tag: Vec<String>,
    },
}

#[derive(StructOpt, Debug)]
enum Sessions {
    // Drops selected session
    #[structopt(name = "drop")]
    DropSession,
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

fn show_sessions<Sessions: IntoIterator<Item = HubExistingSession>>(
    sessions: Sessions,
) -> impl IntoFuture<Item = (), Error = gu_client::error::Error> {
    use prettytable::{cell, format, row, Table};

    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER);

    table.add_row(row!["Id", "Name", "Created", "Tags"]);
    for it in sessions {
        table.add_row(row![
            it.id,
            it.spec.name.unwrap_or_default(),
            it.created.naive_local(),
            join_str(it.spec.tags.iter()),
        ]);
    }

    if table.is_empty() {
        Ok(println!("no sessions found"))
    } else {
        Ok(table.printstd())
    }
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
        if table.is_empty() {
            Ok(println!("no deployments found"))
        } else {
            Ok(table.printstd())
        }
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

                    deployment.delete().and_then(move |_| {
                        Ok(eprintln!("deployment id={}, name={} dropped", id, name))
                    })
                }),
        )
        .and_then(|_| Ok(()))
    }))
}

fn show_session(
    driver: &HubConnection,
    session_id: u64,
) -> Box<dyn Future<Item = (), Error = gu_client::error::Error>> {
    let hub_session = driver.hub_session(session_id);

    Box::new(
        hub_session
            .info()
            .join4(
                hub_session.list_peers(),
                hub_session.list_blobs(),
                hub_session.config(),
            )
            .and_then(move |(info, peers, blobs, config)| {
                println!("id={}: name={}", session_id, info.name.unwrap_or_default());
                println!("\nconfig:\n------");
                println!("{}", serde_json::to_string_pretty(&config).unwrap());
                println!("\npeers:\n------");
                for peer in peers {
                    println!(" - {:?}", peer.node_id)
                }
                println!("\nblobs:\n------");
                for blob in blobs {
                    println!(" - {:?}", blob.id)
                }

                Ok(())
            }),
    )
}

fn render_task(
    connection: &HubConnection,
    opts: Render,
) -> Box<dyn Future<Item = (), Error = gu_client::error::Error>> {
    use gu_model::chrono::prelude::*;

    let spec = HubSessionSpec {
        expires: None,
        allocation: gu_model::session::AllocationMode::MANUAL,
        name: Some(
            opts.name
                .unwrap_or_else(|| format!("blender at {:?}", Utc::now())),
        ),
        tags: vec!["gu:render".to_string(), "gu:blender".to_string()]
            .into_iter()
            .collect(),
    };

    let peers = connection.list_peers();

    Box::new(connection.new_session(spec).and_then(|session| {
        // Work
        // 1. Upload resources to session
        // 2. Create deployments on peer
        // 3. Run processing
        // 4. upload results
        ;
        Ok(())
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
                None => show_peer(driver.peer(provider_id)),
            },
            ClientArgs::ListSessions => Box::new(
                driver
                    .list_sessions()
                    .and_then(|sessions| show_sessions(sessions)),
            ),
            ClientArgs::Sessions {
                session_id,
                command,
            } => match command {
                Some(Sessions::DropSession) => Box::new(driver.hub_session(session_id).delete()),
                None => show_session(&driver, session_id),
            },
            ClientArgs::Render(render_opts) => render_task(&driver, render_opts),
            v => {
                eprintln!("unimplemented opts: {:?}", v);
                unimplemented!()
            }
        }
    }))?;

    Ok(())
}
