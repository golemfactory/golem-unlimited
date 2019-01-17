extern crate actix;
extern crate actix_web;
extern crate bytes;
extern crate futures;
extern crate gu_client;
extern crate gu_model;
extern crate serde_json;

use actix::Arbiter;
use bytes::Bytes;
use futures::{future, stream, Future, Stream};
use gu_client::async::HubConnection;
use gu_model::session::{BlobInfo, HubSessionSpec};

fn main() {
    let hub_connection = HubConnection::from_addr("127.0.0.1:61622").expect("Invalid address.");
    actix::System::run(move || {
        Arbiter::spawn(
            hub_connection
                .new_session(HubSessionSpec {
                    name: Some("my_session".to_string()),
                    ..HubSessionSpec::default()
                })
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
                    let bytes = Bytes::from("abcde");
                    let stream = stream::iter_ok::<Vec<Bytes>, actix_web::Error>(vec![
                        bytes,
                        Bytes::from("test!"),
                    ]);
                    future::ok(hub_session.clone())
                        .join3(blob.upload_from_stream(stream), future::ok(blob.clone()))
                })
                .and_then(|(hub_session, _, blob)| {
                    println!("Successfully uploaded blob.");
                    future::ok(hub_session.clone()).join(blob.download().collect())
                })
                .and_then(|(hub_session, vec)| {
                    println!("Downloaded blob: {:?}", vec);
                    future::ok(hub_session.clone()).join(hub_session.list_blobs())
                })
                .and_then(|(hub_session, blobs)| {
                    println!("All blobs: {:#?}", blobs.collect::<Vec<BlobInfo>>());
                    future::ok(hub_session.clone()).join(
                        hub_session.add_peers(&["0x2e908c75bbc34997c7464ea2f9118cb5de19f0a6"]),
                    )
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
