extern crate actix;
extern crate gu_lan;
extern crate env_logger;

use actix::prelude::*;

fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let sys = actix::System::new("none_example");

    gu_lan::resolve::ResolveActor{}.start();

    let _ = sys.run();
}