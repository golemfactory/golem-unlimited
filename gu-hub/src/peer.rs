
use actix::prelude::*;
use gu_actix::prelude::*;
use futures::prelude::*;
use serde::{Serialize, Deserialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;
use gu_base::{App, Decorator, Module, SubCommand, LogModule};
use gu_p2p::NodeId;
use std::any::*;
use actix_web::{self, Scope, Responder, HttpRequest, HttpResponse, AsyncResponder, http};

pub struct PeerModule;

impl Module for PeerModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(SubCommand::with_name("peer"))
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        let log : Option<&LogModule> =  decorator.extract();

        match log {
            Some(l) => eprintln!("have log verb {}", l.verbosity()),
            None => eprintln!("no log module")
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/peer", scope)
    }
}

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope.route("", http::Method::GET, list_peers)
        .route("/send-to", http::Method::POST, peer_send)
}

fn list_peers<S>(r: HttpRequest<S>) -> impl Responder {
    use gu_p2p::rpc::peer::*;

    PeerManager::from_registry().send(ListPeers)
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res| {
            //debug!("res={:?}", res);
            Ok(HttpResponse::Ok().json(res))
        })
        .responder()
}

#[derive(Serialize, Deserialize)]
struct SendMessage {
    node_id : NodeId,
    destination_id : u32,
    body : JsonValue
}

fn peer_send(r: actix_web::Json<SendMessage>) -> impl Responder {
    use gu_p2p::rpc::public_destination;
    use gu_p2p::rpc::reply::*;

    ReplyRouter::from_registry().send(CallRemoteUntyped(r.node_id, public_destination(r.destination_id), r.body.clone()))
        .flatten_fut()
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("err: {}", e)))
        .and_then(|res| {
            Ok(HttpResponse::Ok().json(res))
        })
        .responder()
}
