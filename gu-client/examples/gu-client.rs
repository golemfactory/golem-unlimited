/***
Command line tool for manage gu-hub instance

**/
use actix::prelude::*;
use failure::Fallible;
use futures::{future, prelude::*};
use gu_client::r#async::*;

fn main() -> Fallible<()> {
    let mut sys = System::new("gu-client");

    sys.block_on(future::lazy(|| {
        let driver = HubConnection::default();
        driver
            .list_peers()
            .and_then(|p| Ok(eprintln!("{:?}", p.collect::<Vec<_>>())))
    }))?;

    Ok(())
}
