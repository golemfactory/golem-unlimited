extern crate actix;
extern crate futures;
extern crate gu_persist;
#[macro_use]
extern crate serde_derive;

extern crate env_logger;

use actix::prelude::*;
use futures::prelude::*;
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
            .flatten_fut()
            .and_then(|c: Arc<MyConfig>| {
                println!("test={}, val={}", c.test, c.val);
                Ok(())
            });
        f.map_err(|e| println!("my err {:?}", e))
            .then(|_| Ok(()))
            .into_actor(self)
            .spawn(ctx);
    }
}

fn main() {
    env_logger::init();

    let sys = actix::System::new("test-config");

    let _ = MyActor.start();

    let _ = sys.run();
}
