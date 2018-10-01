//! Aplication event bus.
//!

extern crate actix;
extern crate futures;

use std::sync::Arc;
use actix::Message;

/// Empty event
#[derive(Clone)]
pub struct Event<T> {
    inner : Arc<(String, T)>
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

pub fn post_event<T>(path : &str, event_data : T) {

}



mod actor;