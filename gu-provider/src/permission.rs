use gu_base::{App, Arg, ArgMatches, Decorator, Module, SubCommand};
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId, SetConfig};

use gu_net::NodeId;
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
}

impl HasSectionId for PermissionConfig {
    const SECTION_ID: &'static str = "permission";
}

enum PermissionModule {
    None,
    Join(NodeId, Option<SocketAddr>),
    Configure,
}

impl Module for PermissionModule {
    fn args_declare<'a, 'b>(&self, app: App<'a, 'b>) -> App<'a, 'b> {
        app.subcommand(
            SubCommand::with_name("join")
                .args(&[
                    Arg::with_name("group-id")
                        .help("group addr through which the node will be managed")
                        .required(true),
                    Arg::with_name("hub-addr")
                        .required(false)
                        .help("<ip>:<port> of hub"),
                ]).about("allows provider to be managed by group"),
        ).subcommand(SubCommand::with_name("configure"))
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
        } else if let Some(_) = matches.subcommand_matches("configure") {
            *self = PermissionModule::Configure;
            return true;
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self {
            PermissionModule::None => (),
            PermissionModule::Join(ref_group_id, _hub_address) => {
                use actix::prelude::*;
                use futures::prelude::*;
                use gu_actix::prelude::*;
                use std::sync::Arc;

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
                            }).map_err(|_| System::current().stop())
                            .and_then(|_r| Ok(System::current().stop())),
                    );
                });
            }
            PermissionModule::Configure => run_configure(),
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

                    loop {
                        println!("Select valid hub:");

                        hubs.iter()
                            .enumerate()
                            .filter(|v| !v.1.node_id.is_empty())
                            .for_each(|(idx, hub)| {
                                let node_id = NodeId::from_str(&hub.node_id).unwrap();

                                println!(
                                    "{} {}) name={}, addr={:?}, node_id={}",
                                    check_box(config.is_managed_by(&node_id)),
                                    idx + 1,
                                    hub.host_name,
                                    hub.address,
                                    hub.node_id
                                )
                            });
                        println!(
                            "{} *) Access is granted to everyone",
                            check_box(config.allow_any)
                        );
                        println!("    s) Save configuration");
                        println!();

                        let mut input_buf = String::new();

                        eprint!(" => ");
                        io::stdin().read_line(&mut input_buf).unwrap();
                        let input = input_buf.trim();
                        if input == "*" {
                            config.allow_any = !config.allow_any;
                        } else if input == "s" {
                            return ConfigManager::from_registry()
                                .send(SetConfig::new(config))
                                .then(|_| Ok(()));
                        } else {
                            let idx: usize = input.parse().unwrap_or_default();
                            if idx > 0 && idx <= hubs.len() {
                                toggle_managed_by(&mut config, &hubs[idx - 1])
                            }
                        }
                    }
                }).map_err(|_| System::current().stop())
                .and_then(|_r| Ok(System::current().stop())),
        );
    });
}

pub fn module() -> impl Module {
    PermissionModule::None
}
