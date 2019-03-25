#![allow(dead_code)]

use super::plugins::{self, ListPlugins, PluginEvent, PluginManager, PluginStatus};
use actix::{fut, prelude::*};
use actix_web::{self, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse};
use futures::{future, prelude::*};
use gu_base::{ArgMatches, Module};
use gu_event_bus;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct ServiceConfig {
    cmd: String
}

pub fn module() -> impl Module {
    LocalServiceModule {
        inner: Arc::new(RwLock::new(BTreeMap::new())),
        manager: None,
    }
}

struct ProxyManager {
    inner: Arc<RwLock<BTreeMap<String, BTreeMap<String, ServiceRunner>>>>,
}

impl ProxyManager {
    fn configure<I>(&mut self, name: &str, it: I)
    where
        I: Iterator<Item = ServiceConfig>,
    {
        let mut w = self.inner.write().unwrap();

        let map: BTreeMap<String, ServiceRunner> = it.map(|cfg| (cfg.cmd.clone(), ServiceRunner::new(cfg))).collect();

        w.insert(name.to_string(), map);
        debug!("w={:?}", *w);
    }

    fn unconfigure(&mut self, name: &str) {
        let mut w = self.inner.write().unwrap();

        w.remove(name);
    }
}

impl Actor for ProxyManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let subscribe_future =
            gu_event_bus::subscribe("/plugins".into(), ctx.address().recipient())
                .and_then(|id| Ok(debug!("id={}", id)))
                .map_err(|_| ());
        let list_plugins = subscribe_future.and_then(|_| {
            PluginManager::from_registry()
                .send(ListPlugins)
                .map_err(|_| ())
        });
        ctx.wait(
            list_plugins
                .into_actor(self)
                .and_then(|plugins: Vec<_>, act, _ctx| {
                    debug!("plugins={:?}", &plugins);
                    plugins
                        .iter()
                        .filter(|plugin| plugin.status() == PluginStatus::Active)
                        .filter_map(|plugin| {
                            let meta: &plugins::PluginMetadata = plugin.metadata();
                            let name = meta.name();
                            let service = meta.service("gu-proxy");
                            if service.is_empty() {
                                None
                            } else {
                                Some((name, service))
                            }
                        })
                        .for_each(|(name, service)| act.configure(name, service.into_iter()));
                    fut::ok(())
                }),
        );
    }
}

impl Handler<gu_event_bus::Event<PluginEvent>> for ProxyManager {
    type Result = ();

    fn handle(&mut self, msg: gu_event_bus::Event<PluginEvent>, _ctx: &mut Self::Context) -> () {
        match msg.data() {
            PluginEvent::New(plugin_meta) => {
                let config: Vec<ServiceConfig> = plugin_meta.service("gu-proxy");
                self.configure(plugin_meta.name(), config.into_iter());
            }
            PluginEvent::Drop(name) => self.unconfigure(&name),
        }
    }
}

struct LocalServiceModule {
    // <plugin> -> <path> -> <url>
    inner: Arc<RwLock<BTreeMap<String, BTreeMap<String, ServiceRunner>>>>,
    manager: Option<Addr<ProxyManager>>,
}

impl Module for LocalServiceModule {
    fn args_consume(&mut self, _matches: &ArgMatches) -> bool {
        let inner = self.inner.clone();
        self.manager = Some(ProxyManager { inner }.start());
        false
    }

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        let inner = self.inner.clone();

        app.handler("/service/local", move |r : &HttpRequest<_>| -> Box<Future<Item = HttpResponse, Error = actix_web::Error>> {
            let tail = (&r.path()[15..]).to_string();
            if let Some(spos) = tail.find("/") {
                let plugin_name = &tail[0..spos];
                let mount_point = &tail[spos+1..];

                unimplemented!()
            }
            future::ok(HttpResponse::NotFound().body("err")).responder()
        })
    }
}

struct ProxyHandler(Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>);




#[derive(Debug)]
struct ServiceRunner {

}


impl ServiceRunner {
    fn new(config : ServiceConfig) -> Self {
        ServiceRunner {}
    }
}