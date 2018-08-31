use actix::prelude::*;
use futures::prelude::*;
use tokio::prelude::*;

use errors::{
    Error,
    Result,
};

use dns_parser::{
    Builder,
    Error as DNSError,
    Packet,
    QueryClass,
    QueryType,
};

use futures::sync::oneshot;

use socket2::{
    Domain,
    Type,
    Protocol,
    Socket
};

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::{
    Ipv4Addr,
    SocketAddr,
    SocketAddrV4,
};

use tokio::net::UdpSocket;
use tokio::reactor::Handle;
use dns_parser::Header;

/// Struct describing single service in .local domain's network
///
/// Service Instance Name = <Instance> . <Service> . <Domain>
struct Service {
    /// Instance name; eg. "gu-provider"
    instance: Cow<'static, str>,
    /// Service type; eg. "_http._tcp"
    service: Cow<'static, str>,
}

impl Message for Service {
    type Result = Result<ServicesList>;
}

struct ServiceInstance {
    data: Header,
}

type ServicesList = Vec<ServiceInstance>;

/// Actor resolving mDNS services names into list of IPs
pub struct ResolveActor {}

fn create_mdns_socket() -> Result<UdpSocket> {
    let socket = Socket::new(
        Domain::ipv4(),
        Type::dgram(),
        Some(Protocol::udp())
    )?;


    let m_ip = Ipv4Addr::new(224, 0, 0, 251);
    let ip = Ipv4Addr::new(127, 0, 0, 1);

    socket.set_multicast_loop_v4(true);
    socket.join_multicast_v4(&m_ip, &ip);

    UdpSocket::from_std(socket.into_udp_socket(), &Handle::current())
        .map_err(Error::from)
}

struct UdpListener {
    socket: UdpSocket,
}

// TODO: UdpCodec instead of Stream to provide decoding of messages and let to .split() socket
// TODO: to get UdpListener

impl Stream for UdpListener {
    type Item = ServiceInstance;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let mut buffer : &mut [u8] = &mut [0; 512];
        match self.socket.poll_recv(buffer) {
            Ok(Async::Ready(data)) => {
                Ok(Async::Ready({
                    let packet = Packet::parse(buffer)?;
                    Some(ServiceInstance {
                        data: packet.header,
                    })
                }))
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e.into()),
        }
    }
}

impl StreamHandler<ServiceInstance, Error> for ResolveActor {
    fn handle(&mut self, item: ServiceInstance, ctx: &mut Context<ResolveActor>) {
        println!("aaaaaa");
    }
}

impl Actor for ResolveActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("asdkhj");
        let socket = create_mdns_socket().unwrap();
        let stream = UdpListener { socket };
        Self::add_stream(stream, ctx);
    }

    fn stopped(&mut self, ctx: &'_ mut <Self as Actor>::Context) {
        println!("stop");
    }
}

impl Handler<Service> for ResolveActor {
    type Result = ActorResponse<ResolveActor, ServicesList, Error >;

    fn handle(&mut self, msg: Service, _ctx: &mut Self::Context) -> <Self as Handler<Service>>::Result {
        let (tx, rx) = oneshot::channel();
        tx.send(Ok(Vec::new()));

        ActorResponse::async(rx
            .then(|r| match r {
                Ok(Ok(v)) => Ok(v),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(e.into())
            })
            .into_actor(self))
        //let hash
        //let query = Builder::new_query(, false);
    }
}