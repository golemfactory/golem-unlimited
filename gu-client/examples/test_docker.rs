use actix::prelude::*;
use futures::prelude::*;
use structopt::StructOpt;

use gu_client::r#async::*;
use gu_client::NodeId;
use gu_model::dockerman::NetDef;
use gu_model::envman::{Command, CreateSession, Image};
use gu_model::session::HubSessionSpec;

#[derive(StructOpt, Debug)]
struct ClientArgs {
    peer_id: NodeId,
}

fn main() {
    let args = ClientArgs::from_args();
    let mut sys = System::new("test-docker");

    let hub_connection = HubConnection::from_addr("127.0.0.1:61622").expect("Invalid address.");

    let s = serde_json::to_string_pretty(&CreateSession::<gu_model::dockerman::CreateOptions> {
        env_type: "docker".to_string(),
        image: Image {
            url: "tomcat:6.0.44".to_string(),
            hash: "sha256:4f00109135274b73a9cd8b3a46f43353a095515088e724a442752a62e9cfa3b3"
                .to_string(),
        },
        name: "tomcat".to_string(),
        tags: vec![],
        note: None,
        options: gu_model::dockerman::CreateOptions::default().with_net(NetDef::Host {}),
    });
    eprintln!("{}", s.unwrap());

    let result = sys.block_on(
        hub_connection.new_session(HubSessionSpec {
            name: Some("test".to_string()),
            ..HubSessionSpec::default()
        }).and_then(|s: HubSessionRef| {
            let peer_id = args.peer_id.clone();
            let s = s.into_inner().unwrap();
            s.add_peers(vec![args.peer_id])
                .and_then(move |_| {
                    s.peer(peer_id).new_session(CreateSession::<gu_model::dockerman::CreateOptions> {
                        env_type: "docker".to_string(),
                        image: Image {
                            url: "tomcat:6.0.44".to_string(),
                            hash: "sha256:4f00109135274b73a9cd8b3a46f43353a095515088e724a442752a62e9cfa3b3".to_string(),
                        },
                        name: "tomcat".to_string(),
                        tags: vec![],
                        note: None,
                        options:
                        gu_model::dockerman::CreateOptions::default().with_net(NetDef::Host {}),
                    })
                })
                .and_then(|tomcat: PeerSession| {
                    tomcat.update(vec![Command::Open, Command::Wait])
                })
        })
    ).unwrap();

    eprintln!("{:?}", result);
}
