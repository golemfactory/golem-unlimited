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
use gu_model::envman::Command;
use gu_model::envman::CreateSession;
use gu_model::envman::Image;
use gu_model::envman::ResourceFormat;
use gu_model::peers::PeerInfo;
use gu_model::session::HubExistingSession;
use gu_model::session::HubSessionSpec;
use gu_model::session::SessionDetails;
use gu_net::NodeId;
use serde::Serialize;
use serde_derive::*;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Stdout;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{fs, io, thread};
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
    #[structopt(
        long,
        short,
        parse(try_from_str = "parse_frame_range"),
        raw(required = "true")
    )]
    frame: Vec<Vec<FrameRange>>,
    /// Render using docker image
    #[structopt(long)]
    docker: bool,
    #[structopt(short, long, parse(try_from_str = "parse_resolution"))]
    resolution: (u32, u32),
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,
    #[structopt(name = "RESOURCE", parse(from_os_str), raw(required = "true"))]
    resource: Vec<PathBuf>,
}

impl Render {
    fn frames(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .frame
            .clone()
            .into_iter()
            .map(|v| v.into_iter().flatten())
            .flatten()
            .collect();
        v.sort_unstable();
        v.dedup();

        v
    }
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

#[derive(Debug)]
enum Progress {
    Step(u64, u64),
    Done,
}

#[derive(Debug)]
enum MainSteps {
    Init {
        total_frames: u64,
    },
    UploadFile {
        file_name: PathBuf,
        total_upload_progress: Progress,
    },
    Frame(u32),
    Done,
}

impl Message for MainSteps {
    type Result = ();
}

struct AsyncProgress {
    mb: Option<pbr::MultiBar<Stdout>>,
    main_bar: pbr::ProgressBar<pbr::Pipe>,
    upload_bar: pbr::ProgressBar<pbr::Pipe>,
}

impl AsyncProgress {
    fn new(frames: u64, resources: u64) -> Self {
        let mut mb = pbr::MultiBar::new();
        let upload_bar = mb.create_bar(resources);
        let main_bar = mb.create_bar(frames + 1);

        AsyncProgress {
            mb: Some(mb),
            main_bar,
            upload_bar,
        }
    }
}

impl Actor for AsyncProgress {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let mut mb = self.mb.take().unwrap();
        thread::spawn(move || mb.listen());
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {}
}

impl Handler<MainSteps> for AsyncProgress {
    type Result = ();

    fn handle(&mut self, msg: MainSteps, ctx: &mut Self::Context) -> Self::Result {
        match msg {
            MainSteps::Init { total_frames } => (),
            MainSteps::UploadFile {
                file_name,
                total_upload_progress: Progress::Step(s, _),
            } => {
                self.upload_bar
                    .message(&format!("upload [{}] ", file_name.display()));
                self.upload_bar.set(s);
            }
            MainSteps::UploadFile {
                file_name,
                total_upload_progress: Progress::Done,
            } => {
                self.upload_bar.finish_println("upload done");
                self.main_bar.add(1);
            }
            MainSteps::Frame(frame) => {
                self.main_bar.message(&format!("[frame {}]:  ", frame));
                self.main_bar.add(1);
            }
            MainSteps::Done => {
                self.main_bar.finish_println("rendering done");
            }
            e => {
                //eprintln!("!!event={:?}", e);
            }
        }
    }
}

fn parse_resolution(r: &str) -> Result<(u32, u32), failure::Error> {
    let mut it = r.split("x");

    Ok(match (it.next(), it.next(), it.next()) {
        (Some(w), Some(h), None) => (w.parse()?, h.parse()?),
        _ => Err(Error::Other(format!("invalid format {}", r)))?,
    })
}

#[derive(Debug, Clone)]
enum FrameRange {
    Single(u32),
    Range { from: u32, to: u32, step_by: u32 },
}

impl Iterator for FrameRange {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        let (item, next) = match self {
            FrameRange::Single(v) => (
                Some(*v),
                FrameRange::Range {
                    from: *v + 1,
                    to: *v,
                    step_by: 1,
                },
            ),
            FrameRange::Range { from, to, step_by } => {
                if from <= to {
                    (
                        Some(*from),
                        FrameRange::Range {
                            from: (*from) + (*step_by),
                            to: (*to),
                            step_by: *step_by,
                        },
                    )
                } else {
                    (
                        None,
                        FrameRange::Range {
                            from: *from,
                            to: *to,
                            step_by: *step_by,
                        },
                    )
                }
            }
        };
        *self = next;
        item
    }
}

fn parse_frame_range(r: &str) -> Result<Vec<FrameRange>, failure::Error> {
    let mut it = r.chars().peekable();
    let mut r = Vec::new();

    fn peek_range<T: Iterator<Item = char>>(
        it: &mut Peekable<T>,
    ) -> Result<Option<FrameRange>, Error> {
        let left = match shift_number(it) {
            Some(n) => n,
            None => return Ok(None),
        };
        if it.peek() != Some(&'-') {
            return Ok(Some(FrameRange::Single(left)));
        }
        let _ = it.next();
        let right = match shift_number(it) {
            Some(n) => n,
            None => return Err(Error::Other(format!("expected number"))),
        };
        let step = match it.peek() {
            Some(&'+') => {
                let _ = it.next();
                shift_number(it).ok_or_else(|| Error::Other(format!("expected number")))?
            }
            _ => 1,
        };

        Ok(Some(FrameRange::Range {
            from: left,
            to: right,
            step_by: step,
        }))
    }

    fn shift_number<T: Iterator<Item = char>>(it: &mut Peekable<T>) -> Option<u32> {
        let mut sum = 0u32;
        while let Some(&ch) = it.peek() {
            match ch {
                '0'...'9' => {
                    let _ = it.next();
                    sum = sum * 10 + (ch as u8 - '0' as u8) as u32
                }
                _ => break,
            }
        }
        Some(sum)
    }

    while let Some(range) = peek_range(&mut it)? {
        r.push(range);
        match it.next() {
            None => return Ok(r),
            Some(',') => (),
            Some(ch) => Err(Error::Other(format!("invalid char '{}'", ch)))?,
        }
    }
    Err(Error::Other(format!("missing range desc")))?
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

struct TaskList<T> {
    tasks: Rc<RefCell<T>>,
}

impl<T> Clone for TaskList<T> {
    fn clone(&self) -> Self {
        TaskList {
            tasks: self.tasks.clone(),
        }
    }
}

impl<T: Iterator> TaskList<T> {
    fn new<K: IntoIterator<IntoIter = T, Item = T::Item>>(v: K) -> Self {
        TaskList {
            tasks: Rc::new(RefCell::new(v.into_iter())),
        }
    }
}

impl<T: Iterator> Stream for TaskList<T>
where
    T::Item: std::fmt::Debug,
{
    type Item = T::Item;
    type Error = ();

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Self::Error> {
        let mut b = self.tasks.borrow_mut();
        let item = b.next();
        log::debug!("item={:?}", item);
        Ok(Async::Ready(item))
    }
}

#[derive(Serialize, Debug)]
struct BlenderTaskSpec {
    crops: Vec<Crop>,
    samples: u32,
    resolution: (u32, u32),
    frames: Vec<u32>,
    scene_file: Option<String>,
    output_format: String,
}

#[derive(Serialize, Debug)]
struct Crop {
    borders_x: (f64, f64),
    borders_y: (f64, f64),
    outfilebasename: String,
}

fn run_worker<S: Stream<Item = (Blob, BlenderTaskSpec), Error = Error>>(
    session: &PeerSession,
    frame: S,
    output_path: PathBuf,
    logger: Addr<AsyncProgress>,
) -> impl Future<Item = (), Error = Error> {
    let session = session.clone();

    frame
        .for_each(move |(blob, spec)| {
            let result_blob = blob.clone();
            let &frame = spec.frames.first().unwrap();
            let outf = format!("output/outf_{:04}.png", frame);
            let output_path = output_path.join(format!("frame_{:04}.png", frame));
            let logger = logger.clone();

            session
                .update(vec![
                    Command::WriteFile {
                        file_path: "resources/spec.json".to_owned(),
                        content: serde_json::to_string(&spec).unwrap(),
                    },
                    Command::Exec {
                        executable: "./gu-render".into(),
                        args: Vec::new(),
                    },
                    Command::UploadFile {
                        uri: blob.uri(),
                        file_path: outf.clone(),
                        format: ResourceFormat::Raw,
                    },
                ])
                .and_then(move |results| {
                    use std::io::prelude::*;
                    log::debug!(
                        "results={:?}, output_file={}",
                        results,
                        output_path.display()
                    );
                    let mut f = match fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(output_path)
                    {
                        Ok(f) => f,
                        Err(err) => return future::Either::B(future::err(err.into())),
                    };

                    future::Either::A(result_blob.download().for_each(move |b| {
                        f.write_all(b.as_ref())?;
                        Ok(())
                    }))
                })
                .and_then(move |_| {
                    logger.do_send(MainSteps::Frame(frame));
                    Ok(())
                })
        })
        .and_then(|_| Ok(log::debug!("done")))
}

struct TarBuildHelper<W: std::io::Write> {
    b: tar::Builder<W>,
    dirs: BTreeSet<PathBuf>,
}

impl<W: std::io::Write> TarBuildHelper<W> {
    fn new(b: tar::Builder<W>) -> Self {
        let mut dirs = BTreeSet::new();

        dirs.insert(PathBuf::new());

        TarBuildHelper { b, dirs }
    }

    fn add_dir(&mut self, d: &Path) -> io::Result<()> {
        use tar::{EntryType, Header};

        log::debug!("dir={:?}", d);
        if self.dirs.contains(d) {
            return Ok(());
        }
        if let Some(parent) = d.parent() {
            self.add_dir(parent)?;
        } else {
            return Ok(());
        }

        self.dirs.insert(d.to_owned());
        let mut h = Header::new_ustar();
        h.set_entry_type(EntryType::Directory);
        h.set_mode(0o644);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(0);
        h.set_size(0);
        h.set_path(d)?;
        h.set_cksum();
        let data2 = io::empty();
        self.b.append(&h, data2)?;
        Ok(())
    }

    pub fn add_file(&mut self, path: &Path, file: &mut fs::File) -> io::Result<()> {
        log::trace!("add file={:?}", path);
        if let Some(dir) = path.parent() {
            self.add_dir(dir)?;
        }
        self.b.append_file(path, file)
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.b.finish()
    }
}

fn render_task(
    connection: &HubConnection,
    opts: Render,
) -> Box<dyn Future<Item = (), Error = gu_client::error::Error>> {
    use gu_model::chrono::prelude::*;
    let frames = opts.frames();

    let mut mb = AsyncProgress::new(frames.len() as u64, opts.resource.len() as u64).start();

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

    let base_path = match common_path::common_path_all(resources.iter().filter_map(|p| {
        if p.is_file() {
            p.parent()
        } else {
            Some(p.as_path())
        }
    })) {
        Some(path) => path,
        None => {
            return Box::new(future::err(Error::Other(format!(
                "unable to find common path for: {:?}",
                resources
            ))));
        }
    };

    let scene_file = match resources.iter().find(|&file_name| {
        file_name
            .extension()
            .map(|ext| ext == "blend")
            .unwrap_or(false)
    }) {
        Some(scene_file) => scene_file.strip_prefix(&base_path).unwrap().to_owned(),
        None => return Box::new(future::err(Error::Other(format!("missing .blend file")))),
    };

    log::debug!("base={:?}, t={:?}", base_path, resources);
    let (tx, rx) = pipe::sync_to_async(5);

    let mut work_resources = resources.clone();
    let work_base = base_path.clone();
    let mut upload_resources = mb.clone();

    thread::spawn(move || {
        use std::io::prelude::*;
        use tar::*;

        match (|| -> Fallible<()> {
            work_resources.sort();
            let mut builder = TarBuildHelper::new(Builder::new(tx));
            let mut pos = 0;
            let total = work_resources.len();

            for res in work_resources {
                let rel_name = res.strip_prefix(&work_base)?;

                let mut f = fs::OpenOptions::new().write(false).read(true).open(&res)?;
                builder.add_file(rel_name, &mut f)?;
                pos += 1;
                upload_resources.do_send(MainSteps::UploadFile {
                    file_name: rel_name.to_owned(),
                    total_upload_progress: Progress::Step(pos, total as u64),
                });
            }
            builder.finish()?;
            Ok(())
        })() {
            Ok(()) => (),
            Err(e) => {
                eprintln!("fail to generate tar: {}", e);
            }
        }
        upload_resources.do_send(MainSteps::UploadFile {
            file_name: "done".into(),
            total_upload_progress: Progress::Done,
        });

        log::debug!("data uploaded");
    });

    let resolution = opts.resolution;

    if let Some(output) = opts.output.as_ref() {
        if !output.exists() {
            fs::create_dir_all(output).unwrap();
        }
    }
    let output = opts.output.unwrap_or_else(|| "./".into());

    let tasks = frames
        .clone()
        .into_iter()
        .map(move |frame| BlenderTaskSpec {
            crops: vec![Crop {
                borders_x: (0.0, 1.0),
                borders_y: (0.0, 1.0),
                outfilebasename: "outf_".to_string(),
            }],
            samples: 0,
            resolution: resolution,
            frames: vec![frame],
            scene_file: Some(format!("{}", scene_file.display())),
            output_format: "PNG".to_string(),
        });

    Box::new(connection.new_session(spec).and_then(move |session| {
        // Work
        // 1. Upload resources to session
        // 2. Create deployments on peer
        // 3. Run processing
        // 4. upload results
        let upload_blob = session.new_blob().and_then(|blob: Blob| {
            log::debug!("new_blob={}", blob.id());
            blob.upload_from_stream(rx)
                .and_then(move |_| Ok(blob.uri()))
        });
        let tasks = future::join_all(
            tasks
                .map(|spec| session.new_blob().and_then(|blob| Ok((blob, spec))))
                .collect::<Vec<_>>(),
        )
        .and_then(|t| Ok(TaskList::new(t)));

        let peers_session: Handle<HubSession> = session.clone();
        let prepare_workers =
            peers.and_then(move |peers| {
                let spec = CreateSession {
                    env_type: "hd".to_string(),
                    image: Image {
                        url: "http://52.31.143.91/images/x86_64/linux/gu-blender.hdi".to_string(),
                        hash: "SHA1:213fad4e020ded42e6a949f61cb660cb69bc9845".to_string(),
                    },
                    name: "".to_string(),
                    tags: vec!["gu:render".into(), "gu:blender".into()],
                    note: None,
                    options: (),
                };

                peers_session
                    .add_peers(peers.map(|p| p.node_id))
                    .and_then(move |peers| {
                        futures::future::join_all(peers.into_iter().map(move |node_id| {
                            peers_session.peer(node_id).new_session(spec.clone())
                        }))
                    })
            });

        let mut frames_done = 0;

        prepare_workers
            .join3(upload_blob, tasks)
            .and_then(move |(workers, blob_uri, tasks)| {
                log::debug!("workers={:?}, blob_id={:?}", workers, blob_uri);
                use futures::unsync::mpsc;

                let workers = futures::future::join_all(workers.into_iter().map(
                    move |worker: PeerSession| {
                        worker
                            .update(vec![Command::DownloadFile {
                                uri: blob_uri.clone(),
                                file_path: "resources".into(),
                                format: ResourceFormat::Tar,
                            }])
                            .and_then(|_| Ok(worker))
                    },
                ));
                let mb = mb.clone();
                let mbf = mb.clone();

                workers
                    .and_then(move |workers| {
                        // Scene downloaded to nodes.
                        futures::future::join_all(workers.into_iter().map(move |w| {
                            run_worker(
                                &w,
                                tasks.clone().map_err(|_| unreachable!()),
                                output.clone(),
                                mb.clone(),
                            )
                        }))
                        .and_then(|_| {
                            log::debug!("work done");
                            drop(session);
                            Ok(())
                        })
                    })
                    .and_then(move |_| {
                        mbf.do_send(MainSteps::Done);
                        Ok(())
                    })
            })
            .map_err(|e| {
                eprintln!("error = {}", e);
                Error::Other(format!("{}", e))
            })
        //Ok(())
    }))
}

fn main() -> Fallible<()> {
    env_logger::init();

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
