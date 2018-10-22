use gu_base::{App, Arg, ArgMatches, Module, SubCommand, Decorator};
use gu_persist::config::{ConfigManager, GetConfig, HasSectionId, SetConfig};

use gu_p2p::NodeId;
use std::net::SocketAddr;
use std::str::FromStr;
use std::collections::HashSet;

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

    fn add(&self, permission : Permission) -> Self {
        let mut permissions = self.permissions.clone();
        permissions.insert(permission);

        Self {
            permissions, allow_any: self.allow_any
        }
    }
}

impl HasSectionId for PermissionConfig {
    const SECTION_ID: &'static str = "permission";
}

enum PermissionModule {
    None,
    Join(NodeId, Option<SocketAddr>)
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
                    return true
                },
                (Some(Err(_)), _) => {
                    eprintln!("invalid group id format");
                    return true;
                }
                (_, Some(Err(_))) => {
                    eprintln!("invalid hub addr format");
                    return true;
                },
                (Some(Ok(group_id)), Some(Ok(hub_addr))) => {
                    *self = PermissionModule::Join(group_id, Some(hub_addr));
                    return true;
                },
                (Some(Ok(group_id)), None) => {
                    *self = PermissionModule::Join(group_id, None);
                    return true;
                },

            }
        }
        false
    }

    fn run<D: Decorator + Clone + 'static>(&self, _decorator: D) {
        match self {
            PermissionModule::None => (),
            PermissionModule::Join(ref_group_id, hub_address) => {
                use actix::prelude::*;
                use gu_actix::prelude::*;
                use futures::prelude::*;
                use std::sync::Arc;

                let group_id = ref_group_id.clone();

                System::run(move || {
                    let config_manager = ConfigManager::from_registry();

                    Arbiter::spawn(
                    config_manager.send(GetConfig::new())
                        .flatten_fut()
                        .and_then(move |c : Arc<PermissionConfig>| {
                            let new_config = c.add(Permission::ManagedBy(group_id));
                            config_manager.send(SetConfig::new(new_config))
                                .flatten_fut()
                        }).map_err(|_| System::current().stop())
                        .and_then(|r| Ok(System::current().stop())));

                });
            }

        }
    }
}

pub fn module() -> impl Module {
    PermissionModule::None
}
