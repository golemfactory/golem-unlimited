use super::error::*;
use actix::prelude::*;
use std::borrow::Cow;

pub struct Fetch(pub Cow<'static, str>);

impl Message for Fetch {
    type Result = Result<Option<Vec<u8>>>;
}

pub struct Put(pub Cow<'static, str>, pub Vec<u8>);

impl Message for Put {
    type Result = Result<()>;
}
