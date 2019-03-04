use gu_base::{App, AppSettings, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId, SetConfig};

use crate::connect::{edit_config_connect_mode, edit_config_hosts, ConnectionChange};
use crate::server::ConnectMode;
use gu_net::NodeId;
use log::error;
use serde_derive::*;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
enum Permission {
    ManagedBy(NodeId),
    Read(NodeId),
    // Can create session in given environment
    CreateSession(NodeId, String),
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct PermissionConfig {
    allow_any: bool,
    permissions: HashSet<Permission>,
}

impl PermissionConfig {
    fn add(&self, permission: Permission) -> Self {
        let mut permissions = self.permissions.clone();
        permissions.insert(permission);

        Self {
            permissions,
            allow_any: self.allow_any,
        }
    }

    fn is_managed_by(&self, node_id: &NodeId) -> bool {
        self.permissions.iter().any(|p| match p {
            Permission::ManagedBy(perm_node_id) => node_id == perm_node_id,
            _ => false,
        })
    }

    fn remove_managed_permissions_except_for(&mut self, except_nodes: &HashSet<NodeId>) {
        self.permissions.retain(|perm| match perm {
            Permission::ManagedBy(node_id) => except_nodes.contains(node_id),
            _ => true,
        });
    }
}

impl HasSectionId for PermissionConfig {
    const SECTION_ID: &'static str = "permission";
}

enum PermissionModule {
    None,
    Join(NodeId, Option<SocketAddr>),
    Configure,
    AllowNode(Option<NodeId>), /* None -> auto mode, grant access to every hub */
    DenyNode(Option<NodeId>),  /* None -> manual mode, connect only to selected hubs */
    NodeAllowedStatus(Option<NodeId>), /* Some(n) => is access granted for n, None => is auto mode on */
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
                            .value_name("node_id")
                            .takes_value(true)
                            .help("Gets node permission status, i.e. whether it has sufficient permissions to connect to this provider. \
                                    If node_id is \"any\", then return whether automatic mode is on.")
                    )
                    .arg(
                        Arg::with_name("allow-node")
                            .long("allow-node")
                            .short("a")
                            .value_name("node_id")
                            .takes_value(true)
                            .help("Allow selected hub to connect to this provider and save the new configuration to config files. \
                                If node_id is \"auto\", then allow any hub to connect to this provider.")
                    )
                    .arg(
                        Arg::with_name("deny-node")
                            .long("deny-node")
                            .short("d")
                            .value_name("node_id")
                            .takes_value(true)
                            .help("Deny connections from selected hub to this provider and save the new configuration to config files. \
                                If node_id is \"auto\", then set connection mode to manual (allow only selected hubs to connect to this provider).")
                    ),
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
            let get_node_id = cmd.value_of("get-node").map(|s| NodeId::from_str(s));
            let allow_node_id = cmd.value_of("allow-node").map(|s| NodeId::from_str(s));
            let deny_node_id = cmd.value_of("deny-node").map(|s| NodeId::from_str(s));
            if get_node_id.is_none() && allow_node_id.is_none() && deny_node_id.is_none() {
                *self = PermissionModule::Configure;
                return true;
            } else {
                let param_to_node_id_or_any = |param, param_name| match param {
                    Some(Ok(node_id)) => Ok(Some(node_id)),
                    Some(Err(_)) => match cmd.value_of(param_name) {
                        Some("auto") => Ok(None),
                        _ => Err(()),
                    },
                    _ => Err(()),
                };
                if let Ok(v) = param_to_node_id_or_any(get_node_id, "get-node") {
                    *self = PermissionModule::NodeAllowedStatus(v);
                    return true;
                }
                if let Ok(v) = param_to_node_id_or_any(allow_node_id, "allow-node") {
                    *self = PermissionModule::AllowNode(v);
                    return true;
                }
                if let Ok(v) = param_to_node_id_or_any(deny_node_id, "deny-node") {
                    *self = PermissionModule::DenyNode(v);
                    return true;
                }
                eprintln!("Invalid node_id.");
                return false;
            }
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        use actix::prelude::*;
        use futures::prelude::*;
        use gu_actix::prelude::*;
        use std::sync::Arc;
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
                                let new_config = c.add(Permission::ManagedBy(group_id));
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
            PermissionModule::NodeAllowedStatus(node_id_or_automatic) => {
                let node_id_or_automatic_copy = node_id_or_automatic.clone();
                System::run(move || {
                    let config_manager = ConfigManager::from_registry();
                    Arbiter::spawn(
                        config_manager
                            .send(GetConfig::new())
                            .flatten_fut()
                            .and_then(move |c: Arc<PermissionConfig>| {
                                println!(
                                    "{}",
                                    match node_id_or_automatic_copy {
                                        Some(node_id) => c.is_managed_by(&node_id),
                                        None => c.allow_any,
                                    }
                                );
                                futures::future::ok(())
                            })
                            .map_err(|_| System::current().stop())
                            .and_then(|_r| Ok(System::current().stop())),
                    );
                });
            }
            PermissionModule::AllowNode(node_id) | PermissionModule::DenyNode(node_id) => {
                let turn_on = match self {
                    PermissionModule::AllowNode(_) => true,
                    _ => false,
                };
                let node_id_copy = node_id.clone();
                System::run(move || {
                    let config_manager = ConfigManager::from_registry();
                    Arbiter::spawn(
                        config_manager
                            .send(GetConfig::new())
                            .flatten_fut()
                            .and_then(move |c: Arc<PermissionConfig>| {
                                let mut new_config = (*c).clone();
                                match node_id_copy {
                                    None => {
                                        new_config.allow_any = turn_on;
                                    }
                                    Some(n) => {
                                        let perm = Permission::ManagedBy(n.clone());
                                        if turn_on {
                                            new_config.permissions.insert(perm);
                                        } else {
                                            new_config.permissions.remove(&perm);
                                        }
                                    }
                                };
                                config_manager
                                    .send(SetConfig::new(new_config))
                                    .flatten_fut()
                            })
                            .then(|_r| Ok(System::current().stop())),
                    );
                });
            }
        }
    }
}

fn check_box(v: bool) -> &'static str {
    if v {
        "[X]"
    } else {
        "[ ]"
    }
}

fn run_configure() {
    use actix::prelude::*;
    use futures::prelude::*;
    use gu_actix::prelude::*;
    use gu_lan;
    use std::io;
    use std::sync::Arc;

    fn toggle_managed_by(config: &mut PermissionConfig, hub: &gu_lan::HubDesc) {
        let node_id = NodeId::from_str(&hub.node_id).unwrap();
        let managed_by = Permission::ManagedBy(node_id);

        if !config.permissions.remove(&managed_by) {
            config.permissions.insert(managed_by);
        }
    }

    System::run(|| {
        let get_config = ConfigManager::from_registry()
            .send(GetConfig::new())
            .flatten_fut()
            .map_err(|e| error!("{}", e))
            .and_then(|c: Arc<PermissionConfig>| Ok(c));

        Arbiter::spawn(
            gu_lan::list_hubs()
                .join(get_config)
                .and_then(|(hubs, config_ref)| {
                    let mut config = (*config_ref).clone();
                    let valid_hubs = hubs
                        .into_iter()
                        .filter(|h| !h.node_id.is_empty())
                        .collect::<Vec<gu_lan::HubDesc>>();
                    let except_node_ids: HashSet<NodeId> = valid_hubs
                        .iter()
                        .map(|desc| NodeId::from_str(&desc.node_id).unwrap())
                        .collect();
                    config.remove_managed_permissions_except_for(&except_node_ids);

                    loop {
                        let mut selected_hubs = vec![];
                        println!("Select valid hub:");

                        valid_hubs
                            .iter()
                            .filter(|v| !v.node_id.is_empty())
                            .enumerate()
                            .for_each(|(idx, hub)| {
                                let node_id = NodeId::from_str(&hub.node_id).unwrap();

                                println!(
                                    "{} {}) name={}, addr={:?}, node_id={}",
                                    check_box(config.is_managed_by(&node_id)),
                                    idx + 1,
                                    hub.host_name,
                                    hub.address,
                                    hub.node_id
                                );

                                if config.is_managed_by(&node_id) {
                                    selected_hubs.push(hub.address);
                                    config.permissions.insert(Permission::ManagedBy(node_id));
                                }
                            });
                        println!(
                            "{} *) Access is granted to everyone",
                            check_box(config.allow_any)
                        );
                        println!("    s) Save configuration");
                        println!();

                        let mut input_buf = String::new();
                        let connect_mode = if config.allow_any {
                            ConnectMode::Auto
                        } else {
                            ConnectMode::Manual
                        };

                        eprint!(" => ");
                        io::stdin().read_line(&mut input_buf).unwrap();
                        let input = input_buf.trim();
                        if input == "*" {
                            config.allow_any = !config.allow_any;
                        } else if input == "s" {
                            let selected_nodes = return ConfigManager::from_registry()
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
