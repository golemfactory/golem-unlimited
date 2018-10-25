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
    url: String,
    mount_point: String,
}

pub fn module() -> impl Module {
    ProxyModule {
        inner: Arc::new(RwLock::new(BTreeMap::new())),
        manager: None,
    }
}

struct ProxyManager {
    inner: Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>,
}

impl ProxyManager {
    fn configure<I>(&mut self, name: &str, it: I)
    where
        I: Iterator<Item = ServiceConfig>,
    {
        let mut w = self.inner.write().unwrap();

        let map: BTreeMap<String, String> = it.map(|cfg| (cfg.mount_point, cfg.url)).collect();

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

    fn handle(
        &mut self,
        msg: gu_event_bus::Event<PluginEvent>,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg.data() {
            PluginEvent::New(plugin_meta) => {
                let config: Vec<ServiceConfig> = plugin_meta.service("gu-proxy");
                self.configure(plugin_meta.name(), config.into_iter());
            }
            PluginEvent::Drop(name) => self.unconfigure(&name),
        }
    }
}

struct ProxyModule {
    // <plugin> -> <path> -> <url>
    inner: Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>,
    manager: Option<Addr<ProxyManager>>,
}

impl Module for ProxyModule {
    fn args_consume(&mut self, _matches: &ArgMatches) -> bool {
        let inner = self.inner.clone();
        self.manager = Some(ProxyManager { inner }.start());
        false
    }

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        let inner = self.inner.clone();

        app.handler("/service/proxy", move |r : &HttpRequest<_>| -> Box<Future<Item = HttpResponse, Error = actix_web::Error>> {
            let tail = (&r.path()[15..]).to_string();
            if let Some(spos) = tail.find("/") {
                let plugin_name = &tail[0..spos];
                let mount_point = &tail[spos+1..];

                let url_opt = {
                    inner.read().unwrap().get(plugin_name).and_then(|f| f.get(mount_point))
                        .map(|s| s.clone())
                };

                use actix_web::client;

                if let Some(url) = url_opt {
                    return client::ClientRequest::get(url)
                        .header("User-Agent", "Mozilla/5.0 (Windows NT 6.1; Win64; x64)")
                        .finish().unwrap()
                        .send()
                        .map_err(actix_web::error::Error::from)
                        .and_then(|resp| {
                            Ok(HttpResponse::Ok().body(actix_web::Body::Streaming(Box::new(resp.payload().from_err()))))
                        }).responder()
                }
                //let body = format!("plugin={} / mount_point={}, url={:?}", plugin_name, mount_point, url);
                //HttpResponse::Ok().body(body)
            }
            future::ok(HttpResponse::NotFound().body("err")).responder()
        })
    }
}

struct ProxyHandler(Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>);
