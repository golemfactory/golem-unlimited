use std::env;

use actix_web::{client, http::Method};
use futures::future::*;
use http::Uri;
use serde::Serialize;

#[derive(Serialize)]
enum Command {
    #[serde(rename_all = "camelCase")]
    RegisterCommand { cmd_name: String, url: String },
}

fn get_service_url() -> Result<Uri, http::Error> {
    match env::var("GU_HUB_ADDR") {
        Ok(addr) => Uri::builder()
            .scheme("http")
            .authority(addr.parse::<http::uri::Authority>()?)
            .path_and_query("/service/local")
            .build(),
        Err(_addr) => Ok(Uri::from_static("http://127.0.0.1:61622/service/local")),
    }
}

/// Registers command service in hub registry.
pub fn register_service(url: &str, cmd_name: &str) -> impl Future<Item = (), Error = ()> {
    let command = Command::RegisterCommand {
        cmd_name: cmd_name.into(),
        url: url.into(),
    };
    let client = client::Client::default();

    get_service_url()
        .into_future()
        .map_err(|e| log::error!("server uri error: {}", e))
        .and_then(move |uri| {
            client
                .request(Method::PATCH, uri)
                .send_json(&command)
                .into_future()
                .map_err(|e| eprintln!("hub connection error: {}", e))
        })
        .and_then(|mut r| {
            r.json()
                .map_err(|e| eprintln!("hub connection error: {}", e))
        })
        .and_then(|v: serde_json::Value| Ok(log::info!("registered service [{}]", v.to_string())))
}
