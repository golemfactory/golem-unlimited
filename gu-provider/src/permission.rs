use gu_base::{App, AppSettings, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId, SetConfig};

use crate::connect::{
    change_single_connection, edit_config_connect_mode, edit_config_hosts, ConnectionChange,
};
use crate::server::ConnectMode;
use futures::future::Either;
use gu_lan::{list_hubs, HubDesc};
use gu_net::NodeId;
use log::error;
use serde_derive::*;
use std::collections::{HashMap, HashSet};
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
    AllowNode(Option<NodeId>, Option<SocketAddr>, Option<String>), /* node, ip, name; 3x None -> auto mode, grant access to every hub */
    DenyNode(Option<NodeId>, Option<SocketAddr>, Option<String>), /* node, ip, name; 3x None -> manual mode, connect only to selected hubs */
    NodeAllowedStatus(Option<NodeId>), /* Some(n) => is access granted for n, None => is auto mode on */
    ListSavedHubs,
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
                        return Ok((None, None, None));
                    }
                    match NodeId::from_str(params[0]) {
                        Ok(node) => {
                            if params.len() == 1 && param_name == "get-node" {
                                Ok((Some(node), None, None))
                            } else if params.len() < 2 || params.len() > 3 {
                                Err(())
                            } else {
                                let sock_addr: SocketAddr =
                                    params[1].parse().expect("Expected ip:port.");
                                Ok((
                                    Some(node),
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
                    *self = PermissionModule::AllowNode(None, None, None);
                    return true;
                }
                if cmd.is_present("deny-unknown") {
                    *self = PermissionModule::DenyNode(None, None, None);
                    return true;
                }
                if cmd.is_present("list-saved-hubs") {
                    *self = PermissionModule::ListSavedHubs;
                    return true;
                }
                if let Ok((node, ip, _)) = param_to_node_and_ip("get-node") {
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
            PermissionModule::AllowNode(node_id, ip, host_name)
            | PermissionModule::DenyNode(node_id, ip, host_name) => {
                let turn_on = match self {
                    PermissionModule::AllowNode(_, _, _) => true,
                    _ => false,
                };
                let node_id_copy = node_id.clone();
                let ip_copy = ip.clone();
                let host_name_copy = host_name.clone();
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
                                            if host_name_copy.is_some() && ip_copy.is_some() {
                                                new_config.saved_hub_desc.insert(
                                                    n,
                                                    HubDesc {
                                                        address: ip_copy.unwrap(),
                                                        host_name: host_name_copy.unwrap(),
                                                        node_id: n,
                                                    },
                                                );
                                            }
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
                            .map_err(|_| eprintln!("Cannot save permissions."))
                            .and_then(move |x| match node_id_copy {
                                None => futures::future::Either::A(
                                    edit_config_connect_mode(if turn_on {
                                        ConnectMode::Auto
                                    } else {
                                        ConnectMode::Manual
                                    })
                                    .map_err(|_| ()),
                                ),
                                Some(n) => futures::future::Either::B(
                                    change_single_connection(
                                        ip_copy.unwrap(),
                                        if turn_on {
                                            ConnectionChange::Connect
                                        } else {
                                            ConnectionChange::Disconnect
                                        },
                                    )
                                    .map_err(|_| ()),
                                ),
                            })
                            .then(|_r| Ok(System::current().stop())),
                    );
                });
            }
            PermissionModule::ListSavedHubs => {
                System::run(move || {
                    let config_manager = ConfigManager::from_registry();
                    Arbiter::spawn(
                        config_manager
                            .send(GetConfig::new())
                            .flatten_fut()
                            .and_then(move |c: Arc<PermissionConfig>| {
                                let mut output: Vec<HubDesc> =
                                    c.saved_hub_desc.values().cloned().collect::<Vec<HubDesc>>();
                                output.sort_unstable_by(|a, b| a.host_name.cmp(&b.host_name));
                                println!("{}", serde_json::to_string_pretty(&output).unwrap());
                                futures::future::ok(())
                            })
                            .map_err(|_| System::current().stop())
                            .and_then(|_r| Ok(System::current().stop())),
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
        let node_id = hub.node_id;
        let managed_by = Permission::ManagedBy(node_id);

        if !config.permissions.remove(&managed_by) {
            config.permissions.insert(managed_by);
            config.saved_hub_desc.insert(node_id, hub.clone());
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
                                config
                                    .permissions
                                    .insert(Permission::ManagedBy(hub.node_id));
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
