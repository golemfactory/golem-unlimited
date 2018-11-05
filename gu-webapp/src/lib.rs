extern crate actix;
extern crate actix_web;
extern crate flate2;
extern crate digest;
extern crate sha1;
extern crate mime;
extern crate regex;

use actix_web::{dev,HttpRequest, Responder, HttpResponse, Error};
use std::collections::HashMap;

pub struct WebApp {
    elements: HashMap<String, WebContent>,
}

impl WebApp {
    pub fn new() -> Self {
        WebApp {
            elements: HashMap::new()
        }
    }

    pub fn add_html(&mut self, path : String, html_str : &str) {
        self.elements.insert(path, WebContent::from_html(html_str));
    }

    pub fn add_bytes(&mut self, path : String, mime : mime::Mime, bytes : Vec<u8>) {
        self.elements.insert(path, WebContent::from_bytes(mime, bytes));
    }

    pub fn add(&mut self, path : String, content : WebContent) {
        self.elements.insert(path, content);
    }
}

impl<S : 'static> dev::Handler<S> for WebApp {
    type Result = Result<dev::AsyncResult<HttpResponse>, Error>;

    fn handle(&self, req: &HttpRequest<S>) -> Self::Result {
        let tail: String = req.match_info().query("tail")?;
        let relpath = tail.trim_left_matches('/');
        if let Some(c) = self.elements.get(relpath) {
            return c.clone().respond_to(&req)?.respond_to(&req)
        }
        HttpResponse::Ok().body(format!("tail={} relpath={}", tail, relpath)).respond_to(&req)
    }
}


mod resource;
mod template;


pub use resource::WebContent;
