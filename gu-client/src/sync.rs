use std::sync::mpsc;
use std::thread;

use actix::prelude::*;
use futures::prelude::*;

pub struct SystemHandle(Addr<Runner>);

struct Runner;

impl Actor for Runner {
    type Context = Context<Self>;
}

struct CallWith<TX, F>(TX, F);

impl<TX, F> Message for CallWith<TX, F> {
    type Result = Result<(), ()>;
}

pub fn start() -> SystemHandle {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let system = System::new("gu-client");

        let runner = Runner.start();

        let _ = tx.send(runner);

        let _ = system.run();
    });

    SystemHandle(rx.recv().unwrap())
}

impl SystemHandle {
    pub fn wait<F, FR>(&self, f: F) -> Result<FR::Item, FR::Error>
    where
        F: FnOnce() -> FR + Sync + Send + 'static,
        FR: Future + 'static,
        FR::Item: Sync + Send + 'static,
        FR::Error: Sync + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();

        self.0.do_send(CallWith(tx, f));
        rx.recv().unwrap()
    }
}

impl<FR, F> Handler<CallWith<mpsc::Sender<Result<FR::Item, FR::Error>>, F>> for Runner
where
    F: FnOnce() -> FR + 'static,
    FR: Future + 'static,
    FR::Item: Sync + Send + 'static,
    FR::Error: Sync + Send + 'static,
{
    type Result = ActorResponse<Runner, (), ()>;

    fn handle(
        &mut self,
        msg: CallWith<mpsc::Sender<Result<FR::Item, FR::Error>>, F>,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let tx = msg.0;
        let future = (msg.1)();

        ActorResponse::r#async(
            future
                .then(move |r| tx.send(r).map(|_| ()))
                .map_err(|_| ())
                .into_actor(self),
        )
    }
}
