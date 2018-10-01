use actix_web::dev::HttpResponseBuilder;
use actix_web::http::StatusCode;
use actix_web::HttpResponse;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json;
use std::fmt::Debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct RestResponse<T> {
    pub message: T,
    pub status: u16,
    // TODO: anything more here?
}

pub trait ToHttpResponse: Serialize + DeserializeOwned + Debug {
    fn to_http_response(&self) -> HttpResponse {
        use serde::Serialize;

        let code = self.status_code();

        let response = RestResponse {
            message: self,
            status: u16::from(code),
        };

        HttpResponse::build(code)
            .content_type("application/json")
            .body(serde_json::to_string(&response).expect("Cannot parse response to json"))
    }

    fn message(&self) -> String;

    fn status_code(&self) -> StatusCode;
}

#[derive(Debug, Serialize, Deserialize)]
pub enum InstallQueryResult {
    Installed,
    Overwritten,
    FileAlreadyExists,
    PluginAlreadyExists,
    InvalidPath,
    InvalidMetadata(String),
    InvalidFile(String),
}

impl ToHttpResponse for InstallQueryResult {
    fn message(&self) -> String {
        use self::InstallQueryResult::*;

        match self {
            Installed => "Plugin installed successfully".to_string(),
            Overwritten => "Previous plugin has been replaced".to_string(),
            FileAlreadyExists => "Plugin file for the plugin already exists".to_string(),
            PluginAlreadyExists => "Plugin already exists".to_string(),
            InvalidPath => "Path to resource is invalid".to_string(),
            InvalidMetadata(m) => format!("Metadata file is invalid - {}", m),
            InvalidFile(m) => format!("Received data is invalid - {}", m),
        }
    }

    fn status_code(&self) -> StatusCode {
        use self::InstallQueryResult::*;

        match self {
            Installed => StatusCode::OK,
            Overwritten => StatusCode::OK,
            FileAlreadyExists => StatusCode::CONFLICT,
            PluginAlreadyExists => StatusCode::CONFLICT,
            InvalidPath => StatusCode::BAD_REQUEST,
            InvalidMetadata(_) => StatusCode::BAD_REQUEST,
            InvalidFile(_) => StatusCode::BAD_REQUEST,
        }
    }
}
