use super::Module;
use clap::{App, Arg, ArgMatches, Shell, SubCommand};
use env_logger;
use std::env;

pub struct LogModule;

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
            env::set_var(
                "RUST_LOG",
                "*=info,gu_p2p=debug,gu_provider=debug,gu_hub=debug",
            )
        }
        env_logger::init();
        debug!("debug");
        false
    }
}

pub struct CompleteModule(String);

impl CompleteModule {
    pub fn new() -> CompleteModule {
        let a: String = env::args().take(1).into_iter().next().unwrap().into();
        CompleteModule(a)
    }
}

impl Module for CompleteModule {
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

    fn args_complete<F>(&self, matches: &ArgMatches, app_gen: &F) -> bool
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
