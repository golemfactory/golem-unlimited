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
use gu_client::r#async::HubConnection;
use gu_model::envman::{self, CreateSession, Image};
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
                    println!("New hub session ready: {:?}.", hub_session);
                    future::ok(hub_session.clone()).join(hub_session.config())
                })
                .and_then(|(hub_session, mut config)| {
                    println!("Session configuration: {:?}.", config);
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
                        "List of all sessions: {:?}.",
                        list_of_sessions.collect::<Vec<_>>()
                    );
                    future::ok(hub_session.clone()).join(hub_session.new_blob())
                })
                .and_then(|(hub_session, blob)| {
                    println!("New blob: {:?}", blob);
                    future::ok(hub_session.clone()).join(hub_session.new_blob())
                })
                .and_then(|(hub_session, blob)| {
                    println!("Another blob: {:?}", blob);
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
                    println!("All blobs: {:?}", blobs.collect::<Vec<BlobInfo>>());
                    future::ok(hub_session.clone()).join(
                        hub_session.add_peers(&["0x58137e1abbd59e039abbff4cdef60da7da3cf464"]),
                    )
                })
                .and_then(|(hub_session, result)| {
                    println!("Successfully added peers {:?}.", result);
                    future::ok(hub_session.clone()).join(
                        future::result(
                            hub_session.peer_from_str("0x58137e1abbd59e039abbff4cdef60da7da3cf464"),
                        )
                        .and_then(|peer| {
                            peer.new_session(CreateSession {
                                env_type: "hd".to_string(),
                                image: Image {
                                    url: "http://52.31.143.91/images/gu-factor-linux.tar.gz"
                                        .to_string(),
                                    hash: "not_implemented".to_string(),
                                },
                                name: "peer_session".to_string(),
                                tags: vec![],
                                note: None,
                                options: (),
                            })
                        }),
                    )
                })
                .and_then(|(hub_session, peer_session)| {
                    println!("Peer session created: {:?}.", peer_session);
                    future::ok(hub_session.clone()).join(peer_session.update(vec![
                        envman::Command::AddTags(vec!["my_tag_1".to_string()]),
                        envman::Command::Exec {
                            executable: "gu-factor".to_string(),
                            args: vec!["100".to_string()],
                        },
                        envman::Command::AddTags(vec!["my_tag_2".to_string()]),
                    ]))
                })
                .and_then(|(_hub_session, update_results)| {
                    println!("Update results: {:?}.", update_results);
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
