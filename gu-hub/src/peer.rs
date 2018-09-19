use actix::prelude::*;
use actix_web::{self, http, AsyncResponder, HttpRequest, HttpResponse, Responder, Scope};
use futures::prelude::*;
use gu_actix::prelude::*;
use gu_base::cli;
use gu_base::{App, ArgMatches, Decorator, LogModule, Module, SubCommand};
use gu_p2p::rpc::peer::PeerInfo;
use gu_p2p::NodeId;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::any::*;

use prettytable::{self, cell::Cell, row::Row, Table};

pub struct PeerModule {
    inner: State,
}

enum State {
    None,
    List,
}

impl PeerModule {
    pub fn new() -> Self {
        PeerModule { inner: State::None }
    }
}

impl Module for PeerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("peer").subcommand(SubCommand::with_name("list")))
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(m) = matches.subcommand_matches("peer") {
            if let Some(m) = m.subcommand_matches("list") {
                self.inner = State::List;
                return true;
            }
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        match self.inner {
            State::None => return (),
            State::List => {
                use super::server::ServerClient;

                System::run(|| {
                    Arbiter::spawn(
                        ServerClient::get("/peer".into())
                            .and_then(|r: Vec<PeerInfo>| Ok(format_peer_table(r)))
                            .map_err(|e| error!("{}", e))
                            .then(|r| Ok(System::current().stop())),
                    )
                });
            }
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/peer", scope)
    }
}

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope
        .route("", http::Method::GET, list_peers)
        .route("/send-to", http::Method::POST, peer_send)
}

fn list_peers<S>(r: HttpRequest<S>) -> impl Responder {
    use gu_p2p::rpc::peer::*;

    PeerManager::from_registry()
        .send(ListPeers)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res| {
            //debug!("res={:?}", res);
            Ok(HttpResponse::Ok().json(res))
        }).responder()
}

#[derive(Serialize, Deserialize)]
struct SendMessage {
    node_id: NodeId,
    destination_id: u32,
    body: JsonValue,
}

fn peer_send(r: actix_web::Json<SendMessage>) -> impl Responder {
    use gu_p2p::rpc::public_destination;
    use gu_p2p::rpc::reply::*;

    ReplyRouter::from_registry()
        .send(CallRemoteUntyped(
            r.node_id,
            public_destination(r.destination_id),
            r.body.clone(),
        )).flatten_fut()
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
        .responder()
}

fn format_peer_table(peers: Vec<PeerInfo>) {
    let mut table = Table::new();
    table.set_titles(row!["Node id", "Name", "Connection", "Sessions"]);
    for peer in peers {
        table.add_row(row![
            peer.node_id,
            peer.node_name,
            peer.peer_addr.unwrap_or_else(|| String::default()),
            peer.sessions.len()
        ]);
    }

    table.set_format(*cli::FORMAT_BASIC);
    table.printstd()
}
