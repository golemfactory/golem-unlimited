use actix::fut;
use actix::prelude::*;
use futures::future;
use futures::prelude::*;
use gu_actix::prelude::*;
use std::collections::BTreeMap;
//use std::sync::Arc;
//use std::any::{TypeId, Any};
//use smallvec::SmallVec;
use super::path::EventPath;
use super::Event;
use std::cmp;

/* TODO: Auto unscbscribe
struct Subscription {
    subscription_id : u64
}
*/

struct EventHub<T: 'static + Send + Sync> {
    last_id: u64,
    workers: BTreeMap<u64, Addr<EventHubWorker<T>>>,
    subscribers: BTreeMap<u64, (String, Recipient<Event<T>>)>,
}

impl<T: 'static + Send + Sync> Default for EventHub<T> {
    fn default() -> Self {
        Self {
            last_id: 1,
            workers: BTreeMap::new(),
            subscribers: BTreeMap::new(),
        }
    }
}

impl<T: 'static + Send + Sync> EventHub<T> {
    #[inline]
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;

        self.last_id
    }
}

impl<T: 'static + Send + Sync> Actor for EventHub<T> {
    type Context = Context<Self>;
}

impl<T: 'static + Send + Sync> Supervised for EventHub<T> {}
impl<T: 'static + Send + Sync> SystemService for EventHub<T> {}

struct AddWorker<T: 'static + Send + Sync>(Addr<EventHubWorker<T>>);

struct WorkerInitData<T: 'static + Send + Sync> {
    worker_id: u64,
    subscribers: Vec<(u64, String, Recipient<Event<T>>)>,
}

impl<T: 'static + Send + Sync> Message for AddWorker<T> {
    type Result = WorkerInitData<T>;
}

impl<T: Send + Sync + 'static> Handler<AddWorker<T>> for EventHub<T> {
    type Result = MessageResult<AddWorker<T>>;

    fn handle(
        &mut self,
        msg: AddWorker<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<AddWorker<T>>>::Result {
        let worker_id = self.next_id();

        self.workers.insert(worker_id, msg.0);

        let subscribers = self
            .subscribers
            .iter()
            .map(|(sub_id, (path, addr))| (*sub_id, path.clone(), addr.clone()))
            .collect();

        MessageResult(WorkerInitData {
            worker_id,
            subscribers,
        })
    }
}

struct Subscribe<T: 'static + Send + Sync> {
    path: String,
    addr: Recipient<Event<T>>,
}

impl<T: 'static + Send + Sync> Message for Subscribe<T> {
    type Result = Result<u64, ()>;
}

impl<T: 'static + Send + Sync> Handler<Subscribe<T>> for EventHub<T> {
    type Result = ActorResponse<EventHub<T>, u64, ()>;

    fn handle(
        &mut self,
        msg: Subscribe<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<Subscribe<T>>>::Result {
        let sub_id = self.next_id();

        self.subscribers
            .insert(sub_id, (msg.path.clone(), msg.addr.clone()));

        let f: Vec<_> = self
            .workers
            .values()
            .map(move |worker| worker.send(AddSubscriber::from_subscribe(sub_id, &msg)))
            .collect();

        ActorResponse::async(
            future::join_all(f)
                .map_err(|_| ())
                .and_then(move |_r| Ok(sub_id))
                .into_actor(self),
        )
    }
}

struct AddSubscriber<T: Send + Sync> {
    sub_id: u64,
    path: String,
    addr: Recipient<Event<T>>,
}

impl<T: Send + Sync> AddSubscriber<T> {
    fn from_subscribe(sub_id: u64, subscribe: &Subscribe<T>) -> Self {
        Self {
            sub_id,
            path: subscribe.path.clone(),
            addr: subscribe.addr.clone(),
        }
    }
}

impl<T: Send + Sync> Message for AddSubscriber<T> {
    type Result = ();
}

pub struct EventHubWorker<T>
where
    T: Send + Sync,
{
    worker_id: Option<u64>,
    subscribers: BTreeMap<String, Vec<(u64, Recipient<Event<T>>)>>,
}

impl<T: Send + Sync> Default for EventHubWorker<T> {
    fn default() -> Self {
        Self {
            worker_id: None,
            subscribers: BTreeMap::new(),
        }
    }
}

impl<T: 'static + Send + Sync> Actor for EventHubWorker<T> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        EventHub::from_registry()
            .send(AddWorker(ctx.address()))
            .map_err(|_e| ())
            .into_actor(self)
            .and_then(|data, act: &mut _, _ctx| {
                act.worker_id = Some(data.worker_id);
                for (sub_id, path, addr) in data.subscribers {
                    upsert_value(&mut act.subscribers, path, (sub_id, addr))
                }
                fut::ok(())
            }).wait(ctx)
    }
}

impl<T: 'static + Send + Sync> Supervised for EventHubWorker<T> {}
impl<T: 'static + Send + Sync> ArbiterService for EventHubWorker<T> {}

impl<T: 'static + Send + Sync> Handler<Event<T>> for EventHubWorker<T> {
    type Result = ();

    fn handle(
        &mut self,
        msg: Event<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<Event<T>>>::Result {
        let path: EventPath = msg.path().into();
        for path_part in path.iter() {
            let subscribers = match self.subscribers.get_mut(path_part) {
                None => return (),
                Some(v) => v,
            };
            subscribers.retain(|(id, sub)| match sub.try_send(msg.clone()) {
                Err(SendError::Closed(_)) => {
                    warn!("removing closed subscriber: {}", id);
                    false
                }
                Err(e) => {
                    error!("{}", e);
                    true
                }
                Ok(()) => true,
            })
        }
    }
}

impl<T: 'static + Send + Sync> Handler<AddSubscriber<T>> for EventHubWorker<T> {
    type Result = ();

    fn handle(
        &mut self,
        msg: AddSubscriber<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<AddSubscriber<T>>>::Result {
        upsert_value(&mut self.subscribers, msg.path, (msg.sub_id, msg.addr))
    }
}

fn upsert_value<K, V>(map: &mut BTreeMap<K, Vec<V>>, k: K, val: V)
where
    K: cmp::Ord,
{
    if let Some(v) = map.get_mut(&k) {
        return v.push(val);
    }
    let mut v = Vec::new();
    v.push(val);
    map.insert(k, v);
}

pub fn subscribe<T: 'static + Sync + Send>(
    path: String,
    addr: Recipient<Event<T>>,
) -> impl Future<Item = u64> {
    EventHub::from_registry()
        .send(Subscribe { path, addr })
        .map_err(|_| ())
        .flatten_fut()
}
