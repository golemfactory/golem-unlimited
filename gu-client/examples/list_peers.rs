extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate gu_client;
extern crate gu_net;

use actix::Arbiter;
use futures::Future;
use gu_client::async::Driver;
//use gu_net::rpc::peer::PeerInfo;

fn main() {
    let driver = Driver::from_addr("10.30.8.179:61622");
    actix::System::run(move || {
        Arbiter::spawn(
            driver
                .list_peers()
                .and_then(|peers| {
                    peers.for_each(|peer| println!("peer={:?}", peer.node_id));
                    Ok(actix::System::current().stop())
                }).map_err(|_| {
                    println!("Error while listing peers.");
                    actix::System::current().stop();
                    ()
                }),
        );
    });
}
