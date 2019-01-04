extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate gu_client;
extern crate gu_model;
extern crate serde_json;

use actix::Arbiter;
use futures::{future, Future};
use gu_client::async::HubConnection;
use gu_client::async::HubSessionInfoBuilder;
use gu_model::session::{BlobInfo, HubSessionSpec};
//use gu_net::rpc::peer::PeerInfo;

fn main() {
    let hub_connection = HubConnection::from_addr("127.0.0.1:61622").expect("Invalid address.");
    actix::System::run(move || {
        Arbiter::spawn(
            hub_connection
                .new_session(
                    HubSessionInfoBuilder::default()
                        .name("my session")
                        .environment("hd"),
                )
                .and_then(|hub_session| {
                    println!("New hub session ready: {:#?}.", hub_session);
                    future::ok(hub_session.clone()).join(hub_session.config())
                })
                .and_then(|(hub_session, mut config)| {
                    println!("Session configuration: {:#?}.", config);
                    config
                        .entry
                        .insert("my_key".to_string(), serde_json::json!("my_value"));
                    future::ok(hub_session.clone()).join(hub_session.set_config(config))
                })
                .and_then(move |(hub_session, _)| {
                    println!("Successfully saved configuration.");
                    future::ok(hub_session.clone()).join(hub_connection.list_sessions())
                })
                .and_then(|(hub_session, list_of_sessions)| {
                    println!(
                        "List of all sessions: {:#?}.",
                        list_of_sessions.collect::<Vec<HubSessionSpec>>()
                    );
                    future::ok(hub_session.clone()).join(hub_session.new_blob())
                })
                .and_then(|(hub_session, blob)| {
                    println!("New blob: {:#?}", blob);
                    future::ok(hub_session.clone()).join(hub_session.new_blob())
                })
                .and_then(|(hub_session, blob)| {
                    println!("Another blob: {:#?}", blob);
                    future::ok(hub_session.clone()).join(hub_session.list_blobs())
                })
                .and_then(|(hub_session, blobs)| {
                    println!("All blobs: {:#?}", blobs.collect::<Vec<BlobInfo>>());
                    future::ok(hub_session.clone()).join(hub_session.add_peers(&["a", "b", "c"]))
                })
                .and_then(|(_hub_session, _)| {
                    println!("Successfully added peers.");
                    future::ok(())
                })
                .map_err(|e| {
                    println!("An error occurred: {:#?}.", e);
                    ()
                })
                .then(|_| {
                    actix::System::current().stop();
                    Ok(())
                }),
        );
    });
}
