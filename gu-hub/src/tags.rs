//! Tag manager
//!
//! Manages tags for providers


use std::collections::{HashMap, HashSet};

use actix::prelude::*;
use log::error;

use gu_net::NodeId;

type Tag = String;
type Tags = HashSet<Tag>;
pub struct TagManager {
    tags: HashMap<NodeId, Tags>
}

impl Actor for TagManager {
    type Context = Context<Self>;
}

#[derive(Message)]
pub struct AddTags {
    node: NodeId,
    tags: Tags
}

#[derive(Message)]
pub struct DeleteTags {
    node: NodeId,
    tags: Tags
}

impl Handler<AddTags> for TagManager {
    type Result = ();
    fn handle(&mut self, msg: AddTags, ctx: &mut Self::Context) -> Self::Result {
        error!("AddTags unimplemented");
    }
}

impl Handler<DeleteTags> for TagManager {
    type Result = ();
    fn handle(&mut self, msg: DeleteTags, ctx: &mut Self::Context) -> Self::Result {
        error!("DeleteTags unimplemented");
    }
}
