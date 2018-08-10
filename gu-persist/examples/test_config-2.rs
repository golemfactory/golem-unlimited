extern crate actix;
extern crate futures;
extern crate gu_persist;
#[macro_use]
extern crate serde_derive;

use actix::prelude::*;
use futures::prelude::*;
use gu_persist::error::*;
use std::borrow::Cow;
use std::env;
use std::sync::Arc;

enum Cmd {
    Fetch(String),
    Put(String, String),
}

struct MyActor;

#[derive(Serialize, Deserialize, Default)]
struct MyConfig {
    test: String,
    val: u64,
}

impl gu_persist::config::HasSectionId for MyConfig {
    const SECTION_ID: &'static str = "my-cfg";
}

impl Actor for MyActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        use gu_persist::config::*;
        let config_mgr = ConfigManager::from_registry();

        println!("started");
        let f = config_mgr
            .send(GetConfig::new())
            .and_then(|r: Result<Arc<MyConfig>>| {
                let c = r.unwrap();
                println!("test={}, val={}", c.test, c.val);
                Ok(())
            });
        f.map_err(|e| println!("err {:?}", e))
            .then(|_| Ok(()))
            .into_actor(self)
            .spawn(ctx);
    }
}

fn main() {
    let sys = actix::System::new("test-config");

    let _ = MyActor.start();

    let _ = sys.run();
}
