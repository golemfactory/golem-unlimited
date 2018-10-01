
use futures::prelude::*;
use actix::prelude::*;
use actix::fut;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::any::{TypeId, Any};
use super::Event;

struct EventHub<T : 'static + Send + Sync> {
    last_id : u64,
    workers : BTreeMap<u64, Addr<EventHubWorker<T>>>
}

impl<T : 'static + Send + Sync> Default for EventHub<T> {
    fn default() -> Self {
        Self {
            last_id: 1,
            workers: BTreeMap::new()
        }
    }
}

impl<T : 'static + Send + Sync> EventHub<T> {

    #[inline]
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;

        self.last_id
    }

}

impl<T : 'static + Send + Sync> Actor for EventHub<T> {
    type Context = Context<Self>;
}

impl<T : 'static + Send + Sync> Supervised for EventHub<T> {}
impl<T : 'static + Send + Sync> SystemService for EventHub<T> {}

struct AddWorker<T : 'static + Send + Sync>(Addr<EventHubWorker<T>>);

impl<T : 'static + Send + Sync> Message for AddWorker<T> {
    type Result = u64;
}

impl<T : Send + Sync + 'static> Handler<AddWorker<T>> for EventHub<T> {
    type Result = MessageResult<AddWorker<T>>;

    fn handle(&mut self, msg: AddWorker<T>, ctx: &mut Self::Context) -> <Self as Handler<AddWorker<T>>>::Result {
        let worker_id = self.next_id();

        self.workers.insert(worker_id, msg.0);

        MessageResult(worker_id)
    }
}

pub struct EventHubWorker<T> where T : Send + Sync {
    worker_id : Option<u64>,
    subscribers : BTreeMap<String, Vec<(u64, Recipient<Event<T>>)> >
}

impl<T : Send + Sync> Default for EventHubWorker<T> {
    fn default() -> Self {
        Self {
            worker_id: None,
            subscribers: BTreeMap::new()
        }
    }
}

impl<T : 'static + Send + Sync> Actor for EventHubWorker<T> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        EventHub::from_registry()
            .send(AddWorker(ctx.address()))
            .map_err(|_e| ())
            .into_actor(self)
            .and_then(|id, mut act, ctx| {
                act.worker_id = Some(id);
                fut::ok(())
            })
            .wait(ctx)
    }
}

impl<T : 'static + Send + Sync> Supervised for EventHubWorker<T> {}
impl<T : 'static + Send + Sync> ArbiterService for EventHubWorker<T> { }

impl<T : 'static + Send + Sync> Handler<Event<T>> for EventHubWorker<T> {
    type Result = ();

    fn handle(&mut self, msg: Event<T>, ctx: &mut Self::Context) -> <Self as Handler<Event<T>>>::Result {
        let subscribers = match self.subscribers.get(msg.path()) {
            None => return (),
            Some(v) => v
        };
        for (_id, sub) in subscribers {
            sub.try_send(msg.clone())
        }
    }
}