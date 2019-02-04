#![allow(proc_macro_derive_resolution_fallback)]

use super::server::ProviderConfig;
use actix::prelude::*;
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    http, App, AsyncResponder, HttpMessage, HttpRequest, HttpResponse, Responder, Scope,
};
use clap::AppSettings;
use futures::{future, stream::Stream, Future};
use gu_actix::flatten::FlattenFuture;
use gu_base::{self, cli, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_lan::{
    actor::{Continuous, MdnsActor, SubscribeInstance},
    NewInstance, ServiceDescription, Subscription,
};
use gu_net::{
    rpc::{
        self,
        ws::{ConnectionSupervisor, IsConnected, StopSupervisor},
    },
    NodeId,
};
use gu_persist::config::{ConfigManager, ConfigSection, GetConfig, SetConfig};
use serde::{de::DeserializeOwned, Serialize};
use serde_json;
use server::{ConnectMode, ProviderClient, ProviderServer};
use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    net::SocketAddr,
};

pub fn module() -> ConnectModule {
    ConnectModule { state: State::None }
}

pub struct ConnectModule {
    state: State,
}

#[derive(PartialEq, Clone, Debug)]
enum State {
    Connect(Vec<SocketAddr>, bool),
    Disconnect(Vec<SocketAddr>, bool),
    Mode(ConnectMode, bool),
    List(ListingType),
    None,
}

impl Into<&'static str> for State {
    fn into(self) -> &'static str {
        match self {
            State::Connect(_, false) => "/connections/connect",
            State::Connect(_, true) => "/connections/connect?save=1",
            State::Disconnect(_, false) => "/connections/disconnect",
            State::Disconnect(_, true) => "/connections/disconnect?save=1",
            State::Mode(x, false) => match x {
                ConnectMode::Auto => "/connections/mode/auto",
                ConnectMode::Manual => "/connections/mode/manual",
            },
            State::Mode(x, true) => match x {
                ConnectMode::Auto => "/connections/mode/auto?save=1",
                ConnectMode::Manual => "/connections/mode/manual?save=1",
            },
            State::List(x) => match x {
                ListingType::Pending => "/connections/list/pending",
                ListingType::Connected => "/connections/list/connected",
                ListingType::All => "/connections/list/all",
            },
            State::None => unreachable!(),
        }
    }
}

impl Module for ConnectModule {
    fn args_declare<'a, 'b>(&self, app: gu_base::App<'a, 'b>) -> gu_base::App<'a, 'b> {
        let host = Arg::with_name("host")
            .index(1)
            .short("h")
            .long("hub address")
            .required(true)
            .takes_value(true)
            .multiple(true)
            .value_name("IP:PORT")
            .help("IP and PORT of a Hub");

        let save = Arg::with_name("save")
            .short("S")
            .required(false)
            .long("save")
            .takes_value(false)
            .help("save change in config file");

        let connect = SubCommand::with_name("connect")
            .about("Connect to a hub without adding it to the config file (use -S to add it to the config file)")
            .arg(host.clone())
            .arg(save.clone());
        let disconnect = SubCommand::with_name("disconnect")
            .about("Disconnect from a hub")
            .arg(host.clone())
            .arg(save.clone());

        let list_pending = SubCommand::with_name("pending")
            .about("List hubs to which the provider is trying to get connected");
        let list_connected = SubCommand::with_name("connected")
            .about("List hubs the provider is currently connected to");
        let list = SubCommand::with_name("list")
            .about("Hub listing")
            .subcommands(vec![list_connected, list_pending]);

        let auto_mode = SubCommand::with_name("auto")
            .about("Connect to the config hubs and additionally automatically connect to all found local hubs")
            .arg(save.clone());
        let manual_mode = SubCommand::with_name("manual")
            .about("Connect just to config hubs")
            .arg(save);

        app.subcommand(
            SubCommand::with_name("hubs")
                .about("Manage hubs connections")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommands(vec![connect, disconnect, manual_mode, auto_mode, list]),
        )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        let get_host: fn(&ArgMatches) -> Vec<SocketAddr> =
            |m| Vec::from_iter(m.values_of("host").unwrap().map(|x| x.parse().unwrap()));
        let save = |matches: &ArgMatches| matches.is_present("save");

        let (name, m) = matches.subcommand();
        if name != "hubs" {
            return false;
        }

        self.state = match m.unwrap().subcommand() {
            ("connect", Some(m)) => State::Connect(get_host(m), save(m)),
            ("disconnect", Some(m)) => State::Disconnect(get_host(m), save(m)),
            ("auto", Some(m)) => State::Mode(ConnectMode::Auto, save(m)),
            ("manual", Some(m)) => State::Mode(ConnectMode::Manual, save(m)),
            ("list", Some(m)) => match m.subcommand() {
                ("pending", Some(_)) => State::List(ListingType::Pending),
                ("connected", Some(_)) => State::List(ListingType::Connected),
                ("all", Some(_)) => State::List(ListingType::All),
                ("", _) => State::List(ListingType::All),
                _ => State::None,
            },
            _ => State::None,
        };

        self.state != State::None
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        if self.state == State::None {
            return;
        }
        let state = self.state.clone();
        System::run(move || {
            let endpoint: &'static str = state.clone().into();
            match state {
                State::Connect(a, _) | State::Disconnect(a, _) => Arbiter::spawn(
                    ProviderClient::post_json(
                        endpoint.to_string(),
                        Vec::from_iter(a.into_iter().map(|x| format!("{}:{}", x.ip(), x.port()))),
                    )
                    .and_then(|()| Ok(()))
                    .map_err(|e| error!("state {:?}", e))
                    .then(|_r| Ok(System::current().stop())),
                ),
                State::Mode(_, _) => Arbiter::spawn(
                    ProviderClient::empty_put(endpoint.to_string())
                        .and_then(|_: ()| Ok(()))
                        .map_err(|e| error!("mode {:?}", e))
                        .then(|_r| Ok(System::current().stop())),
                ),
                State::List(listing_type) => Arbiter::spawn(
                    ProviderClient::get(endpoint.to_string())
                        .and_then(move |l: Vec<(SocketAddr, ListingType)>| {
                            println!("Listing {:?} hubs", listing_type);
                            cli::format_table(
                                row!["IP", "address", "port", "status"],
                                || "No hubs found",
                                l.iter()
                                    .filter(|e| match listing_type {
                                        ListingType::All => true,
                                        lt => e.1 == lt,
                                    })
                                    .map(|e| {
                                        row![
                                            match e.0 {
                                                SocketAddr::V4(_) => "V4",
                                                _ => "V6",
                                            },
                                            e.0.ip(),
                                            e.0.port(),
                                            format!("{:?}", e.1),
                                        ]
                                    }),
                            );
                            Ok(())
                        })
                        .map_err(|e| error!("list {:?}", e))
                        .then(|_r| Ok(System::current().stop())),
                ),
                _ => unimplemented!(),
            }
        });
    }

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        app.scope("/connections", scope)
    }
}

fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    let mode_lambda = |m: ConnectMode| move |x| mode_scope(x, &m);
    let connect_lambda = |m: ConnectionChange| move |x| connect_scope(x, &m);

    let edit_connection_scope = |method: http::Method| {
        |scope: Scope<S>| {
            scope
                .route(
                    "/connect",
                    method.clone(),
                    connect_lambda(ConnectionChange::Connect),
                )
                .route(
                    "/disconnect",
                    method,
                    connect_lambda(ConnectionChange::Disconnect),
                )
        }
    };

    scope
        .nested("/list/", |scope| {
            scope
                .route("/pending", http::Method::GET, list_scope)
                .route("/connected", http::Method::GET, list_scope)
                .route("/all", http::Method::GET, list_scope)
        })
        .route(
            "/mode/auto",
            http::Method::PUT,
            mode_lambda(ConnectMode::Auto),
        )
        .route(
            "/mode/manual",
            http::Method::PUT,
            mode_lambda(ConnectMode::Manual),
        )
        .nested("/", edit_connection_scope(http::Method::POST))
}

#[derive(Message, Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub(crate) enum ListingType {
    Pending,
    Connected,
    All,
}

fn list_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    ProviderServer::from_registry()
        .send(ListSockets)
        .map_err(|e| {
            ErrorInternalServerError(format!("Mailbox error during message processing {:?}", e))
        })
        .and_then(|result| {
            result
                .and_then(|list| Ok(HttpResponse::Ok().json(list)))
                .map_err(|e| {
                    ErrorInternalServerError(format!("Error during message processing {:?}", e))
                })
        })
        .responder()
}

fn mode_scope<S>(r: HttpRequest<S>, m: &ConnectMode) -> impl Responder {
    ProviderServer::from_registry()
        .send(ConnectModeMessage {
            mode: m.clone(),
            save: parse_save_param(&r),
        })
        .map_err(|e| {
            ErrorInternalServerError(format!("Mailbox error during message processing {:?}", e))
        })
        .and_then(|result| {
            result
                .and_then(|_| {
                    Ok(HttpResponse::Ok()
                        .content_type("application/json")
                        .body("null"))
                })
                .map_err(|e| {
                    ErrorInternalServerError(format!("Error during message processing {:?}", e))
                })
        })
        .responder()
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum ConnectionChange {
    Connect,
    Disconnect,
}

#[derive(Message, Clone)]
#[rtype(result = "Result<Option<()>, String>")]
pub(crate) struct ConnectionChangeMessage {
    pub change: ConnectionChange,
    pub hubs: Vec<SocketAddr>,
    pub save: bool,
}

fn connect_scope<S>(r: HttpRequest<S>, m: &ConnectionChange) -> impl Responder {
    let change = *m;
    let save = parse_save_param(&r);

    let hubs = r
        .payload()
        .map_err(|e| ErrorBadRequest(format!("Couldn't get request body: {:?}", e)))
        .concat2()
        .and_then(|a| {
            serde_json::from_slice::<Vec<SocketAddr>>(a.as_ref())
                .map_err(|e| ErrorBadRequest(format!("Couldn't parse request body: {:?}", e)))
        });

    hubs.and_then(move |hubs| {
        ProviderServer::from_registry()
            .send(ConnectionChangeMessage { change, hubs, save })
            .map_err(|e| {
                ErrorInternalServerError(format!("Mailbox error during message processing {:?}", e))
            })
    })
    .and_then(|result| {
        result
            .and_then(|_| {
                Ok(HttpResponse::Ok()
                    .content_type("application/json")
                    .body("null"))
            })
            .map_err(|e| {
                ErrorInternalServerError(format!("Error during message processing {:?}", e))
            })
    })
    .map_err(|e| {
        error!("{}", e);
        e
    })
    .responder()
}

fn parse_save_param<S>(r: &HttpRequest<S>) -> bool {
    r.query()
        .get("save")
        .map(|s| s.as_str() == "1")
        .unwrap_or_default()
}

#[derive(Message, Clone)]
#[rtype(result = "Result<Option<()>, String>")]
pub(crate) struct ConnectModeMessage {
    pub mode: ConnectMode,
    pub save: bool,
}

pub(crate) fn edit_config_connect_mode(
    mode: ConnectMode,
) -> impl Future<Item = Option<()>, Error = String> {
    fn editor(c: &ProviderConfig, data: ConnectMode) -> Option<ProviderConfig> {
        use std::ops::Deref;

        if c.connect_mode == data {
            None
        } else {
            let mut config = c.deref().clone();
            config.connect_mode = data;
            Some(config)
        }
    }

    edit_config(mode, editor)
}

pub(crate) fn edit_config_hosts(
    list: Vec<SocketAddr>,
    change: ConnectionChange,
) -> impl Future<Item = Option<()>, Error = String> {
    fn editor(
        c: &ProviderConfig,
        data: (Vec<SocketAddr>, ConnectionChange),
    ) -> Option<ProviderConfig> {
        use std::ops::Deref;

        let mut config = c.deref().clone();
        edit_config_list(config.hub_addrs.clone(), data.0, data.1).map(|new| {
            config.hub_addrs = new;
            config
        })
    }

    edit_config((list, change), editor)
}

fn edit_config<C, A, F>(data: A, fun: F) -> impl Future<Item = Option<()>, Error = String>
where
    C: ConfigSection + Send + Sync + Default + DeserializeOwned + Serialize + 'static,
    A: 'static,
    F: Fn(&C, A) -> Option<C> + 'static,
{
    use futures::{future, Future};
    use std::{ops::Deref, sync::Arc};
    let manager = ConfigManager::from_registry();

    manager
        .send(GetConfig::new())
        .flatten_fut()
        .and_then(move |config: Arc<C>| {
            if let Some(new) = fun(config.deref(), data) {
                future::Either::A(
                    manager
                        .send(SetConfig::new(new))
                        .flatten_fut()
                        .and_then(|_| Ok(Some(()))),
                )
            } else {
                future::Either::B(future::ok(None))
            }
        })
        .map_err(|e| e.to_string())
}

fn edit_config_list(
    old: Vec<SocketAddr>,
    list: Vec<SocketAddr>,
    change: ConnectionChange,
) -> Option<Vec<SocketAddr>> {
    use std::iter::FromIterator;

    let mut old: HashSet<_> = HashSet::from_iter(old.into_iter());
    let len = old.len();

    match change {
        ConnectionChange::Connect => {
            list.into_iter().for_each(|sock| {
                old.insert(sock);
            });
        }
        ConnectionChange::Disconnect => {
            list.into_iter().for_each(|sock| {
                old.remove(&sock);
            });
        }
    }

    if len == old.len() {
        None
    } else {
        Some(Vec::from_iter(old.into_iter()))
    }
}

pub struct ConnectManager {
    node_id: NodeId,
    connections: HashMap<SocketAddr, Addr<ConnectionSupervisor>>,
    subscription: Option<Subscription>,
}

impl ConnectManager {
    pub fn init<I>(id: NodeId, hubs: I) -> Self
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        let mut manager = ConnectManager {
            node_id: id,
            connections: HashMap::new(),
            subscription: None,
        };

        hubs.into_iter().for_each(|hub| manager.connect_to(hub));

        manager
    }

    fn list(&self) -> impl Future<Item = Vec<(SocketAddr, ListingType)>, Error = String> {
        let vec = Vec::from_iter(self.connections.clone().into_iter().map(|elt| {
            elt.1.send(IsConnected).and_then(move |is_connected| {
                Ok((
                    elt.0,
                    match is_connected {
                        true => ListingType::Connected,
                        _ => ListingType::Pending,
                    },
                ))
            })
        }));

        future::join_all(vec).map_err(|e| e.to_string())
    }

    fn connect_to(&mut self, addr: SocketAddr) {
        if self.connections.contains_key(&addr) {
            return;
        }

        let supervisor = rpc::ws::start_connection(self.node_id, addr);
        self.connections.insert(addr, supervisor);
    }

    fn disconnect(&mut self, addr: SocketAddr) -> impl Future<Item = Option<()>, Error = String> {
        if let Some(supervisor) = self.connections.remove(&addr) {
            future::Either::A(
                supervisor
                    .send(StopSupervisor)
                    .and_then(|_| Ok(Some(())))
                    .map_err(|e| format!("{}", e)),
            )
        } else {
            future::Either::B(future::ok(None))
        }
    }
}

impl Actor for ConnectManager {
    type Context = Context<Self>;
}

impl Handler<NewInstance> for ConnectManager {
    type Result = ();

    fn handle(&mut self, msg: NewInstance, _ctx: &mut Context<Self>) -> () {
        if let (Some(ip), Some(port)) = (msg.data.addrs_v4.first(), msg.data.ports.first()) {
            use std::net::IpAddr;

            let ip = IpAddr::V4(*ip);
            let sock = SocketAddr::new(ip, *port);
            self.connect_to(sock);
        } else {
            error!("Invalid mDNS instance")
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<Vec<(SocketAddr, ListingType)>, String>")]
pub(crate) struct ListSockets;

impl Handler<ListSockets> for ConnectManager {
    type Result = ActorResponse<Self, Vec<(SocketAddr, ListingType)>, String>;

    fn handle(&mut self, _: ListSockets, _ctx: &mut Context<Self>) -> Self::Result {
        let list = self.list();
        ActorResponse::async(list.into_actor(self))
    }
}

#[derive(Message)]
#[rtype(result = "Option<()>")]
pub struct Connect(pub SocketAddr);

impl Handler<Connect> for ConnectManager {
    type Result = Option<()>;

    fn handle(&mut self, msg: Connect, _ctx: &mut Context<Self>) -> Option<()> {
        if self.connections.contains_key(&msg.0) {
            return None;
        }

        let supervisor = rpc::ws::start_connection(self.node_id, msg.0);
        self.connections.insert(msg.0, supervisor);
        Some(())
    }
}

#[derive(Message)]
#[rtype(result = "Result<Option<()>, String>")]
pub struct Disconnect(pub SocketAddr);

impl Handler<Disconnect> for ConnectManager {
    type Result = ActorResponse<Self, Option<()>, String>;

    fn handle(&mut self, msg: Disconnect, _ctx: &mut Context<Self>) -> Self::Result {
        ActorResponse::async(self.disconnect(msg.0).into_actor(self))
    }
}

#[derive(Message)]
#[rtype(result = "Result<Option<()>, String>")]
pub struct AutoMdns(pub bool);

impl Handler<AutoMdns> for ConnectManager {
    type Result = ActorResponse<Self, Option<()>, String>;

    fn handle(&mut self, msg: AutoMdns, ctx: &mut Context<Self>) -> Self::Result {
        if msg.0 && self.subscription.is_none() {
            ActorResponse::async(
                MdnsActor::<Continuous>::from_registry()
                    .send(SubscribeInstance {
                        service: ServiceDescription::new("gu-hub", "_unlimited._tcp"),
                        rec: ctx.address().recipient(),
                    })
                    .flatten_fut()
                    .map_err(|e| format!("{}", e))
                    .into_actor(self)
                    .and_then(|res, act: &mut Self, _ctx| {
                        act.subscription = Some(res);

                        future::ok(Some(())).into_actor(act)
                    }),
            )
        } else if !msg.0 {
            self.subscription = None;
            ActorResponse::reply(Ok(Some(())))
        } else {
            ActorResponse::reply(Ok(None))
        }
    }
}
