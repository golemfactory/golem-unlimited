use actix::prelude::*;
use futures::prelude::*;
use tokio::prelude::*;

use bytes::BytesMut;

use errors::{Error, ErrorKind, Result};
use mdns_codec::MdnsCodec;
use service::{Service, ServiceInstance, ServicesList};
use futures::sync::oneshot;
use socket2::{Domain, Type, Protocol, Socket, SockAddr};

use std::borrow::Cow;
use std::collections::HashMap;
use std::net::{Ipv4Addr, IpAddr, SocketAddrV4};

use tokio::net::{UdpFramed, UdpSocket};
use tokio::reactor::Handle;
use tokio_codec::{Decoder, Encoder};
use dns_parser::Header;
use std::net::SocketAddr;
use std::time::Duration;
use actix::AsyncContext;
use futures::sync::mpsc;
use futures::sink::SendAll;
use futures::sync;


/// Actor resolving mDNS services names into list of IPs
#[derive(Debug, Default, Clone)]
pub struct ResolveActor {
    sender: Option<sync::mpsc::Sender<((Service, u16), SocketAddr)>>,
}

pub type Response = ActorResponse<ResolveActor, ServicesList, Error>;

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
        let any_interface = Ipv4Addr::new(0, 0, 0, 0);

        // TODO: this is not recommended on Windows
        let socket_address = SocketAddrV4::new(multicast_ip, 0);

        socket.set_reuse_address(true)?;
        socket.set_multicast_loop_v4(true)?;
        socket.join_multicast_v4(&multicast_ip, &any_interface)?;
        socket.bind(&socket_address.into())?;

        UdpSocket::from_std(socket.into_udp_socket(), &Handle::current())
            .map_err(Error::from)
    }

    fn build_response<F>(&self, fut: F, ctx: &mut Context<Self>) -> Response
        where
            F: Future<Item=(), Error=Error> + 'static
    {
        let (tx, rx) = oneshot::channel();

        // Create the job that will be run in the future
        ctx.run_later(Duration::from_secs(1), move |act, ctx| {
            let job = fut
                .and_then(|_| {
                    tx.send(Ok(Vec::new()));
                    Ok(())
                })
                .map_err(|_| ())
                .into_actor(act);

            ctx.spawn(job);
        });

        // Result that can be resolved in the future
        ActorResponse::async(rx
            .and_then(|r| r)
            .map_err(Error::from)
            .into_actor(self))
    }
}

impl StreamHandler<(ServiceInstance, SocketAddr), Error> for ResolveActor {
    fn handle(&mut self, item: (ServiceInstance, SocketAddr), ctx: &mut Context<ResolveActor>) {
        debug!("Handle UDP packet");
    }
}

impl Actor for ResolveActor {
    type Context = Context<Self>;

    /// Creates stream handler for incoming mDNS packets
    fn started(&mut self, ctx: &mut Self::Context) {
        let socket = Self::create_mdns_socket().unwrap();
        let (sink, stream)  = UdpFramed::new(socket, MdnsCodec{}).split();

        Self::add_stream(stream, ctx);

        let (tx, rx) = mpsc::channel(16);
        ctx.spawn(sink
            .send_all(rx.map_err(|_| ErrorKind::UninitializedChannelReceiver))
            .then(|_| Ok(()))
            .into_actor(self)
        );

        self.sender = Some(tx);
        debug!("Resolve actor started");
    }

    fn stopped(&mut self, _ctx: &mut <Self as Actor>::Context) {
        debug!("Resolve actor stopped");
    }
}

impl Handler<Service> for ResolveActor {
    type Result = ActorResponse<ResolveActor, ServicesList, Error>;

    fn handle(&mut self, msg: Service, ctx: &mut Self::Context) -> <Self as Handler<Service>>::Result {
        debug!("Handling Service message");
        let multicast_ip = Ipv4Addr::new(224, 0, 0, 251);
        let addr = SocketAddrV4::new(multicast_ip, 5353).into();

        let message = ((msg, 1), addr);
        let sender = self.sender.clone();

        let future = match sender {
            Some(a) => future::Either::A(a.send(message)
                .and_then(|sender| Ok(sender.flush()))
                .and_then(|a| Ok(debug!("Completed")))
                .map_err(|_| ErrorKind::FutureSendError.into())),
            None => future::Either::B(future::err(ErrorKind::ActorNotInitialized.into())),
        };

        self.build_response(future, ctx)
    }
}
