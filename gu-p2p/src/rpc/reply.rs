use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;

use super::super::NodeId;
use super::context::RemotingContext;
use super::gen_destination_id;
use super::message::{
    DestinationId, EmitMessage, MessageId, RouteMessage, TransportError, TransportResult,
};
use super::router::{MessageRouter, BindDestination, LocalEndpoint};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;

use futures::unsync::oneshot;
use std::collections::HashMap;
use std::error::Error;
use std::{fmt, io};

#[derive(Debug)]
pub enum SendError {
    GenBody(Box<Error>),
    ParseBody(Box<Error>, String),
    MailBox(MailboxError),
    Canceled,
}

impl SendError {
    #[inline]
    fn body<E: Error + 'static>(e: E) -> Self {
        SendError::GenBody(Box::new(e))
    }

    fn parse_body<E: Error + 'static>(e: E, body: String) -> Self {
        SendError::ParseBody(Box::new(e), body)
    }
}

impl Error for SendError {
    fn description(&self) -> &str {
        match self {
            SendError::Canceled => "canceled",
            SendError::GenBody(e) => e.description(),
            SendError::ParseBody(e, _) => e.description(),
            SendError::MailBox(e) => "mailbox error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            SendError::GenBody(e) => Some(e.as_ref()),
            SendError::ParseBody(e, _) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SendError::GenBody(e) => write!(f, "gen body error: {}", e),
            SendError::ParseBody(e, b) => write!(f, "parse response {} :: {}", e, b),
            SendError::MailBox(e) => write!(f, "mailbox {}", e),
            SendError::Canceled => write!(f, "canceled"),
        }
    }
}

impl From<MailboxError> for SendError {
    fn from(e: MailboxError) -> Self {
        SendError::MailBox(e)
    }
}

pub struct ReplyRouter {
    router: Addr<MessageRouter>,
    destination_id: DestinationId,
    reply_map: HashMap<MessageId, oneshot::Sender<RouteMessage<String>>>,
}

impl LocalEndpoint for Addr<ReplyRouter> {
    fn handle(&mut self, message: RouteMessage<String>, ctx: &mut <MessageRouter as Actor>::Context) {
        self.do_send(message)
    }
}

impl Actor for ReplyRouter {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {}
}

impl Supervised for ReplyRouter {}

impl ArbiterService for ReplyRouter {

    fn service_started(&mut self, ctx: &mut Context<Self>) {
        self.router.do_send(BindDestination {
            destination_id: self.destination_id.clone(),
            endpoint: Box::new(ctx.address()),
        })
    }
}

impl Default for ReplyRouter {
    fn default() -> Self {
        info!("new router");
        ReplyRouter {
            router: MessageRouter::from_registry(),
            destination_id: gen_destination_id(),
            reply_map: HashMap::new(),
        }
    }
}

impl Handler<RouteMessage<String>> for ReplyRouter {
    type Result = ();

    fn handle(&mut self, msg: RouteMessage<String>, ctx: &mut Self::Context) -> Self::Result {
        if let Some(tx) = self.reply_map.remove(&msg.msg_id) {
            tx.send(msg);
        }
    }
}

pub struct CallRemote<T>(NodeId, DestinationId, T)
where
    T: Message;

impl<T> Message for CallRemote<T>
where
    T: Message,
{
    type Result = Result<T::Result, SendError>;
}

impl<T> Handler<CallRemote<T>> for ReplyRouter
where
    T: Serialize + Message,
    T::Result: DeserializeOwned,
{
    type Result = ActorResponse<ReplyRouter, T::Result, SendError>;

    fn handle(
        &mut self,
        msg: CallRemote<T>,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<CallRemote<T>>>::Result {
        let body = match serde_json::to_string(&msg.2) {
            Ok(b) => b,
            Err(e) => return ActorResponse::reply(Err(SendError::body(e))),
        };

        ActorResponse::async(
            MessageRouter::from_registry()
                .send(EmitMessage {
                    dest_node: msg.0,
                    destination: msg.1,
                    correlation_id: None,
                    ts: 0,
                    expires: None,
                    body: TransportResult::Ok(body),
                })
                .flatten_fut()
                .map_err(|e| SendError::body(e))
                .into_actor(self)
                .and_then(|msg_id, act, ctx| {
                    use futures::unsync::oneshot;
                    let (tx, rx) = oneshot::channel();
                    act.reply_map.insert(msg_id, tx);

                    rx.map_err(|_| SendError::Canceled)
                        .and_then(|route_msg: RouteMessage<String>| {
                            Ok(match serde_json::from_str(route_msg.body.as_ref()) {
                                Ok(t) => Ok(t),
                                Err(e) => Err(SendError::parse_body(e, route_msg.body)),
                            })
                        })
                        .flatten_fut()
                        .into_actor(act)
                }),
        )
    }
}
