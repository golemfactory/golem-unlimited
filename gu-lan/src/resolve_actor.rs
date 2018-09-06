use actix::prelude::*;
use futures::prelude::*;
use tokio::prelude::*;

use errors::{Error, ErrorKind, Result};
use mdns_codec::MdnsCodec;
use service::{Service, ServiceInstance, Services};
use futures::sync::oneshot;
use socket2::{Domain, Type, Protocol, Socket};

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use tokio::net::{UdpFramed, UdpSocket};
use tokio::reactor::Handle;
use std::net::SocketAddr;
use std::time::Duration;
use actix::AsyncContext;
use futures::sync::mpsc;
use futures::sync;
use std::collections::HashSet;
use gu_actix::FlattenFuture;
use mdns_codec::ParsedPacket;


/// Actor resolving mDNS services names into list of IPs
#[derive(Debug, Default)]
pub struct ResolveActor {
    sender: Option<sync::mpsc::Sender<((Service, u16), SocketAddr)>>,
    next_id: u16,
    map: HashMap<u16, Services>,
}

pub type Response = ActorResponse<ResolveActor, HashSet<ServiceInstance>, Error>;

impl ResolveActor {
    pub fn new() -> Self {
        ResolveActor::default()
    }

    fn create_mdns_socket() -> Result<UdpSocket> {
        let socket = Socket::new(
            Domain::ipv4(),
            Type::dgram(),
            Some(Protocol::udp())
        )?;

        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let any_ip = Ipv4Addr::new(0, 0, 0, 0);

        let socket_address = SocketAddrV4::new(any_ip, 0);

        socket.set_reuse_address(true)?;
        socket.set_multicast_loop_v4(true)?;
        socket.join_multicast_v4(&multicast_ip, &any_ip)?;
        socket.bind(&socket_address.into())?;

        UdpSocket::from_std(socket.into_udp_socket(), &Handle::current())
            .map_err(Error::from)
    }

    fn hashset_of_services(&mut self, id: u16) -> Result<HashSet<ServiceInstance>> {
        self.map
            .remove(&id)
            .ok_or(ErrorKind::MissingKey.into())
            .map(|services| services.set())
    }

    fn build_response<F>(&mut self, fut: F, _ctx: &mut Context<Self>, id: u16) -> Response
        where
            F: Future<Item=(), Error=Error> + 'static
    {
        let (tx, rx) = oneshot::channel();

        ActorResponse::async(
        fut.into_actor(self)
            .and_then(move |_r, act, ctx| {
                ctx.run_later(Duration::from_secs(1), move |act, _ctx| {
                    let _ = tx.send(act.hashset_of_services(id))
                        .map_err(|e| error!("{:?}", e));
                });
                rx.flatten_fut()
                    .into_actor(act)
            })
        )
    }
}

impl StreamHandler<(ParsedPacket, SocketAddr), Error> for ResolveActor {
    fn handle(&mut self, (packet, _): (ParsedPacket, SocketAddr), _ctx: &mut Context<ResolveActor>) {
        if let Some(services) = self.map.get_mut(&packet.0) {
            for service in packet.1 {
                services.add_instance(service.0, service.1);
            }
        }


    }
}

impl Actor for ResolveActor {
    type Context = Context<Self>;

    /// Creates stream handler for incoming mDNS packets
    fn started(&mut self, ctx: &mut Self::Context) {
        let socket = Self::create_mdns_socket().expect("Creation of mDNS socket failed");
        let (sink, stream)  = UdpFramed::new(socket, MdnsCodec{}).split();

        Self::add_stream(stream, ctx);

        let (tx, rx) = mpsc::channel(16);
        ctx.spawn(sink
            .send_all(rx.map_err(|_| ErrorKind::UninitializedChannelReceiver))
            .then(|_| Ok(()))
            .into_actor(self)
        );

        self.sender = Some(tx);
    }
}

impl Handler<Service> for ResolveActor {
    type Result = Response;

    fn handle(&mut self, msg: Service, ctx: &mut Self::Context) -> Response {
        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let addr = SocketAddrV4::new(multicast_ip, 5353).into();

        let id = self.next_id;
        self.next_id = id.wrapping_add(1);

        self.map.insert(id, Services::new(msg.to_string()));

        let message = ((msg, id), addr);

        let sender = self.sender.clone();

        let future = match sender {
            Some(a) => future::Either::A(a.send(message)
                .and_then(|sender| Ok(sender.flush()))
                .and_then(|_| Ok(()))
                .map_err(|_| ErrorKind::FutureSendError.into())),
            None => future::Either::B(future::err(ErrorKind::ActorNotInitialized.into())),
        };

        self.build_response(future, ctx, id)
    }
}

#[cfg(test)]
mod tests {
    use resolve_actor::ResolveActor;

    #[test]
    fn create_mdns_socket() {
        let socket = ResolveActor::create_mdns_socket();

        assert!(socket.is_ok());
    }
}