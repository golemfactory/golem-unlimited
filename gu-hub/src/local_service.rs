#![allow(dead_code)]

use super::plugins::{self, ListPlugins, PluginEvent, PluginManager, PluginStatus};
use actix::{fut, prelude::*};
use actix_web::Responder;
use actix_web::{self, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse, Json};
use futures::{future, prelude::*};
use gu_base::{ArgMatches, Module};
use gu_event_bus;
use std::collections::BTreeSet;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
struct ServiceConfig {
    cmd: String,
}

pub fn module() -> impl Module {
    LocalServiceModule {
        plugin_commands: Arc::new(RwLock::new(BTreeMap::new())),
        command_proxy_path: Arc::new(RwLock::new(BTreeMap::new())),
        manager: None,
    }
}

struct ProxyManager {
    // Plugin -> Set of Commands
    plugin_commands: Arc<RwLock<BTreeMap<String, BTreeSet<String>>>>,
    command_proxy_path: Arc<RwLock<BTreeMap<String, ProxyPath>>>,
}

impl ProxyManager {
    fn configure<I>(&mut self, name: &str, it: I)
    where
        I: Iterator<Item = ServiceConfig>,
    {
        let mut w = self.plugin_commands.write().unwrap();

        let map: BTreeSet<_> = it.map(|cfg| cfg.cmd.clone()).collect();

        w.insert(name.to_string(), map);
        debug!("w={:?}", *w);
    }

    fn unconfigure(&mut self, name: &str) {
        let mut w = self.plugin_commands.write().unwrap();

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
                            let service = meta.service("gu-service");
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
                let config: Vec<ServiceConfig> = plugin_meta.service("gu-service");
                self.configure(plugin_meta.name(), config.into_iter());
            }
            PluginEvent::Drop(name) => self.unconfigure(&name),
        }
    }
}

struct LocalServiceModule {
    // <plugin> -> <path> -> <url>
    plugin_commands: Arc<RwLock<BTreeMap<String, BTreeSet<String>>>>,
    command_proxy_path: Arc<RwLock<BTreeMap<String, ProxyPath>>>,
    manager: Option<Addr<ProxyManager>>,
}

#[derive(Deserialize)]
enum Command {
    #[serde(rename_all = "camelCase")]
    RegisterCommand { cmd_name: String, url: String },
}

fn split_path(path: &str) -> Option<(&str, &str)> {
    if let Some(spos) = path.find("/") {
        let (l, r) = path.split_at(spos);
        Some((l, &r[1..]))
    } else {
        None
    }
}

fn split_path2(path: &str) -> Option<(&str, &str, &str)> {
    split_path(path).and_then(|(p1, t)| {
        let (p2, t) = split_path(t)?;

        Some((p1, p2, t))
    })
}

impl Module for LocalServiceModule {
    fn args_consume(&mut self, _matches: &ArgMatches) -> bool {
        let plugin_commands = self.plugin_commands.clone();
        let command_proxy_path = self.command_proxy_path.clone();
        self.manager = Some(
            ProxyManager {
                plugin_commands,
                command_proxy_path,
            }
            .start(),
        );
        false
    }

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        let plugin_commands = self.plugin_commands.clone();
        let command_proxy_path = self.command_proxy_path.clone();

        let command_proxy_path_r = command_proxy_path.clone();
        let plugin_commands_r = plugin_commands.clone();
        app.route(
            "/service/local",
            actix_web::http::Method::PATCH,
            move |r: Json<Command>| match r.into_inner() {
                Command::RegisterCommand { cmd_name, url } => {
                    let r = command_proxy_path_r
                        .write()
                        .unwrap()
                        .insert(cmd_name, ProxyPath::Remote { url });
                    Json(r.is_none())
                }
            },
        )
        .route(
            "/service/local",
            actix_web::http::Method::GET,
            move |r: HttpRequest<_>| Json(plugin_commands_r.read().unwrap().clone()),
        )
        .handler("/service/local/", move |r: &HttpRequest<_>| {
            let tail = (&r.path()[15..]).to_string();
            if let Some((plugin_name, command, request_url)) = split_path2(&tail) {
                match plugin_commands.read().unwrap().get(plugin_name) {
                    Some(commands) => {
                        if !commands.contains(command) {
                            return actix_web::Either::A(HttpResponse::NotFound().body(format!(
                                "command {} not found in plugin {}",
                                command, plugin_name
                            )));
                        }
                    }
                    None => {
                        return actix_web::Either::A(
                            HttpResponse::NotFound()
                                .body(format!("plugin not found: {}", plugin_name)),
                        );
                    }
                };
                if let Some(proxy_path) = command_proxy_path.read().unwrap().get(command) {
                    return actix_web::Either::B(proxy_path.create_request(request_url, r));
                } else {
                    return actix_web::Either::A(
                        HttpResponse::NotFound().body(format!("command: {}", command)),
                    );
                }
            }
            actix_web::Either::A(HttpResponse::NotFound().body("err"))
        })
    }
}

struct ProxyHandler(Arc<RwLock<BTreeMap<String, BTreeMap<String, String>>>>);

#[derive(Debug)]
struct ServiceRunner {}

impl ServiceRunner {
    fn new(config: ServiceConfig) -> Self {
        ServiceRunner {}
    }
}

enum ProxyPath {
    Remote { url: String },
    Local { cmd: String },
}

impl ProxyPath {
    fn create_request<S>(&self, path: &str, req: &HttpRequest<S>) -> impl Responder {
        use actix_web::{client, http, Body};

        let mut b = client::ClientRequest::build();

        match self {
            ProxyPath::Remote { url } => {
                b.uri(&format!("{}{}", url, path));
            }
            ProxyPath::Local { cmd } => {
                unimplemented!();
            }
        }

        b.method(req.method().clone());

        for (k, v) in req.headers() {
            b.header(k, v.clone());
        }

        b.streaming(req.payload())
            .unwrap()
            .send()
            .map_err(|e| actix_web::error::ErrorInternalServerError(e))
            .and_then(|resp| {
                Ok(HttpResponse::build(resp.status())
                    .body(Body::Streaming(Box::new(resp.payload().from_err()))))
            })
            .responder()
    }
}
