use actix::prelude::*;
use actix_web::*;
use gu_actix::prelude::*;

use super::actor::{Hardware, HardwareActor, HardwareQuery};

fn hwinfo<S>(_: HttpRequest<S>) -> impl Responder {
    "unimplemnted"
}
