use super::context::{start_actor, RemotingContext};
use actix::prelude::*;
use futures::prelude::*;
use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    sync::Mutex,
};

struct RemotingRegistry {
    inner: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl Default for RemotingRegistry {
    fn default() -> Self {
        RemotingRegistry {
            inner: HashMap::new(),
        }
    }
}

impl RemotingRegistry {
    fn get<T>(&mut self) -> Addr<T>
    where
        T: Actor<Context = RemotingContext<T>> + Any + Default,
    {
        let type_id = TypeId::of::<T>();
        let (addr, is_new): (Addr<T>, bool) = match self.inner.get(&type_id) {
            Some(addr) => match Any::downcast_ref::<Addr<T>>(addr.as_ref()) {
                Some(v) => (v.clone(), false),
                None => panic!("unexpected downcast RemotingRegistry::GetAddr"),
            },
            None => (start_actor(T::default()), true),
        };

        if is_new {
            let r = self.inner.insert(type_id, Box::new(addr.clone()));
            assert!(r.is_none())
        }
        addr
    }
}

lazy_static! {
    static ref REGISTRY: Mutex<RemotingRegistry> = Mutex::new(RemotingRegistry::default());
}

// TODO: Add thread local cache

pub trait RemotingSystemService: Actor<Context = RemotingContext<Self>> + Default {
    fn from_registry() -> Addr<Self> {
        REGISTRY.lock().unwrap().get()
        //start_actor(Self::default())
        /*RemotingRegistry::from_registry().send(GetAddr::default())
            .wait().unwrap()
        */
    }
}
