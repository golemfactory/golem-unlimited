
use gu_base::{Module, Decorator, ArgMatches};
use futures::prelude::*;
use actix::prelude::*;
use actix_web::{self, App, HttpRequest, HttpResponse, Responder, HttpMessage, http};
use super::plugins::PluginEvent;
use gu_event_bus;
use std::sync::{Arc, RwLock};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct ServiceConfig {
    url : String,
    mount_point : String,
}

pub fn module() -> impl Module {
    ProxyModule { inner: Arc::new(RwLock::new(BTreeMap::new())), manager: None}
}

struct ProxyManager {
    inner : Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>
}

impl ProxyManager {

    fn configure<I>(&mut self, name : &str, it : I) where I : Iterator<Item=ServiceConfig> {
        let mut w = self.inner.write().unwrap();

        let map : BTreeMap<String, String> = it.map(|cfg| (cfg.mount_point, cfg.url)).collect();

        w.insert(name.to_string(), map);
    }

    fn unconfigure(&mut self, name : &str) {
        let mut w = self.inner.write().unwrap();

        w.remove(name);
    }
}

impl Actor for ProxyManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut <Self as Actor>::Context) {
        let f= gu_event_bus::subscribe("/plugins".into(), ctx.address().recipient())
            .and_then(|id| Ok(debug!("id={}", id)))
            .map_err(|_| ())
            .into_actor(self);
        eprintln!("ProxyManager [STARTING]");
        ctx.wait(f);
        eprintln!("ProxyManager [STARTED]")
    }
}

impl Handler<gu_event_bus::Event<PluginEvent>> for ProxyManager {
    type Result = ();

    fn handle(&mut self, msg: gu_event_bus::Event<PluginEvent>, ctx: &mut Self::Context) -> Self::Result {
        eprintln!("got event: {:?}", msg.data());
        match msg.data() {
            PluginEvent::New(plugin_meta) => {
                let config : Vec<ServiceConfig> = plugin_meta.service("gu-proxy");
                self.configure(plugin_meta.name(), config.into_iter());
            }
            PluginEvent::Drop(name) => self.unconfigure(&name)
        }
    }
}

struct ProxyModule {
    // <plugin> -> <path> -> <url>
    inner : Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>,
    manager : Option<Addr<ProxyManager>>
}

impl Module for ProxyModule {
    fn args_consume(&mut self, _matches: &ArgMatches) -> bool {
        let inner = self.inner.clone();
        self.manager = Some(ProxyManager { inner}.start());
        false
    }


    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        let inner = self.inner.clone();

        app.handler("/service/proxy", move |r : &HttpRequest<_>| -> HttpResponse {
            let tail = (&r.path()[15..]).to_string();
            if let Some(spos) = tail.find("/") {
                let plugin_name = &tail[0..spos];
                let mount_point = &tail[spos+1..];

                let url = {
                    inner.read().unwrap().get(plugin_name).and_then(|f| f.get(mount_point))
                        .map(|s| s.clone())
                };
                let body = format!("plugin={} / mount_point={}, url={:?}", plugin_name, mount_point, url);
                HttpResponse::Ok().body(body)
            }
            else {
                HttpResponse::NotFound().body("err")
            }
        })
    }


}

struct ProxyHandler(Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>);

