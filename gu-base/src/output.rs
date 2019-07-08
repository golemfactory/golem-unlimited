use super::Module;
use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use env_logger;
use std::env;
use std::sync::atomic::{AtomicIsize, Ordering};

static LISTING_FORMAT: AtomicIsize = AtomicIsize::new(0);

#[derive(Clone, Copy)]
pub(crate) enum ListingFormat {
    Json,
    Table,
}

impl From<isize> for ListingFormat {
    fn from(v: isize) -> Self {
        match v {
            0 => ListingFormat::Table,
            1 => ListingFormat::Json,
            _ => panic!("invalid ListingFormat value: {}", v),
        }
    }
}

impl ListingFormat {
    #[inline]
    fn as_int(&self) -> isize {
        match self {
            ListingFormat::Table => 0,
            ListingFormat::Json => 1,
        }
    }
}

pub(crate) fn listing_format() -> ListingFormat {
    LISTING_FORMAT.load(Ordering::Relaxed).into()
}

pub struct LogModule;

impl LogModule {
    pub fn verbosity(&self) -> isize {
        0
    }
}

impl Module for LogModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("json")
                .long("json")
                .help("Sets the output format to json"),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if env::var("RUST_LOG").is_err() {
            match matches.occurrences_of("v") {
                0 => env::set_var("RUST_LOG", "error"),
                1 => env::set_var("RUST_LOG", "info"),
                2 => env::set_var(
                    "RUST_LOG",
                    "info,gu_net=debug,gu_provider=debug,gu_hub=debug,gu_event_bus=debug",
                ),
                _ => env::set_var("RUST_LOG", "debug"),
            }
        }
        if matches.is_present("json") {
            LISTING_FORMAT.store(ListingFormat::Json.as_int(), Ordering::Relaxed);
        }
        env_logger::init();
        false
    }
}

pub struct AutocompleteModule(String);

impl AutocompleteModule {
    pub fn new() -> AutocompleteModule {
        let shell: String = env::args().take(1).into_iter().next().unwrap().into();
        AutocompleteModule(shell)
    }
}

impl Module for AutocompleteModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(
            SubCommand::with_name("completions")
                .about("Generates completion scripts for your shell")
                .setting(AppSettings::ArgRequiredElseHelp)
                .arg(
                    Arg::with_name("SHELL")
                        .required(true)
                        .possible_values(&["bash", "fish", "zsh"])
                        .help("The shell to generate the script for"),
                ),
        )
    }

    fn args_autocomplete<F>(&self, matches: &ArgMatches, app_gen: &F) -> bool
    where
        F: Fn() -> App<'static, 'static>,
    {
        use std::io;

        if let Some(sub_matches) = matches.subcommand_matches("completions") {
            let shell = sub_matches.value_of("SHELL").unwrap();
            let prg : &str = self.0.as_ref();
            app_gen().gen_completions_to(prg, shell.parse().unwrap(), &mut io::stdout());
            return true;
        }
        false
    }
}
