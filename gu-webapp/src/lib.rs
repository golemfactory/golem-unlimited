extern crate actix;
extern crate actix_web;
extern crate flate2;
extern crate sha1;

use actix::prelude::*;
use std::borrow::Cow;
use std::collections::HashMap;

pub struct WebApp {
    elements: HashMap<String, WebContent>,
}

pub struct WebContent {
    sha1sum: String,
    content_type: Cow<'static, str>,
    bytes: Vec<u8>,
}
