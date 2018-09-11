extern crate actix;
extern crate actix_web;
extern crate clap;
extern crate futures;

#[macro_use]
extern crate log;
extern crate env_logger;

pub use clap::{App, Arg, ArgMatches, SubCommand};
use futures::future;
use futures::prelude::*;
use std::sync::Arc;

pub trait Decorator : Clone + Sync + Send {

    fn decorate_webapp<S : 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S>;
}

pub trait Module {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app
    }

    fn args_complete<F>(&self, matches: &ArgMatches, app_gen: &F) -> bool where
        F: Fn() -> App<'static, 'static>, {
        false
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool
    {
        false
    }

    fn prepare(&mut self) -> Box<Future<Item = (), Error = ()>> {
        Box::new(future::ok(()))
    }

    fn run<D : Decorator + Clone + 'static>(&self, decorator : D) {

    }

    fn decorate_webapp<S : 'static>(&self, app : actix_web::App<S>) ->  actix_web::App<S> {
        app
    }
}

impl<M : Module + Sync + Send> Decorator for Arc<M> {
    fn decorate_webapp<S : 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        (**self).decorate_webapp(app)
    }
}

pub trait ModuleChain<M>
where
    M: Module,
{
    type Output: Module;

    fn chain(self, rhs: M) -> Self::Output;
}

impl<M1, M2> ModuleChain<M2> for M1
where
    M1: Module,
    M2: Module,
{
    type Output = ChainModule<M1, M2>;

    fn chain(self, rhs: M2) -> Self::Output {
        ChainModule { m1: self, m2: rhs }
    }
}

pub struct ChainModule<M1: Module, M2: Module> {
    m1: M1,
    m2: M2,
}

impl<M1, M2> Module for ChainModule<M1, M2>
where
    M1: Module,
    M2: Module,
{
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        let app = self.m1.args_declare(app);
        self.m2.args_declare(app)
    }

    fn args_complete<F>(&self, matches: &ArgMatches, app_gen: &F) -> bool where
        F: Fn() -> App<'static, 'static>, {
        if self.m1.args_complete(matches, app_gen) {
            true
        }
        else if self.m2.args_complete(matches, app_gen) {
            true
        }
        else {
            false
        }
    }


    fn args_consume(&mut self, matches: &ArgMatches) -> bool
    {
        let b1 = self.m1.args_consume(matches);
        let b2 = self.m2.args_consume(matches);

        b1 || b2
    }

    fn prepare(&mut self) -> Box<Future<Item = (), Error = ()>> {
        Box::new(self.m1.prepare().join(self.m2.prepare()).map(|(_, _)| ()))
    }

    fn run<D: Decorator + Clone + 'static>(&self, decorator: D) {
        self.m1.run(decorator.clone());
        self.m2.run(decorator);
    }

    #[inline]
    fn decorate_webapp<S : 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        let app = self.m1.decorate_webapp(app);
        self.m2.decorate_webapp(app)
    }
}

mod output;

pub use output::{LogModule, CompleteModule};
use std::any::Any;

pub struct GuApp<F>(pub F) where F : Fn() -> App<'static, 'static>;

impl<F> GuApp<F> where F : Fn() -> App<'static, 'static> {

    pub fn run<M : Module + 'static + Sync + Send>(&mut self, mut module : M)
    {
        let matches = module.args_declare(self.0()).get_matches();

        if ! (module.args_complete(&matches, &|| module.args_declare(self.0())) ||
            module.args_consume(&matches)) {
            eprintln!("{}", matches.usage());
        }
        else {
            let rcmod = Arc::new(module);
            rcmod.run(rcmod.clone())
        }
    }

}

