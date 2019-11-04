use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    str::FromStr,
    sync::Arc,
};

use log::error;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use actix::prelude::*;
use actix_web::{HttpResponse, Path, Query, Scope};
use futures::prelude::*;

use gu_actix::prelude::*;
use gu_base::{App, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_lan::HubDesc;
use gu_net::NodeId;
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId, SetConfig};

use crate::connect::{
    change_single_connection, edit_config_connect_mode, edit_config_hosts, ConnectionChange,
};
use crate::server::ConnectMode;
use futures::future;

#[derive(Serialize_repr, Deserialize_repr, Clone, PartialEq, Eq, Hash, Copy)]
#[repr(u8)]
enum AccessLevel {
    NoAccess = 0,
    Sandbox = 1,
    FullAccess = 2,
}

impl Default for AccessLevel {
    fn default() -> AccessLevel {
        AccessLevel::NoAccess
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
enum Permission {
    ManagedBy(NodeId, AccessLevel),
    Read(NodeId),
    // Can create session in given environment
    CreateSession(NodeId, String),
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct PermissionConfig {
    allow_any: AccessLevel,
    permissions: HashSet<Permission>,
    #[serde(default)]
    saved_hub_desc: HashMap<NodeId, HubDesc>,
}

impl PermissionConfig {
    fn add(&self, permission: Permission) -> Self {
        let mut permissions = self.permissions.clone();
        permissions.insert(permission);

        Self {
            permissions,
            allow_any: self.allow_any,
            saved_hub_desc: self.saved_hub_desc.clone(),
        }
    }

    fn highest_permission(&self, node_id: &NodeId) -> AccessLevel {
        self.permissions
            .iter()
            .fold(AccessLevel::NoAccess, |h, x| match x {
                Permission::ManagedBy(n, level) => {
                    if *level as i32 > h as i32 && *n == *node_id {
                        *level
                    } else {
                        h
                    }
                }
                _ => h,
            })
    }

    fn is_managed_by(&self, node_id: &NodeId) -> bool {
        self.highest_permission(node_id) != AccessLevel::NoAccess
    }

    fn remove_managed_permissions_except_for(&mut self, except_nodes: &HashSet<NodeId>) {
        self.permissions.retain(|perm| match perm {
            Permission::ManagedBy(node_id, _) => except_nodes.contains(node_id),
            _ => true,
        });
    }
}

impl HasSectionId for PermissionConfig {
    const SECTION_ID: &'static str = "permissions";
}

#[derive(Clone)]
enum NodeOrAuto {
    Node(NodeId),
    Auto,
}

enum PermissionModule {
    None,
    Join(NodeId, Option<SocketAddr>),
    Configure,
    AllowNode(NodeOrAuto, Option<SocketAddr>, Option<String>), /* all params required when Node(NodeId) is used */
    DenyNode(NodeOrAuto, Option<SocketAddr>, Option<String>), /* all params required when Node(NodeId) is used */
    NodeAllowedStatus(NodeOrAuto),
    ListSavedHubs,
}

fn config_future(
) -> impl Future<Item = std::sync::Arc<PermissionConfig>, Error = gu_persist::error::Error> {
    ConfigManager::from_registry()
        .send(GetConfig::new())
        .flatten_fut()
}

fn list_saved_hubs_future() -> impl Future<Item = String, Error = ()> {
    config_future()
        .and_then(move |c: Arc<PermissionConfig>| {
            let mut output: Vec<HubDesc> =
                c.saved_hub_desc.values().cloned().collect::<Vec<HubDesc>>();
            output.sort_unstable_by(|a, b| a.host_name.cmp(&b.host_name));
            Ok(serde_json::to_string_pretty(&output).unwrap())
        })
        .map_err(|e| error!("{}", e))
}

fn get_allowed_nodes_and_auto_future() -> impl Future<Item = String, Error = ()> {
    #[derive(Serialize)]
    struct Reply {
        auto: AccessLevel,
        allow: Vec<NodeId>,
    }
    config_future()
        .and_then(move |c: Arc<PermissionConfig>| {
            let nodes: Vec<NodeId> = c
                .permissions
                .iter()
                .filter_map(|e| match e {
                    Permission::ManagedBy(n, _) => Some(*n),
                    _ => None,
                })
                .collect();
            Ok(serde_json::to_string_pretty(&Reply {
                auto: c.allow_any,
                allow: nodes,
            })
            .unwrap())
        })
        .map_err(|e| error!("{}", e))
}

fn get_node_status_future(node_or_auto: NodeOrAuto) -> impl Future<Item = AccessLevel, Error = ()> {
    config_future()
        .and_then(move |c: Arc<PermissionConfig>| {
            Ok(match node_or_auto {
                NodeOrAuto::Node(node_id) => c.highest_permission(&node_id),
                NodeOrAuto::Auto => c.allow_any,
            })
        })
        .map_err(|e| error!("{}", e))
}

fn set_node_status_future(
    access_level: AccessLevel,
    node_or_auto: NodeOrAuto,
    ip: Option<SocketAddr>,
    host_name: Option<String>,
) -> impl Future<Item = (), Error = ()> {
    let node_or_auto_copy = node_or_auto.clone();
    let config_manager = ConfigManager::from_registry();
    config_manager
        .send(GetConfig::new())
        .flatten_fut()
        .and_then(move |c: Arc<PermissionConfig>| {
            let mut new_config = (*c).clone();
            match node_or_auto {
                NodeOrAuto::Auto => {
                    new_config.allow_any = access_level;
                }
                NodeOrAuto::Node(n) => {
                    new_config.permissions.retain(|perm| match perm {
                        Permission::ManagedBy(n2, _) => *n2 != n,
                        _ => true,
                    });
                    if access_level != AccessLevel::NoAccess {
                        if host_name.is_some() && ip.is_some() {
                            new_config.saved_hub_desc.insert(
                                n.clone(),
                                HubDesc {
                                    address: ip.unwrap(),
                                    host_name: host_name.unwrap(),
                                    node_id: n.clone(),
                                },
                            );
                        }
                        let perm = Permission::ManagedBy(n.clone(), access_level);
                        new_config.permissions.insert(perm);
                    }
                }
            };
            config_manager
                .send(SetConfig::new(new_config))
                .flatten_fut()
        })
        .map_err(|_| eprintln!("Cannot save permissions."))
        .and_then(move |_| match node_or_auto_copy {
            NodeOrAuto::Auto => futures::future::Either::A(
                edit_config_connect_mode(if access_level != AccessLevel::NoAccess {
                    ConnectMode::Auto
                } else {
                    ConnectMode::Manual
                })
                .map_err(|_| ()),
            ),
            NodeOrAuto::Node(_node_id) => futures::future::Either::B(
                change_single_connection(
                    ip.unwrap(),
                    if access_level != AccessLevel::NoAccess {
                        ConnectionChange::Connect
                    } else {
                        ConnectionChange::Disconnect
                    },
                )
                .map_err(|_| ()),
            ),
        })
        .map(|_| ())
}

impl Module for PermissionModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app
            /*.subcommand(
                SubCommand::with_name("join")
                    .setting(AppSettings::ArgRequiredElseHelp)
                    .args(&[
                        Arg::with_name("group-id")
                            .help("group addr through which the node will be managed")
                            .required(true),
                        Arg::with_name("hub-addr")
                            .required(false)
                            .help("<ip>:<port> of hub"),
                    ])
                    .about("allows provider to be managed by group"),
            )
            */
            .subcommand(
                SubCommand::with_name("configure")
                    .about("Displays a UI that can be used to configure a local server")
                    .arg(
                        Arg::with_name("get-node")
                            .long("get-node")
                            .short("g")
                            .value_names(&["node_id"])
                            .help("Gets node permission status, i.e. whether it has sufficient permissions to connect to this provider. \
                                    If node_id is \"auto\", then return whether automatic mode is on.")
                    )
                    .arg(
                        Arg::with_name("allow-node")
                            .long("allow-node")
                            .short("a")
                            .value_names(&["node_id", "ip:port", "host_name"])
                            .help("Allows selected hub to connect to this provider and save the new configuration to config files.")
                    )
                    .arg(
                        Arg::with_name("deny-node")
                            .long("deny-node")
                            .short("d")
                            .value_names(&["node_id", "ip:port", "host_name"])
                            .help("Deny connections from selected hub to this provider and save the new configuration to config files.")
                    )
                    .arg(
                        Arg::with_name("allow-all")
                            .short("A")
                            .long("allow-all")
                            .help("Allows any hub to connect to this provider (turn automatic mode on) \
                                    and save the new configuration to config files.")
                    )
                    .arg(
                        Arg::with_name("deny-unknown")
                            .short("D")
                            .long("deny-unknown")
                            .help("Denies connections from unknown hubs to this provider (turn manual mode on) \
                                    and save the new configuration to config files.")
                    )
                    .arg(
                        Arg::with_name("list-saved-hubs")
                            .short("l")
                            .long("list-saved-hubs")
                            .help("Lists all hubs that were ever used by this provider.")
                    )
            )
    }

    fn args_consume(&mut self, matches: &ArgMatches) -> bool {
        if let Some(cmd) = matches.subcommand_matches("join") {
            let group_id = cmd.value_of("group-id").map(|s| NodeId::from_str(s));
            let hub_addr = cmd.value_of("hub-addr").map(|s| SocketAddr::from_str(s));
            match (group_id, hub_addr) {
                (None, _) => {
                    eprintln!("missing group id for join");
                    return true;
                }
                (Some(Err(_)), _) => {
                    eprintln!("invalid group id format");
                    return true;
                }
                (_, Some(Err(_))) => {
                    eprintln!("invalid hub addr format");
                    return true;
                }
                (Some(Ok(group_id)), Some(Ok(hub_addr))) => {
                    *self = PermissionModule::Join(group_id, Some(hub_addr));
                    return true;
                }
                (Some(Ok(group_id)), None) => {
                    *self = PermissionModule::Join(group_id, None);
                    return true;
                }
            }
        } else if let Some(cmd) = matches.subcommand_matches("configure") {
            if vec![
                "get-node",
                "allow-node",
                "deny-node",
                "allow-all",
                "deny-unknown",
                "list-saved-hubs",
            ]
            .iter()
            .all(|x| !cmd.is_present(x))
            {
                *self = PermissionModule::Configure;
                return true;
            } else {
                let param_to_node_and_ip = |param_name| {
                    let values = cmd.values_of(param_name);
                    if values.is_none() {
                        return Err(());
                    }
                    let params = values.unwrap().into_iter().collect::<Vec<_>>();
                    if param_name == "get-node" && params.len() == 1 && params[0] == "auto" {
                        return Ok((NodeOrAuto::Auto, None, None));
                    }
                    match NodeId::from_str(params[0]) {
                        Ok(node) => {
                            if params.len() == 1 && param_name == "get-node" {
                                Ok((NodeOrAuto::Node(node), None, None))
                            } else if params.len() < 2 || params.len() > 3 {
                                Err(())
                            } else {
                                let sock_addr: SocketAddr =
                                    params[1].parse().expect("Expected ip:port.");
                                Ok((
                                    NodeOrAuto::Node(node),
                                    Some(sock_addr),
                                    Some(
                                        (if params.len() == 2 { "" } else { params[2] })
                                            .to_string(),
                                    ),
                                ))
                            }
                        }
                        _ => Err(()),
                    }
                };
                if cmd.is_present("allow-all") {
                    *self = PermissionModule::AllowNode(NodeOrAuto::Auto, None, None);
                    return true;
                }
                if cmd.is_present("deny-unknown") {
                    *self = PermissionModule::DenyNode(NodeOrAuto::Auto, None, None);
                    return true;
                }
                if cmd.is_present("list-saved-hubs") {
                    *self = PermissionModule::ListSavedHubs;
                    return true;
                }
                if let Ok((node, _, _)) = param_to_node_and_ip("get-node") {
                    *self = PermissionModule::NodeAllowedStatus(node);
                    return true;
                }
                if let Ok((node, ip, host_name)) = param_to_node_and_ip("allow-node") {
                    *self = PermissionModule::AllowNode(node, ip, host_name);
                    return true;
                }
                if let Ok((node, ip, host_name)) = param_to_node_and_ip("deny-node") {
                    *self = PermissionModule::DenyNode(node, ip, host_name);
                    return true;
                }
                eprintln!("Invalid parameters.");
                return false;
            }
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self {
            PermissionModule::None => (),
            PermissionModule::Join(ref_group_id, _hub_address) => {
                let group_id = ref_group_id.clone();

                System::run(move || {
                    let config_manager = ConfigManager::from_registry();

                    Arbiter::spawn(
                        config_manager
                            .send(GetConfig::new())
                            .flatten_fut()
                            .and_then(move |c: Arc<PermissionConfig>| {
                                let new_config =
                                    c.add(Permission::ManagedBy(group_id, AccessLevel::FullAccess));
                                config_manager
                                    .send(SetConfig::new(new_config))
                                    .flatten_fut()
                            })
                            .map_err(|_| System::current().stop())
                            .and_then(|_r| Ok(System::current().stop())),
                    );
                });
            }
            PermissionModule::Configure => run_configure(),
            PermissionModule::NodeAllowedStatus(node_or_auto) => {
                let node_or_auto_copy = node_or_auto.clone();
                System::run(move || {
                    Arbiter::spawn(
                        get_node_status_future(node_or_auto_copy)
                            .and_then(|status| {
                                Ok(println!(
                                    "{}",
                                    serde_json::to_string_pretty(&status).unwrap()
                                ))
                            })
                            .then(|_| Ok(System::current().stop())),
                    )
                });
            }
            PermissionModule::AllowNode(node_or_auto, ip, host_name)
            | PermissionModule::DenyNode(node_or_auto, ip, host_name) => {
                let access_level = match self {
                    PermissionModule::AllowNode(_, _, _) => AccessLevel::FullAccess,
                    _ => AccessLevel::NoAccess,
                };
                let node_or_auto_copy = node_or_auto.clone();
                let ip_copy = ip.clone();
                let host_name_copy = host_name.clone();
                System::run(move || {
                    Arbiter::spawn(
                        set_node_status_future(
                            access_level,
                            node_or_auto_copy,
                            ip_copy,
                            host_name_copy,
                        )
                        .then(|_| Ok(System::current().stop())),
                    )
                });
            }
            PermissionModule::ListSavedHubs => {
                System::run(move || {
                    Arbiter::spawn(
                        list_saved_hubs_future()
                            .and_then(|hubs| Ok(println!("{}", hubs)))
                            .then(|_| Ok(System::current().stop())),
                    )
                });
            }
        }
    }

    fn decorate_webapp<S: 'static>(&self, app: actix_web::App<S>) -> actix_web::App<S> {
        app.scope("/nodes", config_methods)
    }
}

fn extract_node_or_auto(path: Path<String>) -> Result<NodeOrAuto, ()> {
    match path.as_str() {
        "auto" => Ok(NodeOrAuto::Auto),
        id => match NodeId::from_str(id) {
            Ok(node_id) => Ok(NodeOrAuto::Node(node_id)),
            Err(_) => Err(()),
        },
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IpHostNameAccessLevel {
    pub address: Option<SocketAddr>,
    pub host_name: Option<String>,
    pub access_level: Option<AccessLevel>,
}

fn config_methods<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope
        .resource("", |r| {
            r.get().with_async(|q: Query<HashMap<String, String>>| {
                (if q.contains_key("saved") {
                    futures::future::Either::A(list_saved_hubs_future())
                } else {
                    futures::future::Either::B(get_allowed_nodes_and_auto_future())
                })
                .map_err(|_| actix_web::error::ErrorInternalServerError("Hub listing"))
                .and_then(|r| future::ok(HttpResponse::Ok().body(format!("{}", r))))
            });
        })
        .resource("{node_id}", |r| {
            r.get().with_async(|path: Path<String>| {
                extract_node_or_auto(path)
                    .into_future()
                    .map_err(|_| actix_web::error::ErrorInternalServerError("bad format"))
                    .and_then(|node_or_auto| {
                        get_node_status_future(node_or_auto)
                            .map_err(|_| actix_web::error::ErrorNotFound("node not found"))
                            .and_then(|selected| {
                                future::ok::<_, actix_web::Error>(
                                    HttpResponse::Ok()
                                        .body(serde_json::to_string_pretty(&selected).unwrap()),
                                )
                            })
                    })
            });

            let put_delete_handler = |is_put: bool| {
                move |(path, hub): (Path<String>, actix_web::Json<IpHostNameAccessLevel>)| {
                    extract_node_or_auto(path)
                        .into_future()
                        .map_err(|_| actix_web::error::ErrorInternalServerError("bad format"))
                        .and_then(move |node_or_auto| {
                            set_node_status_future(
                                if is_put {
                                    hub.access_level.unwrap_or(AccessLevel::FullAccess)
                                } else {
                                    AccessLevel::NoAccess
                                },
                                node_or_auto,
                                hub.address,
                                hub.host_name.clone(),
                            )
                            .map_err(|_| {
                                actix_web::error::ErrorInternalServerError("cannot delete node")
                            })
                            .and_then(|_| Ok(HttpResponse::Ok().finish()))
                        })
                }
            };

            r.put().with_async(put_delete_handler(true));
            r.delete().with_async(put_delete_handler(false));
        })
}

fn check_box(v: bool) -> &'static str {
    if v {
        "[X]"
    } else {
        "[ ]"
    }
}

fn run_configure() {
    use std::io;

    fn toggle_managed_by(config: &mut PermissionConfig, hub: &gu_lan::HubDesc) {
        let node_id = hub.node_id;
        let managed_by = Permission::ManagedBy(node_id, AccessLevel::FullAccess);
        let mut removed = false;
        config.permissions.retain(|perm| match perm {
            Permission::ManagedBy(n, _) => {
                if *n == node_id {
                    removed = true;
                    false
                } else {
                    true
                }
            }
            _ => true,
        });
        if !removed {
            config.permissions.insert(managed_by);
            config.saved_hub_desc.insert(node_id, hub.clone());
        }
    }

    System::run(|| {
        let get_config = config_future()
            .map_err(|e| error!("Cannot get config. Use --user to access configuration files for local user ({})", e))
            .and_then(|c: Arc<PermissionConfig>| Ok(c));

        Arbiter::spawn(
            gu_lan::list_hubs()
                .join(get_config)
                .and_then(|(hubs, config_ref)| {
                    let mut config = (*config_ref).clone();
                    let mut valid_hubs_set = hubs
                        .into_iter()
                        .map(|h| (h.node_id, h))
                        .collect::<HashMap<NodeId, gu_lan::HubDesc>>();
                    for hub_desc in config_ref.saved_hub_desc.values() {
                        if !valid_hubs_set.contains_key(&hub_desc.node_id) {
                            valid_hubs_set.insert(hub_desc.node_id, hub_desc.clone());
                        }
                    }
                    let valid_hubs: Vec<gu_lan::HubDesc> =
                        valid_hubs_set.values().cloned().collect();
                    let except_node_ids: HashSet<NodeId> =
                        valid_hubs.iter().map(|desc| desc.node_id).collect();
                    config.remove_managed_permissions_except_for(&except_node_ids);

                    loop {
                        let mut selected_hubs = HashSet::<SocketAddr>::new();
                        println!("Select valid hub:");

                        valid_hubs.iter().enumerate().for_each(|(idx, hub)| {
                            println!(
                                "{} {}) name={}, addr={:?}, node_id={}",
                                check_box(config.is_managed_by(&hub.node_id)),
                                idx + 1,
                                hub.host_name,
                                hub.address,
                                hub.node_id.to_string()
                            );

                            if config.is_managed_by(&hub.node_id) {
                                selected_hubs.insert(hub.address);
                                config.permissions.insert(Permission::ManagedBy(
                                    hub.node_id,
                                    AccessLevel::FullAccess,
                                ));
                            }
                        });
                        println!(
                            "{} *) Access is granted to everyone",
                            check_box(config.allow_any != AccessLevel::NoAccess)
                        );
                        println!("    s) Save configuration");
                        println!();

                        let mut input_buf = String::new();
                        let connect_mode = if config.allow_any != AccessLevel::NoAccess {
                            ConnectMode::Auto
                        } else {
                            ConnectMode::Manual
                        };

                        eprint!(" => ");
                        io::stdin().read_line(&mut input_buf).unwrap();
                        let input = input_buf.trim();
                        if input == "*" {
                            config.allow_any = if config.allow_any == AccessLevel::NoAccess {
                                AccessLevel::FullAccess
                            } else {
                                AccessLevel::NoAccess
                            }
                        } else if input == "s" {
                            return ConfigManager::from_registry()
                                .send(SetConfig::new(config))
                                .map_err(|_| ())
                                .and_then(move |_| {
                                    edit_config_connect_mode(connect_mode).map_err(|_| ())
                                })
                                .and_then(|_| {
                                    edit_config_hosts(
                                        selected_hubs,
                                        ConnectionChange::Connect,
                                        true,
                                    )
                                    .map_err(|_| ())
                                })
                                .then(|_| Ok(()));
                        } else {
                            let idx: usize = input.parse().unwrap_or_default();
                            if idx > 0 && idx <= valid_hubs.len() {
                                toggle_managed_by(&mut config, &valid_hubs[idx - 1])
                            }
                        }
                    }
                })
                .map_err(|_| System::current().stop())
                .and_then(|_r| Ok(System::current().stop())),
        );
    });
}

pub fn module() -> impl Module {
    PermissionModule::None
}

#[cfg(test)]
mod test {
    use super::IpHostNameAccessLevel;

    #[test]
    fn test_deserialize() {
        let input = r#"{
            "hostName": "localhost",
            "address": "127.0.0.1:80",
            "accessLevel": 1
            }"#;

        let _: IpHostNameAccessLevel = serde_json::from_str(input).unwrap();
    }
}
