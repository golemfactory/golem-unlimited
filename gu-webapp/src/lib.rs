extern crate sha1;
extern crate flate2;
extern crate actix;
extern crate actix_web;

use actix::prelude::*;
use std::collections::HashMap;
use std::borrow::Cow;

pub struct WebApp {
    elements : HashMap<String, WebContent>
}

pub struct WebContent {
    sha1sum : String,
    content_type : Cow<'static, str>,
    bytes : Vec<u8>
}

