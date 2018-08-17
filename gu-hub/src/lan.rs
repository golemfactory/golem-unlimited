use clap::{App, Arg, ArgMatches, SubCommand};

pub fn clap_declare<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("lan").subcommand(SubCommand::with_name("list"))
}

pub fn clap_match(m: &ArgMatches) {
    if let Some(m) = m.subcommand_matches("lan") {
        clap_match_lan(m)
    }
}

fn clap_match_lan(m: &ArgMatches) {
    if let Some(m) = m.subcommand_matches("list") {
        unimplemented!("list")
    } else {
        println!("{}", m.usage())
    }
}
