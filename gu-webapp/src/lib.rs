#![allow(dead_code)]

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
