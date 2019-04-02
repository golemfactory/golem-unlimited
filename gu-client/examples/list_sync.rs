extern crate gu_client;
extern crate gu_net;

use gu_client::r#async::HubConnection;
use gu_client::sync;

fn main() {
    let h = sync::start();

    let hub_connection = HubConnection::from_addr("127.0.0.1:61622").expect("Invalid address.");

    let peers = {
        let c = hub_connection.clone();
        h.wait(move || c.list_peers()).unwrap()
    };

    println!("got peers");
    for peer in peers {
        println!("p={:?}", peer);
    }
}
