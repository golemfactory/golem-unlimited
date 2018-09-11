use actix_web;
use gu_base::{App, Decorator, Module, SubCommand};
use gu_p2p::NodeId;
use std::any::*;

pub struct PeerModule;

fn p2p_server<S>(_r: &actix_web::HttpRequest<S>) -> &'static str {
    "ok"
}

impl Module for PeerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("peer"))
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        eprintln!("decorate  peer");
        app.handler("/peer", p2p_server)
    }
}
