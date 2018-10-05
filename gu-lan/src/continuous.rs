use actix::prelude::*;
use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::Message;
use actix::Recipient;
use actor;
use actor::send_mdns_query;
use futures::sync::mpsc;
use service::ServiceDescription;
use service::ServiceInstance;
use service::ServicesDescription;
use std::collections::BinaryHeap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;
use std::cmp::Ordering;
use std::collections::HashMap;

static SKIP_INTERVAL_PERCENTAGE: u64 = 80;
static INTERVAL_MULTIPLIER: u32 = 5;
static MAX_INTERVAL: Duration = Duration::from_secs(2);
static START_INTERVAL: Duration = Duration::from_secs(1);
static CLEAR_MEMORY_PERIOD: u64 = 4;
static SERVICE_TTL: u64 = 4;

struct ExponentialNotify {
    interval: Duration,
    max_interval: Duration,
    last_query: Instant,
    own_last_query: Instant,
}

impl ExponentialNotify {
    fn new() -> ExponentialNotify {
        let now = Instant::now();
        ExponentialNotify {
            interval: START_INTERVAL,
            max_interval: MAX_INTERVAL,
            last_query: now,
            own_last_query: now,
        }
    }
}

impl ExponentialNotify {
    /// Returns None if it is time to send query,
    /// Otherwise it gives time interval after which querying should be considered
    fn query_time(&mut self) -> Option<Duration> {
        use std::ops::Mul;

        let percent =
            100 * (self.last_query - self.own_last_query).as_secs() / self.interval.as_secs();

        let now = Instant::now();
        self.last_query = now;
        self.own_last_query = now;

        let interval = self.interval;
        self.interval = self
            .interval
            .mul(INTERVAL_MULTIPLIER)
            .min(self.max_interval);

        if percent > SKIP_INTERVAL_PERCENTAGE {
            None
        } else {
            Some(interval)
        }
    }

    fn query_on_the_web(&mut self) {
        self.last_query = Instant::now();
    }
}

/// TODO: Node ID instead of the whole description
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
struct ServiceInstanceId {
    id: Vec<String>,
    host: String,
}

impl From<ServiceInstance> for ServiceInstanceId {
    fn from(instance: ServiceInstance) -> Self {
        Self {
            id: instance.txt,
            host: instance.host,
        }
    }
}

#[derive(Debug, Ord, PartialEq, Eq)]
struct HeapItem(Instant, ServiceInstanceId);

impl PartialOrd<Self> for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0).map(|a| a.reverse())
    }
}

impl From<(Instant, ServiceInstanceId)> for HeapItem {
    fn from((a, b): (Instant, ServiceInstanceId)) -> Self {
        HeapItem(a, b)
    }
}

struct MemoryManager {
    ttl: Duration,
    queue: BinaryHeap<HeapItem>,
    time_map: HashMap<ServiceInstanceId, Instant>,
    data_map: HashMap<ServiceInstanceId, ServiceInstance>,
}

impl MemoryManager {
    pub fn new(ttl: Duration) -> Self {
        MemoryManager {
            ttl,
            queue: BinaryHeap::new(),
            time_map: HashMap::new(),
            data_map: HashMap::new(),
        }
    }

    pub fn update(&mut self, data: ServiceInstance) -> Option<ServiceInstance> {
        let now = Instant::now();
        let id: ServiceInstanceId = data.clone().into();

        self.queue.push((now, id.clone()).into());
        let result = match self.time_map.insert(id.clone(), now) {
            Some(a) => None,
            None => Some(data.clone())
        };
        self.data_map.insert(id, data);

        result
    }

    fn conditionally_destroy_instance(&mut self, time: Instant) -> bool {
        use std::collections::binary_heap::PeekMut;
        use std::collections::hash_map::Entry;
        use std::ops::Add;

        match self.queue.peek_mut() {
            Some(top) => {
                let heap_top = top.0;

                if top.0.add(self.ttl) < time
                {
                    match self.time_map.entry(top.1.clone()) {
                        Entry::Occupied(a) => {
                            if *a.get() == top.0 {
                                a.remove_entry();
                                self.data_map.remove(&top.1);
                            }
                        }
                        _ => (),
                    }

                    PeekMut::pop(top);
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    pub fn clear_memory(&mut self) {
        let time = Instant::now();

        while self.conditionally_destroy_instance(time) {}
    }

    pub fn memory(&self) -> Vec<ServiceInstance> {
        self.data_map.values().map(|a| a.clone()).collect()
    }
}

struct RequestForMdnsQuery;

pub struct ForeignMdnsQueryInfo;

impl Message for RequestForMdnsQuery {
    type Result = ();
}

impl Message for ForeignMdnsQueryInfo {
    type Result = ();
}

impl Handler<ForeignMdnsQueryInfo> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, msg: ForeignMdnsQueryInfo, _ctx: &mut Context<Self>) -> () {
        self.notifier.query_on_the_web();
    }
}

pub struct ContinuousInstancesList {
    name: ServiceDescription,
    memory: MemoryManager,
    notifier: ExponentialNotify,
    sender: mpsc::Sender<((ServicesDescription, u16), SocketAddr)>,
    subscribers: HashSet<Recipient<NewInstance>>,
}

impl ContinuousInstancesList {
    pub fn new(
        name: ServiceDescription,
        sender: mpsc::Sender<((ServicesDescription, u16), SocketAddr)>,
    ) -> Self {
        ContinuousInstancesList {
            name,
            memory: MemoryManager::new(Duration::from_secs(SERVICE_TTL)),
            notifier: ExponentialNotify::new(),
            sender,
            subscribers: HashSet::new(),
        }
    }

    fn services(&self) -> ServicesDescription {
        ServicesDescription::new(vec![self.name.clone()])
    }
}

impl Actor for ContinuousInstancesList {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(Duration::from_secs(CLEAR_MEMORY_PERIOD), |act, _ctx| {
            act.memory.clear_memory()
        });

        fn query_loop(
            act: &mut ContinuousInstancesList,
            ctx: &mut Context<ContinuousInstancesList>,
        ) {
            let vec = act.services();
            ctx.spawn(
                send_mdns_query(Some(act.sender.clone()), vec, 0)
                    .into_actor(act)
                    .map_err(|e, _, _| error!("mDNS query error: {:?}", e)),
            );
            let dur = act.notifier.query_time().unwrap_or(Duration::from_secs(0));

            ctx.run_later(dur, query_loop);
        };

        query_loop(self, ctx);
        ctx.run_interval(Duration::from_secs(1), |act, _ctx| {
            act.memory.memory().into_iter().map(|a| a.host).for_each(|a| println!("{:?}", a));
            println!();
        });
    }
}

impl Handler<RequestForMdnsQuery> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, _msg: RequestForMdnsQuery, ctx: &mut Context<Self>) -> () {
        use futures::Future;
        ctx.spawn(
            actor::send_mdns_query(
                Some(self.sender.clone()),
                ServicesDescription::new(vec![self.name.clone()]),
                0,
            ).map_err(|_| ())
            .into_actor(self),
        );
    }
}

pub struct ReceivedMdnsInstance(ServiceInstance);

impl ReceivedMdnsInstance {
    pub fn new(s: ServiceInstance) -> Self {
        ReceivedMdnsInstance(s)
    }
}

pub struct NewInstance {
    pub data: ServiceInstance,
}

impl Message for NewInstance {
    type Result = ();
}

impl Message for ReceivedMdnsInstance {
    type Result = ();
}

impl Handler<ReceivedMdnsInstance> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, msg: ReceivedMdnsInstance, _ctx: &mut Context<Self>) -> () {
        let msg = msg.0;
        if let Some(inst) = self.memory.update(msg) {
            println!("SEND");
            for s in self.subscribers.clone() {
                s.do_send(NewInstance { data: inst.clone() });
            }
        }
    }
}

pub struct Subscribe {
    pub rec: Recipient<NewInstance>,
}

impl Message for Subscribe {
    type Result = ();
}

impl Handler<Subscribe> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, msg: Subscribe, ctx: &mut Context<Self>) -> () {
        self.subscribers.insert(msg.rec.clone());
        for inst in self.memory.memory() {
            msg.rec.do_send(NewInstance { data: inst.clone() });
        }
    }
}

pub struct Unsubscribe {
    rec: Recipient<NewInstance>,
}

impl Message for Unsubscribe {
    type Result = ();
}

impl Handler<Unsubscribe> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, msg: Unsubscribe, ctx: &mut Context<Self>) -> () {
        self.subscribers.remove(&msg.rec);

        if self.subscribers.is_empty() {
            ctx.stop()
        }
    }
}