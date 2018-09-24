use super::Module;
use clap::{App, Arg, ArgMatches, Shell, SubCommand};
use env_logger;
use std::env;

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
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if env::var("RUST_LOG").is_err() {
            match matches.occurrences_of("v") {
                0 => env::set_var("RUST_LOG", "error"),
                1 => env::set_var("RUST_LOG", "info"),
                _ => env::set_var(
                    "RUST_LOG",
                    "info,gu_p2p=debug,gu_provider=debug,gu_hub=debug",
                ),
            }
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
            let prg = self.0.as_ref();
            app_gen().gen_completions_to(prg, shell.parse().unwrap(), &mut io::stdout());
            return true;
        }
        false
    }
}
