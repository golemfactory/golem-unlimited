use actix::fut;
use actix::prelude::*;
use actix_web::{self, *};
use futures::{future, prelude::*};
use gu_actix::*;

use super::super::NodeId;
use super::error::ErrorKind;
use super::{
    gen_destination_id, public_destination, DestinationId, EmitMessage, MessageId, MessageRouter,
    RouteMessage, RpcError,
};
use futures::unsync::oneshot;
use std::collections::HashMap;

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope.route("", http::Method::GET, mock_base).route(
        "/{destinationId}",
        http::Method::POST,
        mock_send,
    )
}

static MOCK_HTML: &str = include_str!("../../html/mock.html");

fn mock_base<S>(r: HttpRequest<S>) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(MOCK_HTML)
}

fn mock_send<S: 'static>(r: HttpRequest<S>, path: Path<(u32,)>) -> impl Responder {
    r.body()
        .map_err(|e| error::ErrorInternalServerError(format!("{}", e)))
        .and_then(|body| {
            String::from_utf8(body.as_ref().into())
                .map_err(|e| error::ErrorInternalServerError(format!("{}", e)))
        })
        .and_then(move |body| {
            Callback::from_registry()
                .send(Forward(RouteMessage {
                    msg_id: gen_destination_id(),
                    sender: NodeId::default(),
                    destination: public_destination(path.0),
                    reply_to: None,
                    correlation_id: None,
                    ts: 0,
                    expires: None,
                    body,
                }))
                .flatten_fut()
                .and_then(|b| Ok(HttpResponse::Ok().body(b)))
                .map_err(|e| error::ErrorInternalServerError(format!("{}", e)))
        })
        .and_then(|r| future::ok(r))
        .or_else(
            |e: actix_web::Error| -> Result<HttpResponse, actix_web::Error> {
                debug!("Error {:?}", &e);

                let mut resp = e.as_response_error().error_response();
                resp.set_body(format!("{}", e));
                Ok(resp)
            },
        )
        .responder()
}

struct Callback {
    reply: HashMap<MessageId, oneshot::Sender<String>>,
    fake_node_id: NodeId,
    tx_map: HashMap<DestinationId, oneshot::Sender<Result<String, RpcError>>>,
}

impl Actor for Callback {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        info!("Callback actor started")
    }
}

impl ArbiterService for Callback {
    fn service_started(&mut self, ctx: &mut Context<Self>) {
        use super::router::*;
        info!("Callback service started");
        MessageRouter::from_registry().do_send(AddEndpoint {
            node_id: self.fake_node_id.clone(),
            recipient: ctx.address().recipient(),
        })
    }
}

impl Handler<EmitMessage<String>> for Callback {
    type Result = Result<MessageId, RpcError>;

    fn handle(
        &mut self,
        msg: EmitMessage<String>,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<EmitMessage<String>>>::Result {
        info!("emit={:?}", &msg.body);
        if let Some(tx) = self.tx_map.remove(&msg.destination) {
            tx.send(msg.body.into()).unwrap();
        }

        Ok(gen_destination_id())
    }
}

impl Supervised for Callback {}

impl Default for Callback {
    fn default() -> Self {
        use rand::prelude::*;
        let mut rng = thread_rng();

        Callback {
            reply: HashMap::new(),
            fake_node_id: rng.gen(),
            tx_map: HashMap::new(),
        }
    }
}

struct Forward(RouteMessage<String>);

impl Message for Forward {
    type Result = Result<String, RpcError>;
}

impl Handler<Forward> for Callback {
    type Result = ActorResponse<Callback, String, RpcError>;

    fn handle(
        &mut self,
        mut msg: Forward,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<Forward>>::Result {
        msg.0.sender = self.fake_node_id;
        msg.0.reply_to = Some(gen_destination_id());
        let (tx, rx) = oneshot::channel();
        self.tx_map.insert(msg.0.reply_to.clone().unwrap(), tx);
        MessageRouter::from_registry().do_send(msg.0);
        ActorResponse::async(rx.flatten_fut().into_actor(self))
    }
}
