use super::server::ServerConfig;
use actix::prelude::*;
use actix_web::{http, App, HttpRequest, Responder, Scope};
use futures::{future, Future};
use gu_actix::flatten::FlattenFuture;
use gu_base::{self, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_lan::{
    actor::{Continuous, MdnsActor, SubscribeInstance},
    NewInstance, ServiceDescription, Subscription,
};
use gu_net::{
    rpc::{
        self,
        ws::{ConnectionSupervisor, StopSupervisor},
    },
    NodeId,
};
use gu_persist::config::{ConfigManager, ConfigSection, GetConfig, SetConfig};
use serde::{de::DeserializeOwned, Serialize};
use server::ConnectMode;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

pub struct ConnectModule {
    state: State,
}

#[derive(PartialEq)]
enum State {
    Connect(SocketAddr),
    Disconnect(SocketAddr),
    Add(SocketAddr),
    Remove(SocketAddr),
    Mode(ConnectMode),
    ListConfig,
    ListConnected,
    None,
}

impl Module for ConnectModule {
    fn args_declare<'a, 'b>(&self, app: gu_base::App<'a, 'b>) -> gu_base::App<'a, 'b> {
        let host = Arg::with_name("host")
            .index(1)
            .short("h")
            .required(true)
            .long("hub address")
            .takes_value(true)
            .value_name("IP:PORT")
            .help("IP and PORT of a Hub");

        let connect = SubCommand::with_name("connect")
            .about("Connect to a host without adding it to the config")
            .arg(host.clone());
        let disconnect = SubCommand::with_name("disconnect")
            .about("Disconnect from a hub")
            .arg(host.clone());
        let add = SubCommand::with_name("add")
            .about("Add host to the config hub list and connect")
            .arg(host.clone());
        let remove = SubCommand::with_name("remove")
            .about("Remove a hub from config and disconnect form it")
            .arg(host.clone());
        let config = SubCommand::with_name("config")
            .about("List hubs contained in config")
            .arg(host);
        let servers =
            SubCommand::with_name("list").about("List hubs the provider is currently connected to");

        let auto_mode = SubCommand::with_name("auto")
            .about("Connect to the config hubs and additionally automatically connect to all found local hubs");
        let config_mode = SubCommand::with_name("config").about("Connect just to config hubs");
        let mode = SubCommand::with_name("mode")
            .about("Change the connection mode")
            .subcommands(vec![auto_mode, config_mode]);

        app.subcommand(SubCommand::with_name("hubs").about("Manipulate hubs connections"))
            .subcommands(vec![
                connect, disconnect, add, remove, mode, config, servers,
            ])
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        let get_host: fn(&ArgMatches) -> SocketAddr =
            |m| m.value_of("host").unwrap().parse().unwrap();

        self.state = match matches.subcommand() {
            ("connect", Some(m)) => State::Connect(get_host(m)),
            ("disconnect", Some(m)) => State::Disconnect(get_host(m)),
            ("add", Some(m)) => State::Add(get_host(m)),
            ("remove", Some(m)) => State::Remove(get_host(m)),
            ("mode", Some(m)) => match m.subcommand_name().unwrap() {
                "auto" => State::Mode(ConnectMode::Auto),
                "config" => State::Mode(ConnectMode::Config),
                _ => State::None,
            },
            ("list", _) => State::ListConnected,
            ("config", _) => State::ListConfig,
            _ => State::None,
        };

        self.state != State::None
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {}

    fn decorate_webapp<S: 'static>(&self, app: App<S>) -> App<S> {
        app.scope("/connect/all", scope)
    }
}

fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    let mode_lambda = |m: ConnectMode| move |x| mode_scope(x, &m);
    let list_lambda = |m: ListingType| move |x| list_scope(x, &m);
    let connect_lambda = |m: ConnectionChange| move |x| connect_scope(x, &m);

    let edit_connection_scope = |method: http::Method| {
        |scope: Scope<S>| {
            scope
                .route(
                    "/add",
                    method.clone(),
                    connect_lambda(ConnectionChange::Add),
                )
                .route(
                    "/remove",
                    method.clone(),
                    connect_lambda(ConnectionChange::Remove),
                )
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
        .route(
            "/config",
            http::Method::GET,
            list_lambda(ListingType::Config),
        )
        .route(
            "/connected",
            http::Method::GET,
            list_lambda(ListingType::Connected),
        )
        .route(
            "/mode/auto",
            http::Method::PUT,
            mode_lambda(ConnectMode::Auto),
        )
        .route(
            "/mode/config",
            http::Method::PUT,
            mode_lambda(ConnectMode::Config),
        )
        .nested("/batch", edit_connection_scope(http::Method::POST))
        .nested(
            "/{hostAddr:.+:.+}",
            edit_connection_scope(http::Method::PUT),
        )
}

#[derive(Message)]
pub(crate) enum ListingType {
    Config,
    Connected,
}

fn list_scope<S>(_r: HttpRequest<S>, _m: &ListingType) -> impl Responder {
    ""
}

fn mode_scope<S>(_r: HttpRequest<S>, _m: &ConnectMode) -> impl Responder {
    ""
}

pub(crate) enum ConnectionChange {
    Add,
    Remove,
    Connect,
    Disconnect,
}

#[derive(Message)]
#[rtype(result = "Result<Option<()>, String>")]
pub(crate) struct ConnectionChangeMessage {
    pub change: ConnectionChange,
    pub hubs: Vec<SocketAddr>,
}

fn connect_scope<S>(_r: HttpRequest<S>, _m: &ConnectionChange) -> impl Responder {
    ""
}

fn dummy_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    ""
}

pub(crate) fn edit_config_connect_mode(
    mode: ConnectMode,
) -> impl Future<Item = Option<()>, Error = String> {
    fn editor(c: &ServerConfig, data: ConnectMode) -> Option<ServerConfig> {
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
    fn editor(c: &ServerConfig, data: (Vec<SocketAddr>, ConnectionChange)) -> Option<ServerConfig> {
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
        ConnectionChange::Add => {
            list.into_iter().for_each(|sock| {
                old.insert(sock);
            });
        }
        ConnectionChange::Remove => {
            list.into_iter().for_each(|sock| {
                old.remove(&sock);
            });
        }
        ConnectionChange::Connect => return None,
        ConnectionChange::Disconnect => return None,
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
        I: IntoIterator<Item=SocketAddr>,
    {
        ConnectManager {
            node_id: id,
            connections: HashMap::new(),
            subscription: None,
        }
    }

    fn connect_to(&mut self, addr: SocketAddr) {
        if self.connections.contains_key(&addr) {
            return;
        }

        let supervisor = rpc::ws::start_connection(self.node_id, addr);
        self.connections.insert(addr, supervisor);
    }

    fn disconnect(&mut self, addr: SocketAddr) -> impl Future<Item = (), Error = String> {
        if let Some(supervisor) = self.connections.remove(&addr) {
            future::Either::A(
                supervisor
                    .send(StopSupervisor)
                    .and_then(|_| Ok(()))
                    .map_err(|e| format!("{}", e)),
            )
        } else {
            future::Either::B(future::ok(()))
        }
    }
}

impl Actor for ConnectManager {
    type Context = Context<Self>;
}

impl Handler<NewInstance> for ConnectManager {
    type Result = ();

    fn handle(&mut self, msg: NewInstance, _ctx: &mut Context<Self>) -> () {
        println!("{:?}", msg.data);
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
#[rtype(result = "Result<(), String>")]
pub struct AutoMdns(pub bool);

impl Handler<AutoMdns> for ConnectManager {
    type Result = ActorResponse<Self, (), String>;

    fn handle(&mut self, msg: AutoMdns, ctx: &mut Context<Self>) -> Self::Result {
        if msg.0 && self.subscription.is_none() {
            ActorResponse::async(MdnsActor::<Continuous>::from_registry()
                .send(SubscribeInstance {
                    service: ServiceDescription::new("gu-hub", "_unlimited._tcp"),
                    rec: ctx.address().recipient(),
                })
                .flatten_fut()
                .map_err(|e| format!("{}", e))
                .into_actor(self)
                .and_then(|res, act: &mut Self, ctx| {
                    act.subscription = Some(res);
                    println!("??");

                    future::ok(()).into_actor(act)
                }))
        } else if !msg.0 {
            self.subscription = None;
            ActorResponse::reply(Ok(()))
        } else {
            ActorResponse::reply(Ok(()))
        }
    }
}
