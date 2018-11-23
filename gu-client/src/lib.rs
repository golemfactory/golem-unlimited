extern crate actix;
extern crate actix_web;
extern crate futures;
extern crate gu_net;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate derive_builder;

mod async;
mod error;

#[cfg(test)]
mod tests {
    use actix::Arbiter;
    use async::Driver;
    use futures::Future;
    //use gu_net::rpc::peer::PeerInfo;
    #[test]
    fn test_driver() {
        actix::System::run(|| {
            let driver = Driver::from_addr("10.30.10.202:61622"); // 10.202:
            Arbiter::spawn(
                /*
                driver
                    .list_peers()
                    .and_then(|peers| {
                        peers.for_each(|peer| println!("peer={:?}", peer.node_id));
                        Ok(actix::System::current().stop())
                    }).map_err(|e| {
                        println!("ERR {:?}", e);
                        actix::System::current().stop();
                        ()
                    }),
                */
                driver
                    .new_session("hd")
                    .name("my session")
                    .send()
                    .and_then(|hub_session| {
                        println!("###{:?}!!!", hub_session);
                        Ok(actix::System::current().stop())
                    }).map_err(|e| {
                        println!("ERR {:?}", e);
                        actix::System::current().stop();
                        ()
                    }),
            );
        });
        assert!(false)
    }
}
