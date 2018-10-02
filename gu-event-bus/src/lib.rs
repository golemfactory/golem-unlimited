//! Aplication event bus.
//!

extern crate actix;
extern crate futures;
extern crate gu_actix;
extern crate smallvec;

#[macro_use]
extern crate log;

use actix::{ArbiterService, Message};
use std::sync::Arc;
use std::fmt::Debug;

/// Empty event
pub struct Event<T> {
    inner: Arc<(String, T)>,
}

impl<T> Clone for Event<T> {
    fn clone(&self) -> Self {
        let inner = self.inner.clone();
        Self { inner }
    }
}

impl<T> Event<T> {
    pub fn path(&self) -> &str {
        self.inner.0.as_ref()
    }

    pub fn data(&self) -> &T {
        &self.inner.1
    }
}

impl<T> Message for Event<T> {
    type Result = ();
}

pub fn post_event<T: 'static + Send + Sync + Debug>(path: String, event_data: T) {
    debug!("sending event: {} => {:?}", &path, &event_data);

    let inner = Arc::new((path, event_data));

    actor::EventHubWorker::from_registry().do_send(Event { inner });
}

pub use actor::subscribe;

mod actor;
mod path;
