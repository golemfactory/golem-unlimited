use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use actix::{
    fut::WrapFuture, Actor, ActorResponse, Arbiter, ArbiterService, Context, Handler, Message,
    Supervised, System,
};
use futures::future::{self, Future};
use log::error;
use zip::{write::FileOptions, ZipWriter};

use gu_base::{App, Arg, ArgMatches, SubCommand};

use crate::plugins::{
    plugin::{DirectoryHandler, PluginHandler},
    rest,
};

#[derive(Debug, Clone, Default)]
pub struct PluginBuilder;

impl Actor for PluginBuilder {
    type Context = Context<Self>;
}

impl Supervised for PluginBuilder {}
impl ArbiterService for PluginBuilder {}

#[derive(Debug, Clone)]
pub struct BuildPluginQuery {
    source: String,
    target: String,
    /// Overwrite previous plugin archive on target path
    overwrite: bool,
    /// Install plugin after building
    install: bool,
    // format: Zip
}

impl Message for BuildPluginQuery {
    type Result = Result<(), ()>;
}

impl<'a> From<ArgMatches<'a>> for BuildPluginQuery {
    fn from(matches: ArgMatches<'a>) -> Self {
        Self {
            source: matches.value_of("PATH").unwrap().to_string(),
            target: matches.value_of("TARGET").unwrap().to_string(),
            overwrite: matches.is_present("replace"),
            install: matches.is_present("install"),
        }
    }
}

pub fn subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("build")
        .about("Build a plugin package from the source directory")
        .arg(
            Arg::with_name("PATH")
                .index(1)
                .help("Path to the app directory")
                .default_value("."),
        )
        .arg(
            Arg::with_name("TARGET")
                .index(2)
                .help("Path to the target directory")
                .default_value("."),
        )
        .arg(
            Arg::with_name("replace")
                .short("r")
                .help("Overwrite previous plugin in target directory"),
        )
        .arg(
            Arg::with_name("install")
                .short("i")
                .help("Install the plugin in the gu-hub after build"),
        )
}

fn relative_path(filename: &Path, base: &Path) -> Result<String, String> {
    filename
        .strip_prefix(base)
        .map_err(|_| format!("Cannot calculate relative {:?} path", filename))?
        .to_str()
        .ok_or(format!(
            "Cannot translate relative {:?} path to str",
            filename
        ))
        .map(|s| s.to_string())
}

fn zip_file(zip: &mut ZipWriter<File>, filename: &Path, base: &Path) -> Result<(), String> {
    let relative = relative_path(filename, base)?;

    zip.start_file(relative, FileOptions::default())
        .map_err(|_| format!("Cannot create {:?} file in archive", filename))?;

    let mut file = File::open(filename).map_err(|_| format!("Cannot open {:?} file", filename))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|_| format!("Cannot read whole file {:?}", filename))?;

    zip.write(buf.as_ref())
        .map_err(|_| format!("Cannot write {:?} file in archive", filename))?;

    Ok(())
}

fn add_directory_recursive(
    zip: &mut ZipWriter<File>,
    dir: &Path,
    base: &Path,
) -> Result<(), String> {
    let relative = relative_path(dir, base)?;

    for entry in fs::read_dir(dir).map_err(|_| format!("Cannot read {:?} directory", dir))? {
        let entry = entry.map_err(|_| format!("Cannot read {:?} file", &relative))?;
        let mut filename = PathBuf::from(dir.clone());
        filename.push(entry.file_name());

        if entry
            .file_type()
            .map_err(|_| format!("Cannot get file type of {:?}", entry))?
            .is_dir()
        {
            add_directory_recursive(zip, &filename, base)?;
        } else {
            zip_file(zip, &filename, base)?;
        }
    }

    Ok(())
}

fn build_plugin(msg: BuildPluginQuery) -> Result<PathBuf, String> {
    let source = PathBuf::from(msg.source);
    let parser = DirectoryHandler::new(source.clone())?;
    let metadata = parser.metadata()?;

    let mut target_file = PathBuf::from(msg.target.clone());
    target_file.push(format!("{}.gu-plugin", metadata.name()));

    if target_file.exists() && !msg.overwrite {
        return Err("File exists in target directory".to_string());
    }

    let mut app_dir = source.clone();
    app_dir.push(metadata.name());

    let file = File::create(&target_file).map_err(|_| "Cannot create target file")?;

    let mut writer = ZipWriter::new(file);
    zip_file(&mut writer, &source.join("gu-plugin.json"), &source)?;

    add_directory_recursive(&mut writer, &app_dir, &source)?;

    Ok(target_file)
}

fn install_plugin(path: &PathBuf, install: bool) -> impl Future<Item = (), Error = ()> {
    if install {
        future::Either::A(
            future::result(rest::read_file(path)).and_then(|buf| rest::install_query_inner(buf)),
        )
    } else {
        future::Either::B(future::ok(()))
    }
}

impl Handler<BuildPluginQuery> for PluginBuilder {
    type Result = ActorResponse<PluginBuilder, (), ()>;

    fn handle(
        &mut self,
        msg: BuildPluginQuery,
        _ctx: &mut Context<Self>,
    ) -> <Self as Handler<BuildPluginQuery>>::Result {
        ActorResponse::r#async(
            future::result(build_plugin(msg.clone()))
                .map_err(|e| error!("{}", e))
                .and_then(move |file| install_plugin(&file, msg.install))
                .into_actor(self),
        )
    }
}

pub fn build_query(msg: &BuildPluginQuery) {
    let msg = msg.clone();

    System::run(|| {
        Arbiter::spawn(
            PluginBuilder::from_registry()
                .send(msg)
                .then(|a| a.unwrap_or_else(|_| Err(error!("Mailbox error"))))
                .map_err(|e| error!("{:?}", e))
                .then(|_r| Ok(System::current().stop())),
        )
    });
}
