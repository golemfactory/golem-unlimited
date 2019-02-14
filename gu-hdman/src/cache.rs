use actix::prelude::*;
use futures::future::Shared;
use futures::prelude::*;
use futures::sync::oneshot;
use gu_actix::prelude::*;
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait CacheProvider {
    type Key: Eq + Hash + Clone + Send;
    type Hint : Send;
    type Value: Clone + Send;
    type Error: Clone + From<oneshot::Canceled> + From<MailboxError> + Send;
    type CheckResult: IntoFuture<Item = Option<Self::Value>, Error = Self::Error>;
    type FetchResult: IntoFuture<Item = Self::Value, Error = Self::Error>;

    fn try_get(&self, key: &Self::Key) -> Self::CheckResult;

    fn fetch(&mut self, key: Self::Key, hint : Self::Hint) -> Self::FetchResult;
}

pub fn resolve<P : CacheProvider + Default + Clone + 'static>(key : P::Key, hint : P::Hint) -> impl Future<Item =P::Value, Error=P::Error> {
    AsyncCache::<P>::from_registry().send(DoFetchOnce(key, hint)).flatten_fut()
}

struct AsyncCache<P: CacheProvider> {
    provider: P,
}

impl<P: CacheProvider + Default> Default for AsyncCache<P> {
    fn default() -> Self {
        let provider = P::default();
        AsyncCache { provider }
    }
}

impl<P: CacheProvider + Default + 'static> Actor for AsyncCache<P> {
    type Context = Context<Self>;
}

impl<P: CacheProvider + Default + 'static> Supervised for AsyncCache<P> {}
impl<P: CacheProvider + Default + 'static> ArbiterService for AsyncCache<P> {}

impl<P: CacheProvider + Default + 'static> Handler<DoFetch<P>> for AsyncCache<P> {
    type Result = ActorResponse<Self, P::Value, P::Error>;

    fn handle(&mut self, msg: DoFetch<P>, ctx: &mut Self::Context) -> Self::Result {
        ActorResponse::r#async(self.provider.fetch(msg.0, msg.1).into_future().into_actor(self))
    }
}

impl<P: CacheProvider + Default + Clone + 'static> Handler<DoFetchOnce<P>> for AsyncCache<P> {
    type Result = ActorResponse<Self, P::Value, P::Error>;

    fn handle(&mut self, msg: DoFetchOnce<P>, ctx: &mut Self::Context) -> Self::Result {
        ActorResponse::r#async(
            self.provider
                .try_get(&msg.0)
                .into_future()
                .into_actor(self)
                .and_then(|opt_v, act, ctx| match opt_v {
                    Some(v) => fut::Either::B(fut::ok(v)),
                    None => fut::Either::A(
                        CacheRegistry::from_registry()
                            .send(DoGet(msg.0, msg.1, ctx.address().recipient()))
                            .flatten_fut()
                            .into_actor(act),
                    ),
                }),
        )
    }
}

struct DoFetchOnce<P: CacheProvider>(P::Key, P::Hint);

impl<P: CacheProvider + 'static> Message for DoFetchOnce<P> {
    type Result = Result<P::Value, P::Error>;
}

impl<P: CacheProvider> AsyncCache<P> {}

struct CacheRegistry<P: CacheProvider> {
    provider: P,
    downloads: HashMap<P::Key, Vec<oneshot::Sender<Result<P::Value, P::Error>>>>,
}

impl<P: CacheProvider + Default> Default for CacheRegistry<P> {
    fn default() -> Self {
        let provider = P::default();
        let downloads = HashMap::new();
        CacheRegistry {
            provider,
            downloads,
        }
    }
}

impl<P: CacheProvider + 'static> Supervised for CacheRegistry<P> {}
impl<P: CacheProvider + Default + 'static> SystemService for CacheRegistry<P> {}

impl<P: CacheProvider> CacheRegistry<P> {
    fn result(&mut self, key: &P::Key, result: Result<P::Value, P::Error>) {
        let _ = self.downloads.remove(&key).and_then(|v| {
            for endpoint in v {
                match endpoint.send(result.clone()) {
                    Err(e) => log::debug!("notification fail"),
                    Ok(f) => (),
                }
            }
            Some(())
        });
    }
}

impl<P: CacheProvider + 'static> Actor for CacheRegistry<P> {
    type Context = Context<Self>;
}

struct DoFetch<P: CacheProvider>(P::Key, P::Hint);

impl<P: CacheProvider + 'static> Message for DoFetch<P> {
    type Result = Result<P::Value, P::Error>;
}

struct DoGet<P: CacheProvider + 'static>(P::Key, P::Hint, Recipient<DoFetch<P>>);

impl<P: CacheProvider + 'static> Message for DoGet<P> {
    type Result = Result<P::Value, P::Error>;
}

impl<P: CacheProvider + Clone + 'static> Handler<DoGet<P>> for CacheRegistry<P> {
    type Result = ActorResponse<Self, P::Value, P::Error>;

    fn handle(&mut self, msg: DoGet<P>, ctx: &mut Self::Context) -> Self::Result {
        if let Some(download) = self.downloads.get_mut(&msg.0) {
            let (tx, rx) = oneshot::channel();
            download.push(tx);

            return ActorResponse::r#async(rx.flatten_fut().into_actor(self));
        }

        let (tx, rx) = oneshot::channel();
        let DoGet(k, h, fetcher) = msg;

        let key = k.clone();
        ctx.spawn(
            fetcher
                .send(DoFetch(k.clone(), h))
                .flatten_fut()
                .into_actor(self)
                .then(move |v, act, ctx| fut::ok(act.result(&key, v))),
        );

        return ActorResponse::r#async(rx.flatten_fut().into_actor(self));
    }
}
