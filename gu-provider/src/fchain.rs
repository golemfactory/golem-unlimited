use actix::ActorFuture;
use futures::{Async, Future};
use serde::export::PhantomData;
use std::iter::IntoIterator;
use std::mem;

struct FChain<Input, Output, F> {
    current: Option<Box<dyn Future<Item = Output, Error = Output>>>,
    chain: <Vec<Input> as IntoIterator>::IntoIter,
    result: Vec<Output>,
    process: F,
}

pub fn process_chain<
    Input,
    Output,
    F: FnMut(Input) -> Box<dyn Future<Item = Output, Error = Output>>,
>(
    input: Vec<Input>,
    mut process: F,
) -> impl Future<Item = Vec<Output>, Error = Vec<Output>> {
    let mut chain = input.into_iter();
    let current = chain.next().map(|input| (process)(input));
    let result = Default::default();
    FChain {
        current,
        chain,
        result,
        process,
    }
}

impl<Input, Output, F> Future for FChain<Input, Output, F>
where
    F: FnMut(Input) -> Box<dyn Future<Item = Output, Error = Output>>,
{
    type Item = Vec<Output>;
    type Error = Vec<Output>;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        while let Some(mut current) = self.current.take() {
            let r = match current.poll() {
                Ok(v) => v,
                Err(e) => {
                    let mut result = mem::replace(&mut self.result, Default::default());
                    result.push(e);
                    return Err(result);
                }
            };
            match r {
                Async::NotReady => {
                    self.current = Some(current);
                    return Ok(Async::NotReady);
                }
                Async::Ready(output) => {
                    self.result.push(output);
                    self.current = self.chain.next().map(|input| (self.process)(input));
                }
            }
        }
        Ok(Async::Ready(mem::replace(
            &mut self.result,
            Default::default(),
        )))
    }
}

struct FuChain<Actor, Input, Output, F> {
    current: Option<Box<dyn Future<Item = Output, Error = Output>>>,
    chain: <Vec<Input> as IntoIterator>::IntoIter,
    result: Vec<Output>,
    process: F,
    actor: PhantomData<Actor>,
}

impl<Input, Output, Actor: actix::Actor, F> ActorFuture for FuChain<Actor, Input, Output, F>
where
    F: FnMut(
        Input,
        &mut Actor,
        &mut Actor::Context,
    ) -> Box<dyn Future<Item = Output, Error = Output>>,
{
    type Item = Vec<Output>;
    type Error = Vec<Output>;
    type Actor = Actor;

    fn poll(
        &mut self,
        srv: &mut Self::Actor,
        ctx: &mut Actor::Context,
    ) -> Result<Async<Self::Item>, Self::Error> {
        while let Some(mut current) = self.current.take() {
            let r = match current.poll() {
                Ok(v) => v,
                Err(e) => {
                    let mut result = mem::replace(&mut self.result, Default::default());
                    result.push(e);
                    return Err(result);
                }
            };
            match r {
                Async::NotReady => {
                    self.current = Some(current);
                    return Ok(Async::NotReady);
                }
                Async::Ready(output) => {
                    self.result.push(output);
                    self.current = self
                        .chain
                        .next()
                        .map(|input| (self.process)(input, srv, ctx));
                }
            }
        }
        Ok(Async::Ready(mem::replace(
            &mut self.result,
            Default::default(),
        )))
    }
}

pub fn process_chain_act<
    Actor: actix::Actor,
    Input,
    Output,
    F: FnMut(Input, &mut Actor, &mut Actor::Context) -> Box<dyn Future<Item = Output, Error = Output>>,
>(
    act: &mut Actor,
    ctx: &mut Actor::Context,
    input: Vec<Input>,
    mut process: F,
) -> impl ActorFuture<Actor = Actor, Item = Vec<Output>, Error = Vec<Output>> {
    let mut chain = input.into_iter();
    let current = chain.next().map(|input| (process)(input, act, ctx));
    let result = Default::default();
    let actor = PhantomData;
    FuChain {
        current,
        chain,
        result,
        process,
        actor,
    }
}
