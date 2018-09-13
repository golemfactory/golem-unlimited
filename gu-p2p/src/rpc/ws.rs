/*
 * Websocket interface for message router
 */

use super::super::proto::wire;
use super::error;
use super::message::{
    EmitMessage, MessageId, NodeId, RouteMessage, TransportError, TransportResult,
};
use super::peer::{self, PeerManager};
use super::router::{AddEndpoint, DelEndpoint, MessageRouter};
use actix::prelude::*;
use actix_web::{self, ws, HttpRequest, HttpResponse};
use futures::prelude::*;
use quick_protobuf::serialize_into_vec;
use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::Add;
use std::{net, time};

fn rpc_to_route<T>(peer_node_id: NodeId, rpc: wire::RpcMessage, body: T) -> RouteMessage<T> {
    RouteMessage {
        msg_id: rpc.message_id.as_ref().into(),
        sender: peer_node_id,
        destination: rpc.destination_id.as_ref().into(),
        reply_to: rpc.reply_to.map(|v| v.as_ref().into()),
        correlation_id: rpc.correlation_id.map(|v| v.as_ref().into()),
        ts: rpc.ts.unwrap_or(0),
        expires: rpc.expires,
        body,
    }
}

fn route_rpc_message(peer_node_id: NodeId, mut rpc: wire::RpcMessage) {
    let (status, payload) = (rpc.status, rpc.payload.take());

    match (status, payload) {
        (wire::RpcStatus::Request, Some(json)) => {
            let body: String = json.into();
            MessageRouter::from_registry().do_send(rpc_to_route(peer_node_id, rpc, body))
        }
        (wire::RpcStatus::Reply, Some(json)) => {
            let body: Result<String, TransportError> = Ok(json.into());
            MessageRouter::from_registry().do_send(rpc_to_route(peer_node_id, rpc, body))
        }
        (wire::RpcStatus::NoDestination, _) => {
            let body: Result<String, TransportError> = Err(TransportError::NoDestination);
            MessageRouter::from_registry().do_send(rpc_to_route(peer_node_id, rpc, body))
        }
        (wire::RpcStatus::BadFormat, Some(msg)) => {
            let body: Result<String, TransportError> = Err(TransportError::BadFormat(msg.into()));
            MessageRouter::from_registry().do_send(rpc_to_route(peer_node_id, rpc, body))
        }
        _ => return (),
    }
}

struct Worker<S: 'static> {
    state: PhantomData<S>,
    node_id: NodeId,
    peer_node_id: Option<NodeId>,
    peer_addr: Option<net::SocketAddr>,
    pong_ts: Option<time::Instant>,
}

impl<S> Worker<S> {
    fn new(node_id: NodeId, peer_addr: Option<net::SocketAddr>) -> Self {
        Worker {
            state: PhantomData,
            node_id,
            peer_node_id: None,
            peer_addr,
            pong_ts: None,
        }
    }

    fn reply_init(&mut self, ctx: &mut <Self as Actor>::Context) {
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

    fn add_endpoint(&mut self, ctx: &mut <Self as Actor>::Context) {
        MessageRouter::from_registry().do_send(AddEndpoint {
            node_id: self.peer_node_id.unwrap(),
            recipient: ctx.address().recipient(),
        });
        PeerManager::from_registry().do_send(peer::UpdatePeer::Update(peer::PeerInfo {
            node_name: String::new(),
            peer_addr: self.peer_addr.map(|addr| format!("{}", addr)),
            node_id: self.peer_node_id.unwrap(),
            sessions: Vec::new(),
            tags: Vec::new(),
        }))
    }
}

impl<S> Actor for Worker<S> {
    type Context = ws::WebsocketContext<Self, S>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(time::Duration::from_secs(2), |act, ctx| {
            if let Some(ts) = act.pong_ts {
                if ts < time::Instant::now() {
                    ctx.stop();
                }
            } else {
                act.pong_ts = Some(time::Instant::now().add(time::Duration::from_secs(5)));
                ctx.ping("t")
            }
        });
    }

    fn stopped(&mut self, ctx: &'_ mut <Self as Actor>::Context) {
        info!("worker done");
        if let Some(peer_id) = self.peer_node_id.take() {
            PeerManager::from_registry().do_send(peer::UpdatePeer::Delete(peer_id))
        }
    }
}

impl<S> StreamHandler<ws::Message, ws::ProtocolError> for Worker<S> {
    fn handle(&mut self, item: ws::Message, ctx: &mut Self::Context) {
        use quick_protobuf::{deserialize_from_slice, BytesReader, MessageRead};

        match item {
            ws::Message::Binary(b) => if self.peer_node_id.is_none() {
                //let mut reader = BytesReader::from_bytes(b.as_ref());
                match deserialize_from_slice::<wire::Hello>(b.as_ref()) {
                    //wire::Hello::from_reader(&mut reader, b.as_ref()) {
                    Ok(hello) => {
                        info!("handshake for: {:?}", hello);
                        let mut peer_node_id: NodeId = hello.node_id.into();
                        self.peer_node_id = Some(peer_node_id);
                        self.reply_init(ctx);
                        self.add_endpoint(ctx);
                    }
                    Err(e) => {
                        ctx.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                    }
                }
            } else {
                debug!("got slice: {:?}", b);
                match deserialize_from_slice(b.as_ref()) {
                    Ok(rpc) => route_rpc_message(self.peer_node_id.clone().unwrap(), rpc),
                    Err(e) => {
                        ctx.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                    }
                }
            },
            ws::Message::Pong(_) => {
                self.pong_ts = None;
            }
            p => warn!("unknown package: {:?}", p),
        }
    }

    fn error(&mut self, err: ws::ProtocolError, ctx: &mut Self::Context) -> Running {
        error!("ws error: {}", err);
        Running::Stop
    }
}

impl<S> Handler<EmitMessage<String>> for Worker<S> {
    type Result = Result<MessageId, error::Error>;

    fn handle(
        &mut self,
        msg: EmitMessage<String>,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<EmitMessage<String>>>::Result {
        use rand::*;
        use smallvec;
        let m: [u8; 8] = thread_rng().gen();
        let (status, payload) = match msg.body {
            TransportResult::Request(ref b) => {
                (wire::RpcStatus::Request, Some(Cow::Borrowed(b.as_ref())))
            }
            TransportResult::Reply(ref b) => {
                (wire::RpcStatus::Reply, Some(Cow::Borrowed(b.as_ref())))
            }
            TransportResult::Err(TransportError::NoDestination) => {
                (wire::RpcStatus::NoDestination, None)
            }
            TransportResult::Err(TransportError::BadFormat(ref err_msg)) => (
                wire::RpcStatus::BadFormat,
                Some(Cow::Borrowed(err_msg.as_ref())),
            ),
        };

        let msg = wire::RpcMessage {
            message_id: Cow::Borrowed(m.as_ref()),
            destination_id: Cow::Borrowed(msg.destination.as_ref()),
            correlation_id: match msg.correlation_id {
                Some(ref v) => Some(Cow::Borrowed(v.as_ref())),
                None => None,
            },
            reply_to: msg.reply_to.as_ref().map(|v| Cow::Borrowed(v.as_ref())),
            ts: (if msg.ts == 0 { None } else { Some(msg.ts) }),
            expires: msg.expires,
            status,
            payload,
        };

        let mut bytes = serialize_into_vec(&msg)?;
        ctx.binary(bytes);
        Ok(smallvec::SmallVec::from(&m[..]))
    }
}

struct Client {
    node_id: NodeId,
    peer_node_id: Option<NodeId>,
    writer: ws::ClientWriter,
}

impl Client {
    fn add_endpoint(&mut self, ctx: &mut <Self as Actor>::Context) {
        MessageRouter::from_registry().do_send(AddEndpoint {
            node_id: self.peer_node_id.unwrap(),
            recipient: ctx.address().recipient(),
        });
    }

    fn connect(uri: &str, node_id: NodeId) -> impl Future<Item = Addr<Client>, Error = ()> {
        info!("start connect");
        ws::Client::new(uri)
            .connect()
            .map_err(|e| {
                error!("connect: {}", e);
                ()
            })
            .map(move |(reader, writer)| {
                let addr = Client::create(move |ctx| {
                    Client::add_stream(reader, ctx);
                    info!("connected");
                    Client {
                        writer,
                        node_id,
                        peer_node_id: None,
                    }
                });

                addr
            })
    }
}

impl Actor for Client {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        use rand::*;
        use smallvec;

        let m: [u8; 8] = thread_rng().gen();

        let hello = wire::Hello {
            role: wire::Role::PROVIDER,
            node_name: None,
            node_id: self.node_id.as_ref().into(),
            instance_id: Cow::Borrowed(&m),
            version: None,
            os: None,
            max_ram: None,
            max_storage: None,
            exec_envs: Vec::new(),
        };
        self.writer.binary(serialize_into_vec(&hello).unwrap())
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for Client {
    fn handle(&mut self, item: ws::Message, ctx: &mut Self::Context) {
        use quick_protobuf::{deserialize_from_slice, BytesReader, MessageRead};

        match item {
            ws::Message::Binary(b) => if self.peer_node_id.is_none() {
                match deserialize_from_slice::<wire::HelloReply>(b.as_ref()) {
                    Ok(hello) => {
                        info!("handshake for: {:?}", hello);
                        self.peer_node_id = Some(hello.node_id.into());
                        self.add_endpoint(ctx);
                    }
                    Err(e) => {
                        warn!("invalid message: {}", e);
                        self.writer.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                    }
                }
            } else {
                match deserialize_from_slice::<wire::RpcMessage>(b.as_ref()) {
                    Ok(rpc) => route_rpc_message(self.peer_node_id.clone().unwrap(), rpc),
                    Err(e) => {
                        error!("rpc format error: {}", e);
                        self.writer.close(Some(ws::CloseReason {
                            code: ws::CloseCode::Protocol,
                            description: Some(format!("{}", e)),
                        }));
                        ctx.stop()
                    }
                }
            },
            ws::Message::Ping(m) => {
                self.writer.pong(m.as_ref());
            }
            ws::Message::Close(r) => {
                warn!("closed: {:?}", r);
                ctx.stop()
            }
            p => warn!("unknown package: {:?}", p),
        }
    }
}

impl Handler<EmitMessage<String>> for Client {
    type Result = Result<MessageId, error::Error>;

    fn handle(
        &mut self,
        msg: EmitMessage<String>,
        ctx: &mut Self::Context,
    ) -> <Self as Handler<EmitMessage<String>>>::Result {
        debug!("emit message: {:?}", msg);

        use rand::*;
        use smallvec;
        let m: [u8; 8] = thread_rng().gen();
        let (status, payload): (_, Option<Cow<str>>) = match msg.body {
            TransportResult::Request(ref b) => (wire::RpcStatus::Request, Some(Cow::Borrowed(b))),
            TransportResult::Reply(ref b) => (wire::RpcStatus::Reply, Some(Cow::Borrowed(b))),
            TransportResult::Err(TransportError::NoDestination) => {
                (wire::RpcStatus::NoDestination, None)
            }
            TransportResult::Err(TransportError::BadFormat(ref err_msg)) => {
                (wire::RpcStatus::BadFormat, Some(Cow::Borrowed(err_msg)))
            }
        };

        let msg = wire::RpcMessage {
            message_id: Cow::Borrowed(m.as_ref()),
            destination_id: Cow::Borrowed(msg.destination.as_ref()),
            reply_to: match msg.reply_to {
                Some(ref v) => Some(Cow::Borrowed(v.as_ref())),
                None => None,
            },
            correlation_id: match msg.correlation_id {
                Some(ref v) => Some(Cow::Borrowed(v.as_ref())),
                None => None,
            },
            ts: (if msg.ts == 0 { None } else { Some(msg.ts) }),
            expires: msg.expires,
            status,
            payload,
        };

        let mut bytes = serialize_into_vec(&msg)?;
        self.writer.binary(bytes);
        Ok(smallvec::SmallVec::from(&m[..]))
    }
}

pub struct ConnectionSupervisor {
    node_id: NodeId,
    peer_address: net::SocketAddr,
    connection: Option<Addr<Client>>,
}

pub fn start_connection(
    node_id: NodeId,
    peer_address: net::SocketAddr,
) -> Addr<ConnectionSupervisor> {
    ConnectionSupervisor {
        node_id,
        peer_address,
        connection: None,
    }.start()
}

impl ConnectionSupervisor {
    fn check(&mut self, ctx: &mut <Self as Actor>::Context) {
        debug!("check: {:?}", self.connection.is_some());
        self.connection = match self.connection.take() {
            Some(addr) => if addr.connected() {
                Some(addr)
            } else {
                warn!("actor is down");
                None
            },
            None => None,
        };

        if self.connection.is_some() {
            debug!("check ok");
            return;
        }

        ctx.spawn(
            Client::connect(&format!("http://{}/ws/", &self.peer_address), self.node_id)
                .into_actor(self)
                .map(|r, act: &mut ConnectionSupervisor, ctx| {
                    debug!("set connection!");
                    act.connection = Some(r);
                })
                .map_err(|err, act, ctx| {
                    error!("fatal, restart, {:?}", &err);
                }),
        );
    }
}

impl Actor for ConnectionSupervisor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let _ = ctx.run_interval(time::Duration::from_secs(10), |act, ctx| act.check(ctx));
    }
}

pub fn route<T: 'static>(
    req: &HttpRequest<T>,
    node_id: NodeId,
) -> Result<HttpResponse, actix_web::Error> {
    let actor = Worker::new(node_id, req.peer_addr());
    ws::start(&req, actor)
}

#[cfg(test)]
mod test {
    use super::wire::*;
    use quick_protobuf::*;
    use std::borrow::Cow;

    #[test]
    fn test_rpc_message() {
        let message_id = [0u8; 32];
        let destination_id = [0u8; 4];
        let msg = "ala ma kota";

        let rpc: RpcMessage = RpcMessage {
            message_id: Cow::Borrowed(&message_id),
            destination_id: Cow::Borrowed(&destination_id),
            correlation_id: None,
            reply_to: None,
            ts: Some(0),
            expires: None,
            status: RpcStatus::Request,
            payload: Some(Cow::Borrowed(&msg)),
        };

        let buf = serialize_into_vec(&rpc).unwrap();

        assert!(deserialize_from_slice::<RpcMessage>(&buf).unwrap() == rpc)
    }

}
