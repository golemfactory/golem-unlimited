use actix::prelude::*;
use futures::prelude::*;
use futures::unsync::oneshot;
use std::cell::Cell;
use std::cell::Ref;
use std::cell::RefCell;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

pub trait AsyncRelease: Send + 'static {
    type Result: Future<Item = ()> + 'static;

    fn release(self) -> Self::Result;
}

pub struct Handle<T: AsyncRelease>(Arc<Inner<T>>);

impl<T: AsyncRelease> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle(self.0.clone())
    }
}

impl<T: AsyncRelease + fmt::Debug> fmt::Debug for Handle<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.0.deref_inner().fmt(fmt)
    }
}

struct Inner<T: AsyncRelease> {
    inner: Option<T>,
}

impl<T: AsyncRelease> Inner<T> {
    fn new(inner: T) -> Self {
        Inner { inner: Some(inner) }
    }

    fn deref_inner(&self) -> &T {
        self.inner.as_ref().unwrap()
    }

    pub fn into_inner(mut self) -> T {
        self.inner.take().unwrap()
    }
}

impl<T: AsyncRelease> Handle<T> {
    #[inline]
    pub fn new(resource: T) -> Handle<T> {
        Handle(Arc::new(Inner::new(resource)))
    }

    #[inline]
    pub fn into_inner(self) -> Option<T> {
        match Arc::try_unwrap(self.0) {
            Ok(mut inner) => inner.inner.take(),
            _ => None,
        }
    }
}

impl<T: AsyncRelease> From<T> for Handle<T> {
    fn from(resource: T) -> Self {
        Handle::new(resource)
    }
}

impl<T: AsyncRelease> Deref for Handle<T> {
    type Target = T;

    fn deref(&self) -> &<Self as Deref>::Target {
        self.0.deref().deref_inner()
    }
}

impl<T: AsyncRelease> Drop for Inner<T> {
    fn drop(&mut self) {
        if let Some(h) = self.inner.take() {
            AsyncResourceManager::from_registry().do_send(DropHandle(h))
        }
    }
}

#[derive(Default)]
struct AsyncResourceManager {
    pending_orders: u64,
    wait_handle: Option<oneshot::Receiver<()>>,
    send_handle: Option<oneshot::Sender<()>>,
}

impl Actor for AsyncResourceManager {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut <Self as Actor>::Context) {
        //eprintln!("rm start");
    }

    fn stopped(&mut self, ctx: &mut <Self as Actor>::Context) {
        //eprintln!("rm stop");
        if let Some(wait_handle) = self.wait_handle.take() {
            ctx.wait(
                wait_handle
                    .and_then(|_| Ok(()))
                    .map_err(|_| ())
                    .into_actor(self),
            )
        }
    }
}

impl Supervised for AsyncResourceManager {}
impl ArbiterService for AsyncResourceManager {}

impl AsyncResourceManager {
    fn inc(&mut self) {
        self.pending_orders += 1;
        //eprintln!("inc {}", self.pending_orders);
        if self.pending_orders == 1 {
            let (tx, rx) = oneshot::channel();

            self.wait_handle = Some(rx);
            self.send_handle = Some(tx);
        }
    }

    fn dec(&mut self) {
        self.pending_orders -= 1;
        //eprintln!("dec {}", self.pending_orders);
        if self.pending_orders == 0 {
            if let Some(tx) = self.send_handle.take() {
                tx.send(()).unwrap()
            }
        }
    }
}

struct DropHandle<T: AsyncRelease>(T);

impl<T: AsyncRelease> Message for DropHandle<T> {
    type Result = Result<(), ()>;
}

impl<T: AsyncRelease> Handler<DropHandle<T>> for AsyncResourceManager {
    type Result = ActorResponse<AsyncResourceManager, (), ()>;

    fn handle(
        &mut self,
        msg: DropHandle<T>,
        _ctx: &mut Self::Context,
    ) -> <Self as Handler<DropHandle<T>>>::Result {
        self.inc();
        ActorResponse::r#async(msg.0.release().into_actor(self).then(|_, act, _ctx| {
            act.dec();
            fut::ok(())
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::time::Duration;
    use tokio_timer::sleep;

    static counter: AtomicIsize = AtomicIsize::new(0);

    struct T;

    fn new_item() -> T {
        let _ = counter.fetch_add(1, Ordering::SeqCst);
        T
    }

    impl AsyncRelease for T {
        type Result = Box<Future<Item = (), Error = tokio_timer::Error> + Send + 'static>;

        fn release(self) -> <Self as AsyncRelease>::Result {
            Box::new(sleep(Duration::from_millis(100)).and_then(|_| {
                let _ = counter.fetch_add(-1, Ordering::SeqCst);
                Ok(())
            }))
        }
    }

    #[test]
    fn test_release() {
        let _ = System::run(|| {
            {
                let a = Handle::new(new_item());
                eprintln!("c={}", counter.load(Ordering::Relaxed));
                let _b = Handle::new(new_item());
                eprintln!("c={}", counter.load(Ordering::Relaxed));
                let _c = a.clone();
                eprintln!("c={}", counter.load(Ordering::Relaxed));
            }
            eprintln!("c={}", counter.load(Ordering::Relaxed));
            Arbiter::spawn(
                tokio_timer::sleep(Duration::from_millis(200))
                    .and_then(|_| {
                        System::current().stop();
                        Ok(())
                    })
                    .map_err(|_| ()),
            )
        });

        use std::thread::sleep;
        sleep(Duration::from_secs(1));
        assert_eq!(0, counter.load(Ordering::Relaxed))
    }
}
