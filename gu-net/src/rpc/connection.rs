use actix::prelude::*;
use actix_web::{ws, Binary};
use futures::{stream::Stream, Async, Future};

// Przyjmuje komunikaty i kieruje je do zainteresowanych odbiorców.
//
struct MessageRouter;

impl Actor for MessageRouter {
    type Context = Context<Self>;
}

impl Default for MessageRouter {
    fn default() -> Self {
        MessageRouter
    }
}

trait MessageHandler<T> {
    type Result: Future;

    fn process(body: T) -> Self::Result;
}
