use actix::prelude::*;
use futures::prelude::*;
use gu_actix::prelude::*;

use super::super::NodeId;
use super::context::RemotingContext;
use super::gen_destination_id;
use super::message::{
    DestinationId, EmitMessage, MessageId, RouteMessage, TransportError, TransportResult,
};
use super::router::{BindReplyDestination, LocalReplyEndpoint, MessageRouter};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json;

use futures::unsync::oneshot;
use std::collections::HashMap;
use std::error::Error;
use std::{fmt, io};

#[derive(Debug)]
pub enum SendError {
    GenBody(Box<Error + Send>),
    ParseBody(Option<Box<Error + Send>>, String),
    MailBox(MailboxError),
    NoDestination,
    Canceled,
}

impl SendError {
    #[inline]
    fn body<E: Error + Send + 'static>(e: E) -> Self {
        SendError::GenBody(Box::new(e))
    }

    fn parse_body<E: Error + Send + 'static>(e: E, body: String) -> Self {
        SendError::ParseBody(Some(Box::new(e)), body)
    }
}

impl Error for SendError {
    fn description(&self) -> &str {
        match self {
            SendError::Canceled => "canceled",
            SendError::GenBody(e) => e.description(),
            SendError::ParseBody(Some(e), _) => e.description(),
            SendError::ParseBody(None, _) => "remote parse error",
            SendError::MailBox(e) => "mailbox error",
            SendError::NoDestination => "no destination",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            SendError::GenBody(e) => Some(e.as_ref()),
            SendError::ParseBody(Some(e), _) => Some(e.as_ref()),
            _ => None,
        }
    }
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SendError::GenBody(e) => write!(f, "gen body error: {}", e),
            SendError::ParseBody(Some(e), b) => write!(f, "parse response {} :: {}", e, b),
            SendError::ParseBody(None, b) => write!(f, "parse request :: {}", b),
            SendError::MailBox(e) => write!(f, "mailbox {}", e),
            SendError::Canceled => write!(f, "canceled"),
            SendError::NoDestination => write!(f, "no destination"),
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
    reply_map: HashMap<MessageId, oneshot::Sender<RouteMessage<Result<String, TransportError>>>>,
}

impl LocalReplyEndpoint for Addr<ReplyRouter> {
    fn handle(
        &mut self,
        message: RouteMessage<Result<String, TransportError>>,
        ctx: &mut <MessageRouter as Actor>::Context,
    ) {
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
        self.router.do_send(BindReplyDestination {
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

impl Handler<RouteMessage<Result<String, TransportError>>> for ReplyRouter {
    type Result = ();

    fn handle(
        &mut self,
        msg: RouteMessage<Result<String, TransportError>>,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        debug!("got message to route: {:?}", msg);
        let msg_id = msg.msg_id.clone();
        if let Some(tx) = self
            .reply_map
            .remove(&msg.correlation_id.clone().unwrap_or_else(|| msg_id))
        {
            tx.send(msg);
        } else {
            warn!("unhandled message");
            debug!("keys: {:?}", self.reply_map);
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

fn parse_body<T: DeserializeOwned>(
    msg_body: Result<String, TransportError>,
) -> Result<Result<T, SendError>, SendError> {
    let body = match msg_body {
        Ok(msg) => msg,
        Err(TransportError::NoDestination) => return Err(SendError::NoDestination),
        Err(TransportError::BadFormat(msg)) => return Err(SendError::ParseBody(None, msg)),
    };

    Ok(match serde_json::from_str(body.as_ref()) {
        Ok(t) => Ok(t),
        Err(e) => Err(SendError::parse_body(e, body)),
    })
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
                    reply_to: Some(self.destination_id.clone()),
                    expires: None,
                    body: TransportResult::Request(body),
                }).flatten_fut()
                .map_err(|e| SendError::body(e))
                .into_actor(self)
                .and_then(|msg_id, act, ctx| {
                    use futures::unsync::oneshot;
                    let (tx, rx) = oneshot::channel();
                    act.reply_map.insert(msg_id, tx);

                    rx.map_err(|_| SendError::Canceled)
                        .and_then(|route_msg: RouteMessage<Result<String, TransportError>>| {
                            parse_body(route_msg.body)
                        }).flatten_fut()
                        .into_actor(act)
                }),
        )
    }
}

pub struct CallRemoteUntyped(pub NodeId, pub DestinationId, pub serde_json::Value);

impl Message for CallRemoteUntyped {
    type Result = Result<serde_json::Value, SendError>;
}

impl Handler<CallRemoteUntyped> for ReplyRouter {
    type Result = ActorResponse<ReplyRouter, serde_json::Value, SendError>;

    fn handle(&mut self, msg: CallRemoteUntyped, ctx: &mut Self::Context) -> Self::Result {
        let body = match serde_json::to_string(&msg.2) {
            Ok(b) => b,
            Err(e) => return ActorResponse::reply(Err(SendError::body(e))),
        };
        use rand::*;
        use smallvec::SmallVec;
        let cid: [u8; 8] = thread_rng().gen();

        use futures::unsync::oneshot;

        let (tx, rx) = oneshot::channel();
        self.reply_map.insert(cid.into(), tx);

        ActorResponse::async(
            MessageRouter::from_registry()
                .send(EmitMessage {
                    dest_node: msg.0,
                    destination: msg.1,
                    correlation_id: Some(SmallVec::from_slice(&cid)),
                    reply_to: Some(self.destination_id.clone()),
                    ts: 0,
                    expires: None,
                    body: TransportResult::Request(body),
                }).flatten_fut()
                .map_err(|e| SendError::body(e))
                .into_actor(self)
                .and_then(move |msg_id, act, ctx| {
                    rx.map_err(|_| SendError::Canceled)
                        .and_then(|route_msg| parse_body(route_msg.body))
                        .flatten_fut()
                        .into_actor(act)
                }),
        )
    }
}
