use actix::prelude::*;
use actix::Actor;
use actix::Context;
use actix::Handler;
use actix::Message;
use actix::Recipient;
use actor::send_mdns_query;
use futures::sync::mpsc;
use rand::thread_rng;
use rand::Rng;
use rand::ThreadRng;
use service::ServiceDescription;
use service::ServiceInstance;
use service::ServicesDescription;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;
use std::time::Instant;
use futures::Future;

static SKIP_INTERVAL_PERCENTAGE: u64 = 80;
static INTERVAL_MULTIPLIER: u32 = 3;
static MAX_INTERVAL: Duration = Duration::from_secs(10);
static START_INTERVAL: Duration = Duration::from_secs(1);
static CLEAR_MEMORY_PERIOD: u64 = 1;
static SERVICE_TTL: u64 = 60;

struct ExponentialNotify {
    interval: Duration,
    max_interval: Duration,
    last_query: Instant,
    rng: Option<ThreadRng>,
}

impl ExponentialNotify {
    fn new() -> ExponentialNotify {
        let now = Instant::now();
        ExponentialNotify {
            interval: START_INTERVAL,
            max_interval: MAX_INTERVAL,
            last_query: now,
            rng: None,
        }
    }
}

impl ExponentialNotify {
    /// Returns Some(()) in first element if it is time to send query,
    /// Second parameter informs about time when this method should be called again
    fn query_time(&mut self) -> (Option<()>, Duration) {
        use std::ops::{Div, Mul};

        // initial wait
        if self.rng.is_none() {
            self.rng = Some(thread_rng());

            self.interval = Duration::from_millis(self.rng.clone().unwrap().gen_range(20, 120));


            return (None, self.interval);
        }

        let now = Instant::now();
        let diff = now - self.last_query;

        let interval_ms = self.interval.subsec_millis() as u64 + 1000 * self.interval.as_secs();
        let diff_ms = diff.subsec_millis() as u64 + 1000 * diff.as_secs();

        let percent =
            100 * diff_ms / interval_ms;

        // increase interval
        self.interval = self
            .interval
            .mul(INTERVAL_MULTIPLIER)
            .min(self.max_interval)
            .mul(1000)
            .div(self.rng.clone().unwrap().gen_range(900, 1100));

        if percent > SKIP_INTERVAL_PERCENTAGE {
            //println!("Less: {:?}", self.interval);

            self.last_query = now;

            (Some(()), self.interval)
        } else {
            //println!("Greater: {:?}", self.interval);
            (None, self.interval - diff)
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
        println!("Update: {:?}", data.name);
        let now = Instant::now();
        let id: ServiceInstanceId = data.clone().into();

        self.queue.push((now, id.clone()).into());
        let result = match self.time_map.insert(id.clone(), now) {
            Some(_) => None,
            None => Some(data.clone()),
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
                if top.0.add(self.ttl) < time {
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

pub struct ForeignMdnsQueryInfo;

impl Message for ForeignMdnsQueryInfo {
    type Result = ();
}

impl Handler<ForeignMdnsQueryInfo> for ContinuousInstancesList {
    type Result = ();

    fn handle(&mut self, _msg: ForeignMdnsQueryInfo, _ctx: &mut Context<Self>) -> () {
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

    fn service(&self) -> ServicesDescription {
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
            let vec = act.service();
            let time = act.notifier.query_time();


            match time.0 {
                 Some(_) => {
                    println!("Query: {:?}", vec);
                    //println!("Sender: {:?}", act.sender.clone());

                    ctx.spawn(
                        send_mdns_query(Some(act.sender.clone()), vec.clone(), 0)
                            .map_err(|e| error!("mDNS query error: {:?}", e))
                            .into_actor(act)
                    );
                }
                _ => (),

            };

            ctx.run_later(time.1, query_loop);
        };

        query_loop(self, ctx);
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
            for s in self.subscribers.clone() {
                let _ = s.do_send(NewInstance { data: inst.clone() }).map_err(|e| error!("error: {:?}", e));
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

    fn handle(&mut self, msg: Subscribe, _ctx: &mut Context<Self>) -> () {
        self.subscribers.insert(msg.rec.clone());
        for inst in self.memory.memory() {
            let _ = msg.rec.do_send(NewInstance { data: inst.clone() }).map_err(|e| error!("error: {:?}", e));
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
