use actix::prelude::*;
use futures::prelude::*;
use tokio::prelude::*;

use errors::{Error, ErrorKind, Result};
use futures::sync::oneshot;
use mdns_codec::MdnsCodec;
use service::{ServiceInstance, ServicesDescription};
use socket2::{Domain, Protocol, Socket, Type};

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use actix::AsyncContext;
use futures::sync;
use futures::sync::mpsc;
use gu_actix::FlattenFuture;
use mdns_codec::ParsedPacket;
use service::Services;
use std::collections::HashSet;
use std::net::SocketAddr::{self, V4};
use std::time::Duration;
use tokio::net::{UdpFramed, UdpSocket};
use tokio::reactor::Handle;

/// Actor resolving mDNS services names into list of IPs
#[derive(Debug, Default)]
pub struct ResolveActor {
    /// Interior, indirect responder sink
    sender: Option<sync::mpsc::Sender<((ServicesDescription, u16), SocketAddr)>>,
    /// Next id for mDNS query
    next_id: u16,
    /// Services for given id
    map: HashMap<u16, Services>,
}

pub type Response = ActorResponse<ResolveActor, HashSet<ServiceInstance>, Error>;

impl ResolveActor {
    pub fn new() -> Self {
        ResolveActor::default()
    }

    fn create_mdns_socket() -> Result<UdpSocket> {
        let socket = Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::udp()))?;

        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let any_ip = Ipv4Addr::new(0, 0, 0, 0);

        let socket_address = SocketAddrV4::new(any_ip, 0);

        socket.set_reuse_address(true)?;
        socket.set_multicast_loop_v4(true)?;
        socket.join_multicast_v4(&multicast_ip, &any_ip)?;
        socket.bind(&socket_address.into())?;

        UdpSocket::from_std(socket.into_udp_socket(), &Handle::current()).map_err(Error::from)
    }

    fn retrieve_services(&mut self, id: u16) -> Result<HashSet<ServiceInstance>> {
        self.map
            .remove(&id)
            .ok_or(ErrorKind::MissingKey.into())
            .and_then(|a| Ok(a.collect()))
    }

    fn build_response<F>(&mut self, fut: F, _ctx: &mut Context<Self>, id: u16) -> Response
    where
        F: Future<Item = (), Error = Error> + 'static,
    {
        let (tx, rx) = oneshot::channel();

        ActorResponse::async(fut.into_actor(self).and_then(move |_r, act, ctx| {
            ctx.run_later(Duration::from_secs(1), move |act, _ctx| {
                let _ = tx
                    .send(act.retrieve_services(id))
                    .and_then(|_a| Ok(()))
                    .map_err(|e| error!("{:?}", e));
            });
            rx.flatten_fut().into_actor(act)
        }))
    }
}

fn zeros(s1: &Ipv4Addr, s2: &Ipv4Addr) -> u8 {
    let oct1 = s1.octets();
    let oct2 = s2.octets();
    let mut zeros = 0;

    for pos in 0..3 {
        let add = (oct1[pos] ^ oct2[pos]).leading_zeros() as u8;
        zeros += add;

        if add != 8 {
            break;
        }
    }

    zeros
}

fn biggest_mask_ipv4(vec: &Vec<Ipv4Addr>, src: &Ipv4Addr) -> Vec<Ipv4Addr> {
    let mut best_instance = None;
    let mut best_score = 33;

    for ip in vec.iter() {
        if best_instance == None {
            let zeros = zeros(src, ip);
            if zeros < best_score {
                best_instance = Some(ip);
                best_score = zeros;
            }
        }
    }

    vec![*best_instance.unwrap_or(src)]
}

impl StreamHandler<(ParsedPacket, SocketAddr), Error> for ResolveActor {
    fn handle(
        &mut self,
        (packet, src): (ParsedPacket, SocketAddr),
        _ctx: &mut Context<ResolveActor>,
    ) {
        if let Some(services) = self.map.get_mut(&packet.id) {
            for mut service in packet.instances {
                match src {
                    V4(sock) => service.addrs_v4 = biggest_mask_ipv4(&service.addrs_v4, sock.ip()),
                    _ => (),
                }

                services.add_instance(service);
            }
        }
    }
}

impl Actor for ResolveActor {
    type Context = Context<Self>;

    /// Creates stream handler for incoming mDNS packets
    fn started(&mut self, ctx: &mut Self::Context) {
        let socket = Self::create_mdns_socket().expect("Creation of mDNS socket failed");
        let (sink, stream) = UdpFramed::new(socket, MdnsCodec {}).split();

        Self::add_stream(stream, ctx);

        let (tx, rx) = mpsc::channel(16);
        ctx.spawn(
            sink.send_all(rx.map_err(|_| ErrorKind::UninitializedChannelReceiver))
                .then(|_| Ok(()))
                .into_actor(self),
        );

        self.sender = Some(tx);
    }
}

impl Supervised for ResolveActor {}

impl ArbiterService for ResolveActor {}

impl Handler<ServicesDescription> for ResolveActor {
    type Result = Response;

    fn handle(&mut self, msg: ServicesDescription, ctx: &mut Self::Context) -> Response {
        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let addr = SocketAddrV4::new(multicast_ip, 5353).into();

        let id = self.next_id;
        self.next_id = id.wrapping_add(1);

        self.map.insert(id, msg.to_services());
        let message = ((msg, id), addr);
        let sender = self.sender.clone();

        let future = match sender {
            Some(a) => future::Either::A(
                a.send(message)
                    .and_then(|sender| Ok(sender.flush()))
                    .and_then(|_| Ok(()))
                    .map_err(|_| ErrorKind::FutureSendError.into()),
            ),
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