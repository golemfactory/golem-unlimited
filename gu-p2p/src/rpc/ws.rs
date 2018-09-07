/*
 * Websocket interface for message router
 */

use super::super::proto::wire;
use super::message::{NodeId, RouteMessage, TransportError, TransportResult, MessageId};
use super::router::MessageRouter;
use actix::prelude::*;
use actix_web::{self, ws, HttpRequest, HttpResponse};
use std::marker::PhantomData;
use quick_protobuf::serialize_into_vec;

struct Worker<S: 'static> {
    state: PhantomData<S>,
    node_id : NodeId,
    peer_node_id: Option<NodeId>,
}

impl<S> Worker<S> {
    fn new(node_id : NodeId) -> Self {
        Worker {
            state: PhantomData,
            node_id,
            peer_node_id: None,
        }
    }

    fn route(&mut self, rpc: wire::RpcMessage) {
        let body: String = match (rpc.status, rpc.payload) {
            (wire::RpcStatus::Success, wire::mod_RpcMessage::OneOfpayload::json(json)) => {
                json.into()
            }
            (wire::RpcStatus::NoDestination, _) => {
                //TransportResult::Err(TransportError::NoDestination)
                return ()
            }
            (wire::RpcStatus::BadFormat, wire::mod_RpcMessage::OneOfpayload::error_msg(msg)) => {
                //TransportResult::Err(TransportError::BadFormat(msg.into()))
                return ()
            }
            _ => return (),
        };

        MessageRouter::from_registry().do_send(RouteMessage {
            msg_id: rpc.message_id.as_ref().into(),
            sender: self.peer_node_id.clone().unwrap(),
            destination: rpc.destination_id.as_ref().into(),
            reply_to: None,
            correlation_id: None,
            ts: rpc.ts.unwrap_or(0),
            expires: rpc.expires,
            body,
        })
    }

    fn reply_init(&mut self, ctx : &mut <Self as Actor>::Context) {
        use std::borrow::Cow;

        let hello = wire::HelloReply {
            role: wire::Role::HUB,
            node_name: None,
            node_id: Cow::Borrowed(self.node_id.as_ref()),
            version: Some(Cow::Borrowed("0.1")),
            max_ping_ms: None,
        };

        let v = serialize_into_vec(&hello).expect("Cannot write message!");

        ctx.binary(v);
    }
}

impl<S> Actor for Worker<S> {
    type Context = ws::WebsocketContext<Self, S>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {

    }
}

impl<S> StreamHandler<ws::Message, ws::ProtocolError> for Worker<S> {
    fn handle(&mut self, item: ws::Message, ctx: &mut Self::Context) {
        use quick_protobuf::{BytesReader, MessageRead};

        match item {
            ws::Message::Binary(b) => if self.peer_node_id.is_none() {
                let mut reader = BytesReader::from_bytes(b.as_ref());
                match wire::Hello::from_reader(&mut reader, b.as_ref()) {
                    Ok(hello) => {
                        info!("handshake for: {:?}", hello);
                        let mut peer_node_id:NodeId = NodeId::default();
                        peer_node_id[..].copy_from_slice(hello.node_id.as_ref());
                        self.peer_node_id = Some(peer_node_id);
                        self.reply_init(ctx)
                    }
                    Err(e) => {
                        ctx.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                    }
                }
            } else {
                let mut reader = BytesReader::from_bytes(b.as_ref());
                match wire::RpcMessage::from_reader(&mut reader, b.as_ref()) {
                    Ok(rpc) => self.route(rpc),
                    Err(e) => {
                        ctx.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                    }
                }
            },
            p => {
                warn!("unknown package: {:?}", p)
            },
        }
    }
}

pub fn route<T: 'static>(req: HttpRequest<T>, node_id : NodeId) -> Result<HttpResponse, actix_web::Error> {
    let actor = Worker::new(node_id);
    ws::start(&req, actor)
}
