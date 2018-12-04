extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate gu_client;

use actix::Arbiter;
use futures::{future, Future};
use gu_client::async::HubConnection;
use gu_client::async::SessionInfoBuilder;
//use gu_net::rpc::peer::PeerInfo;

fn main() {
    let hub_connection = HubConnection::from_addr("127.0.0.1:61622").expect("Invalid address.");
    actix::System::run(move || {
        Arbiter::spawn(
            hub_connection
                .new_session(
                    SessionInfoBuilder::default()
                        .name("my session")
                        .environment("hd"),
                ).and_then(|hub_session| {
                    println!("New hub session ready: {:#?}.", hub_session);
                    future::ok(hub_session.clone()).join(hub_session.new_blob())
                }).and_then(|(hub_session, blob)| {
                    println!("New blob: {:#?}", blob);
                    future::ok(hub_session.clone()).join(hub_session.add_peers(&["a", "b", "c"]))
                }).and_then(|(_hub_session, _)| {
                    println!("Successfully added peers.");
                    future::ok(())
                }).map_err(|e| {
                    println!("An error occurred: {:#?}.", e);
                    ()
                }).then(|_| {
                    actix::System::current().stop();
                    Ok(())
                }),
        );
    });
}
