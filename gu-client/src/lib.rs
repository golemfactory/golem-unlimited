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
    use async::SessionInfoBuilder;
    use futures::Future;
    //use gu_net::rpc::peer::PeerInfo;
    #[test]
    fn test_driver() {
        let driver = Driver::from_addr("10.30.8.179:61622");
        actix::System::run(move || {
            Arbiter::spawn(
                driver
                    .new_session(
                        SessionInfoBuilder::default()
                            .name("my session")
                            .environment("hd"),
                    ).and_then(|_hub_session| {
                        println!("New hub session ready.");
                        Ok(actix::System::current().stop())
                    }).map_err(|_| {
                        println!("Cannot open a hub session.");
                        actix::System::current().stop();
                        ()
                    }),
            );
        });
        assert!(false)
    }
}
