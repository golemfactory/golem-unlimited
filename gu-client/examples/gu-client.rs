/***
Command line tool for manage gu-hub instance

**/
use actix::prelude::*;
use failure::Fallible;
use futures::{future, prelude::*};
use gu_actix::pipe;
use gu_actix::release::Handle;
use gu_client::error::Error;
use gu_client::r#async::*;
use gu_model::peers::PeerInfo;
use gu_model::session::HubExistingSession;
use gu_model::session::HubSessionSpec;
use gu_model::session::SessionDetails;
use gu_net::NodeId;
use std::collections::BTreeSet;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{fs, io, thread};
use structopt::*;
use gu_model::envman::CreateSession;
use gu_model::envman::Image;
use gu_model::envman::Command;
use gu_model::envman::ResourceFormat;
use serde_derive::*;
use serde::Serialize;


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
    frame: Vec<u32>,
    #[structopt(long)]
    docker: bool,

    #[structopt(short, long, parse(try_from_str="parse_resolution"))]
    resolution : (u32, u32),

    #[structopt(name = "RESOURCE", parse(from_os_str), raw(required = "true"))]
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

fn parse_resolution(r : &str) -> Result<(u32, u32), Error> {
    let it = r.split("x");

    Err(Error::Other(format!("err")))
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

#[derive(Serialize, Debug)]
struct BlenderTaskSpec {
    crops : Vec<Crop>,
    samples: u32,
    resolution: (u32, u32),
    frames: Vec<u32>,
    scene_file: Option<String>,
    output_format: Option<String>,
}

#[derive(Serialize, Debug)]
struct Crop {
    borders_x : (f64, f64),
    borders_y : (f64, f64),
    outfilebasename: String,
}

fn run_worker(session : &PeerSession, frame : futures::unsync::mpsc::Receiver<BlenderTaskSpec>) -> impl Future<Item=(), Error=Error> {
    future::err(unimplemented!())
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

    let resources: Vec<_> = match opts
        .resource
        .iter()
        .map(|f| fs::canonicalize(f))
        .collect::<io::Result<Vec<_>>>()
    {
        Ok(v) => v,
        Err(e) => return Box::new(future::err(Error::Other(format!("{}", e)))),
    };

    let base_path = match common_path::common_path_all(resources.iter().map(|p| p.as_path())) {
        Some(path) => path,
        None => {
            return Box::new(future::err(Error::Other(format!(
                "unable to find common path for: {:?}",
                resources
            ))));
        }
    };
    eprintln!("base={:?}, t={:?}", base_path, resources);
    let (tx, rx) = pipe::sync_to_async(5);

    let mut work_resources = resources.clone();
    let work_base = base_path.clone();
    thread::spawn(move || {
        use tar::*;
        use std::io::prelude::*;

        match (|| -> Fallible<()> {
            work_resources.sort();
            let mut builder = Builder::new(tx);
            let mut dirs = BTreeSet::new();

            for res in work_resources {
                let rel_name = res.strip_prefix(&work_base)?;

                {
                    for a in rel_name.parent().unwrap().ancestors().collect::<Vec<_>>().into_iter().rev().skip(1) {
                        if !dirs.contains(a) {
                            eprintln!("prep dir={}", a.display());
                            let mut h= Header::new_ustar();
                            h.set_entry_type(EntryType::Directory);
                            h.set_mode(0o644);
                            h.set_uid(0);
                            h.set_gid(0);
                            h.set_mtime(0);
                            h.set_size(0);
                            h.set_path(a)?;
                            h.set_cksum();
                            let mut data2 = io::repeat(0).take(0);
                            builder.append(&h, data2)?;
                            eprintln!("dir={}", a.display());
                            dirs.insert(a.to_owned());
                        }
                    }
                }

                let mut f = fs::OpenOptions::new().write(false).read(true).open(&res)?;
                builder.append_file(rel_name, &mut f)?
            }
            builder.finish()?;
            Ok(())
        })() {
            Ok(()) => (),
            Err(e) => {
                eprintln!("fail to generate tar: {}", e);
            }
        }
        eprintln!("data uploaded");
    });


    let (task_tx, task_rx) = crossbeam_channel::unbounded();

    for &frame in &opts.frame {
        task_tx.send(BlenderTaskSpec {
            crops: vec![Crop {
                borders_x: (0.0, 1.0),
                borders_y: (0.0, 1.0),
                outfilebasename: "outf_".to_string()
            }],
            samples: 0,
            resolution: opts.resolution,
            frames: vec![frame ],
            scene_file: None,
            output_format: None
        }).unwrap();
    }



    Box::new(connection.new_session(spec).and_then(move |session| {
        // Work
        // 1. Upload resources to session
        // 2. Create deployments on peer
        // 3. Run processing
        // 4. upload results
        let upload_blob = session.new_blob().and_then(|blob: Blob| {
            eprintln!("new_blob={}", blob.id());
            blob.upload_from_stream(rx)
                .and_then(move |_| Ok(blob.uri()))
        });
        let peers_session: Handle<HubSession> = session.clone();
        let prepare_workers = peers.and_then(move |peers| {
            let spec = CreateSession {
                env_type: "hd".to_string(),
                image: Image {
                    url: "http://52.31.143.91/images/x86_64/linux/gu-blender.hdi".to_string(),
                    hash: "SHA1:213fad4e020ded42e6a949f61cb660cb69bc9845".to_string()
                },
                name: "".to_string(),
                tags: vec!["gu:render".into(), "gu:blender".into()],
                note: None,
                options: ()
            };

            peers_session
                .add_peers(peers.map(|p| p.node_id))
                .and_then(move |peers| {
                    futures::future::join_all(peers.into_iter().map(move |node_id| peers_session.peer(node_id).new_session(spec.clone())))
                })
        });

        prepare_workers
            .join(upload_blob)
            .and_then(move |(workers, blob_uri)| {
                eprintln!("workers={:?}, blob_id={:?}", workers, blob_uri);
                use futures::unsync::mpsc;

                let workers = futures::future::join_all(workers.into_iter().map(move |worker : PeerSession| worker.update(vec![Command::DownloadFile {
                    uri: blob_uri.clone(),
                    file_path: "resources".into(),
                    format: ResourceFormat::Tar
                }]).and_then(|_| Ok(worker))));

                workers.and_then(move |workers| {
                    // Scene downloaded to nodes.

                    eprintln!("workers={:?}", workers);
                    drop(session);
                    Ok(())
                })
            })
            .map_err(|e| Error::Other(format!("{}", e)))
        //Ok(())
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
