extern crate actix_web;
extern crate gu_webapp;
extern crate mime;

use actix_web::{server, App, Path};
use gu_webapp::*;

fn index(info: Path<(String, u32)>) -> String {
    format!("Hello {}! id:{}", info.0, info.1)
}

fn main() {


    server::new(
        || {
            let mut wp = WebApp::new();
            wp.add("smok.html".into(),
                   WebContent::from_gzbytes(mime::TEXT_HTML_UTF_8, include_bytes!("test.html.gz").as_ref().into()));
            wp.add("cover.css".into(),
                   WebContent::from_gzbytes(mime::TEXT_CSS_UTF_8, include_bytes!("cover.css.gz").as_ref().into()));

            App::new()
            .resource("/{name}/{id}/index.html", |r| r.with(index))
        .handler("/test", wp)
        })
        .bind("127.0.0.1:8087").unwrap()
        .run();
}
