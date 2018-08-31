use super::message::{self, public_destination, DestinationId};
use super::router::{self, MessageRouter};
use actix::dev::*;
use actix::prelude::*;
use futures::{prelude::*, future};
use futures::sync::oneshot::Sender;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::any;
use std::collections::HashMap;
use std::marker::PhantomData;
use serde_json;

pub struct RemotingContext<A>
where
    A: Actor<Context = RemotingContext<A>>,
{
    inner: ContextParts<A>,
    mb: Option<Mailbox<A>>,
    destinations: HashMap<any::TypeId, DestinationId>,
}

impl<A> ActorContext for RemotingContext<A>
where
    A: Actor<Context = Self>,
{
    fn stop(&mut self) {
        self.inner.stop();
    }

    fn terminate(&mut self) {
        self.inner.terminate()
    }

    fn state(&self) -> ActorState {
        self.inner.state()
    }
}

impl<A> AsyncContext<A> for RemotingContext<A>
where
    A: Actor<Context = Self>,
{
    fn spawn<F>(&mut self, fut: F) -> SpawnHandle
    where
        F: ActorFuture<Item = (), Error = (), Actor = A> + 'static,
    {
        self.inner.spawn(fut)
    }

    fn wait<F>(&mut self, fut: F)
    where
        F: ActorFuture<Item = (), Error = (), Actor = A> + 'static,
    {
        self.inner.wait(fut)
    }

    #[doc(hidden)]
    #[inline]
    fn waiting(&self) -> bool {
        self.inner.waiting()
            || self.inner.state() == ActorState::Stopping
            || self.inner.state() == ActorState::Stopped
    }

    fn cancel_future(&mut self, handle: SpawnHandle) -> bool {
        self.inner.cancel_future(handle)
    }

    #[inline]
    fn address(&self) -> Addr<A> {
        self.inner.address()
    }
}

impl<A> AsyncContextParts<A> for RemotingContext<A>
    where
        A: Actor<Context = Self>,
{
    fn parts(&mut self) -> &mut ContextParts<A> {
        &mut self.inner
    }
}

impl<A, M> ToEnvelope<A, M> for RemotingContext<A>
    where
        A: Actor<Context = RemotingContext<A>> + Handler<M>,
        M: Message + Send + 'static,
        M::Result: Send,
{
    fn pack(msg: M, tx: Option<Sender<M::Result>>) -> Envelope<A> {
        Envelope::new(msg, tx)
    }
}



impl<A> RemotingContext<A>
where
    A: Actor<Context = Self>,
{
    #[inline]
    pub(crate) fn new() -> RemotingContext<A> {
        let mb = Mailbox::default();
        RemotingContext {
            inner: ContextParts::new(mb.sender_producer()),
            destinations: HashMap::new(),
            mb: Some(mb),
        }
    }

    #[inline]
    pub fn run(self, act: A) -> Addr<A> {
        let fut = self.into_future(act);
        let addr = fut.address();
        Arbiter::spawn(fut);
        addr
    }

    pub fn into_future(mut self, act: A) -> ContextFut<A, Self> {
        let mb = self.mb.take().unwrap();
        ContextFut::new(self, act, mb)
    }


    pub fn bind<T: any::Any + Send>(&mut self, destination_id: u32)
    where
        A: Handler<T>,
        T: Message + DeserializeOwned,
        T::Result: Serialize + Send,
        A::Context : ToEnvelope<A ,T>
    {
        let type_id = any::TypeId::of::<T>();
        let addr = self.address();
        let endpoint = Box::new(AddrWrapper {
            addr,
            message: PhantomData,
        });
        MessageRouter::from_registry().do_send(router::BindDestination {
            destination_id: public_destination(destination_id),
            endpoint,
        })
    }
}

struct AddrWrapper<A, T>
where
    A: Actor + Handler<T>,
    T: Message + DeserializeOwned,
    T::Result: Serialize,
{
    addr: Addr<A>,
    message: PhantomData<T>,
}

unsafe impl<A, T> Send for AddrWrapper<A, T> where
    A: Actor + Handler<T>,
    T: Message + DeserializeOwned,
    T::Result: Serialize
{}

impl<A, T> router::LocalEndpoint for AddrWrapper<A, T>
where
    A: Actor + Handler<T>,
    T: Message + DeserializeOwned + Send,
    T::Result: Serialize + Send,
    A::Context : ToEnvelope<A ,T>
{
    fn handle(
        &mut self,
        message: message::RouteMessage<String>,
        ctx: &mut <MessageRouter as Actor>::Context,
    ) {
        let m = message.clone();

        match message.from_json() {
            Err(err) => if let Some(msg) = message::EmitMessage::reply(
                &m,
                message::TransportResult::bad_request(format!("{}", err)),
            ) {
                ctx.notify(msg)
            },
            Ok(message) => {
                let m = message.unit();

                let f = self
                    .addr
                    .send(message.body)
                    .then(|r| match r {
                        Ok(b) => serde_json::to_string(&b).map_err(|_|()),
                        Err(e) => Err(())
                    })
                    .and_then(|r| {
                        m.do_reply(r, |reply| ctx.notify(reply));
                        future::ok(())
                    });
            }
        }
    }
}


pub fn start_actor<A : Actor>(actor : A) -> Addr<A>
    where
        A: Actor<Context = RemotingContext<A>>,
{
    RemotingContext::new().run(actor)
}