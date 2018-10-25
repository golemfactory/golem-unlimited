use actix::prelude::*;
use futures::prelude::*;
use tokio::prelude::*;

use codec::MdnsCodec;
use errors::{Error, ErrorKind, Result};
use futures::sync::oneshot;
use service::{ServiceInstance, ServicesDescription};
use socket2::{Domain, Protocol, Socket, Type};

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
};

use actix::AsyncContext;
use codec::ParsedPacket;
use continuous::{
    ContinuousInstancesList, ForeignMdnsQueryInfo, NewInstance, ReceivedMdnsInstance, Subscribe,
    Subscription,
};
use futures::sync::mpsc;
use gu_actix::FlattenFuture;
use service::{ServiceDescription, Services};
use std::{
    collections::HashSet,
    net::SocketAddr::{self, V4},
    time::Duration,
};
use tokio::{
    net::{UdpFramed, UdpSocket},
    reactor::Handle,
};

/// Actor resolving mDNS services names into list of IPs
#[derive(Debug, Default)]
pub struct MdnsActor<T: MdnsConnection> {
    /// Interior, indirect responder sink
    sender: Option<mpsc::Sender<((ServicesDescription, u16), SocketAddr)>>,
    data: Box<T>,
}

pub trait MdnsConnection: 'static + Default + Sized {
    fn port() -> u16;

    fn unicast_query() -> bool;

    fn handle_packet(&mut self, packet: ParsedPacket, src: SocketAddr);
}

pub type OneShotResponse<T> = ActorResponse<MdnsActor<T>, HashSet<ServiceInstance>, Error>;
pub type ContinuousResponse<T> = ActorResponse<MdnsActor<T>, Subscription, Error>;

#[derive(Debug, Default)]
pub struct OneShot {
    /// Next id for mDNS query
    next_id: u16,
    /// Services for given id
    map: HashMap<u16, Services>,
}

#[derive(Default)]
pub struct Continuous {
    /// Services for given id
    map: HashMap<String, Addr<ContinuousInstancesList>>,
}

impl MdnsConnection for OneShot {
    fn port() -> u16 {
        0
    }

    fn unicast_query() -> bool {
        true
    }

    fn handle_packet(&mut self, packet: ParsedPacket, src: SocketAddr) {
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

impl MdnsConnection for Continuous {
    fn port() -> u16 {
        5353
    }

    fn unicast_query() -> bool {
        false
    }

    fn handle_packet(&mut self, packet: ParsedPacket, _src: SocketAddr) {
        for name in packet.questions {
            self.map
                .get(&name)
                .map(|list| list.do_send(ForeignMdnsQueryInfo));
        }

        for instance in packet.instances {
            self.map
                .get(&instance.name)
                .map(|list| list.do_send(ReceivedMdnsInstance::new(instance)));
        }
    }
}

impl<T: MdnsConnection> MdnsActor<T> {
    pub fn new() -> Self {
        MdnsActor::default()
    }

    fn create_mdns_socket() -> Result<UdpSocket> {
        let socket = Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::udp()))?;

        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let any_ip = Ipv4Addr::new(0, 0, 0, 0);

        let socket_address = SocketAddrV4::new(any_ip, T::port());

        socket.set_reuse_address(true)?;
        socket.set_multicast_loop_v4(true)?;
        socket.join_multicast_v4(&multicast_ip, &any_ip)?;
        socket.bind(&socket_address.into())?;

        UdpSocket::from_std(socket.into_udp_socket(), &Handle::current()).map_err(Error::from)
    }
}

pub fn send_mdns_query(
    sender: Option<mpsc::Sender<((ServicesDescription, u16), SocketAddr)>>,
    services: ServicesDescription,
    id: u16,
) -> impl Future<Item = (), Error = Error> {
    let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
    let addr = SocketAddrV4::new(multicast_ip, 5353).into();

    let message = ((services, id), addr);

    match sender {
        Some(a) => future::Either::A(
            a.send(message)
                .and_then(|sender| Ok(sender.flush()))
                .and_then(|_| Ok(()))
                .map_err(|e| {
                    println!("{}", e);
                    ErrorKind::FutureSendError.into()
                }),
        ),
        None => future::Either::B(future::err(ErrorKind::ActorNotInitialized.into())),
    }
}

impl MdnsActor<OneShot> {
    fn retrieve_services(&mut self, id: u16) -> Result<HashSet<ServiceInstance>> {
        self.data
            .map
            .remove(&id)
            .ok_or(ErrorKind::MissingKey.into())
            .and_then(|a| Ok(a.collect()))
    }

    fn build_response<F>(
        &mut self,
        fut: F,
        _ctx: &mut Context<Self>,
        id: u16,
    ) -> OneShotResponse<OneShot>
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

struct PacketPair {
    pub packet: ParsedPacket,
    pub socket: SocketAddr,
}

impl Message for PacketPair {
    type Result = ();
}

impl<T: MdnsConnection> Handler<PacketPair> for MdnsActor<T> {
    type Result = ();

    fn handle(&mut self, msg: PacketPair, _ctx: &mut Context<MdnsActor<T>>) -> () {
        T::handle_packet(&mut self.data, msg.packet, msg.socket)
    }
}

impl<T: MdnsConnection> Actor for MdnsActor<T> {
    type Context = Context<Self>;

    /// Creates stream handler for incoming mDNS packets
    fn started(&mut self, ctx: &mut Self::Context) {
        use futures::Stream;

        let socket = Self::create_mdns_socket().expect("Creation of mDNS socket failed");
        let (sink, stream) = UdpFramed::new(socket, MdnsCodec(T::unicast_query())).split();

        ctx.add_message_stream(
            stream
                .map(|(packet, socket)| PacketPair { packet, socket })
                .map_err(|_| ()),
        );

        let (tx, rx) = mpsc::channel(16);
        ctx.spawn(
            rx.map_err(|_| ErrorKind::UninitializedChannelReceiver)
                .forward(sink)
                .map_err(|e| error!("{:?}", e))
                .and_then(|_| Ok(()))
                .into_actor(self),
        );

        self.sender = Some(tx);
    }
}

impl<T: MdnsConnection> Supervised for MdnsActor<T> {}

impl<T: MdnsConnection> SystemService for MdnsActor<T> {}

impl Handler<ServicesDescription> for MdnsActor<OneShot> {
    type Result = OneShotResponse<OneShot>;

    fn handle(
        &mut self,
        msg: ServicesDescription,
        ctx: &mut Self::Context,
    ) -> OneShotResponse<OneShot> {
        let id = self.data.next_id;
        self.data.next_id = id.wrapping_add(1);

        self.data.map.insert(id, Services::from(&msg));
        let future = send_mdns_query(self.sender.clone(), msg, id);

        self.build_response(future, ctx, id)
    }
}

pub struct SubscribeInstance {
    pub service: ServiceDescription,
    pub rec: Recipient<NewInstance>,
}

impl Message for SubscribeInstance {
    type Result = Result<Subscription>;
}

impl Handler<SubscribeInstance> for MdnsActor<Continuous> {
    type Result = ContinuousResponse<Continuous>;

    fn handle(
        &mut self,
        msg: SubscribeInstance,
        _ctx: &mut Self::Context,
    ) -> ContinuousResponse<Continuous> {
        use std::collections::hash_map::Entry;

        let res = match self.data.map.entry(msg.service.to_string()) {
            Entry::Vacant(a) => {
                let service =
                    ContinuousInstancesList::new(msg.service.clone(), self.sender.clone().unwrap())
                        .start();
                a.insert(service.clone().into());
                service.send(Subscribe { rec: msg.rec })
            }
            Entry::Occupied(ref mut b) => {
                if b.get().connected() {
                    b.get_mut().send(Subscribe { rec: msg.rec })
                } else {
                    b.insert({
                        ContinuousInstancesList::new(
                            msg.service.clone(),
                            self.sender.clone().unwrap(),
                        )
                        .start()
                    })
                    .send(Subscribe { rec: msg.rec })
                }
            }
        };

        ActorResponse::async(res.map_err(|_| ErrorKind::Mailbox.into()).into_actor(self))
    }
}

#[cfg(test)]
mod tests {
    use actor::{MdnsActor, OneShot};

    #[test]
    fn create_mdns_socket() {
        let socket = MdnsActor::<OneShot>::create_mdns_socket();

        assert!(socket.is_ok());
    }
}
