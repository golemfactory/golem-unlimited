use actix::prelude::*;

struct RunOnce<F>
where
    F: FnOnce() + 'static,
{
    call: Option<F>,
}

impl<F> RunOnce<F>
where
    F: FnOnce() + 'static,
{
    fn new(call: F) -> Self {
        RunOnce { call: Some(call) }
    }
}

impl<F> Actor for RunOnce<F>
where
    F: FnOnce() + 'static,
{
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        match self.call.take() {
            Some(call) => call(),
            None => (),
        }
        ctx.stop();
    }
}

pub fn run_once<F>(f: F)
where
    F: FnOnce() + 'static,
{
    RunOnce::new(f).start();
}
