extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate gu_client;

use actix::Arbiter;
use futures::Future;
use gu_client::async::Driver;
use gu_client::async::SessionInfoBuilder;
//use gu_net::rpc::peer::PeerInfo;

fn main() {
    let driver = Driver::from_addr("10.30.8.179:61622");
    actix::System::run(move || {
        Arbiter::spawn(
            driver
                .new_session(
                    SessionInfoBuilder::default()
                        .name("my session")
                        .environment("hd"),
                ).and_then(|hub_session| {
                    println!("New hub session ready: {}.", hub_session.session_id);
                    Ok(actix::System::current().stop())
                }).map_err(|_| {
                    println!("Cannot open a hub session.");
                    actix::System::current().stop();
                    ()
                }),
        );
    });
}
