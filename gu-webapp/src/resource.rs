
use actix_web::{http::header, Responder, HttpRequest, HttpResponse, HttpMessage, Error, http::StatusCode};
use sha1::{Digest, Sha1};
use mime::{TEXT_HTML_UTF_8, Mime};

#[derive(Clone)]
pub struct WebContent {
    sha1sum: String,
    content_type: Mime,
    gzip : bool,
    bytes: Vec<u8>,
}

impl WebContent {

    pub fn etag(&self) -> header::EntityTag {
        header::EntityTag::strong(self.sha1sum.clone())
    }

    pub fn from_html(html : &str) -> Self {
        Self::from_bytes(TEXT_HTML_UTF_8, html.as_bytes().to_owned())
    }

    pub fn from_bytes(content_type : Mime, bytes : Vec<u8>) -> Self {
        let sha1sum = format!("{:x}", Sha1::digest(bytes.as_ref()));

        Self {
            sha1sum, content_type, bytes, gzip: false
        }
    }

    pub fn from_gzbytes(content_type : Mime, bytes : Vec<u8>) -> Self {
        let sha1sum = format!("{:x}", Sha1::digest(bytes.as_ref()));

        Self {
            sha1sum, content_type, bytes, gzip: true
        }
    }
}

impl Responder for WebContent {
    type Item = HttpResponse;
    type Error = Error;

    fn respond_to<S: 'static>(self, req: &HttpRequest<S>) -> Result<HttpResponse, Error> {
        let etag = self.etag();

        let not_modified = !none_match(Some(&etag), req);

        let mut resp = HttpResponse::build(StatusCode::OK);
        resp.set(header::ContentType(self.content_type));
        resp.set(header::ETag(etag));

        resp.set(header::CacheControl(vec![
            header::CacheDirective::MaxAge(15),
            header::CacheDirective::Public
        ]));
        if self.gzip {
            resp.content_encoding(header::ContentEncoding::Identity);
            resp.header("content-encoding", "gzip");
        }

        if not_modified {
            return Ok(resp.status(StatusCode::NOT_MODIFIED).finish());
        }

        Ok(resp
            .body(self.bytes))

    }
}

fn none_match<S>(etag: Option<&header::EntityTag>, req: &HttpRequest<S>) -> bool {
    match req.get_header::<header::IfNoneMatch>() {
        Some(header::IfNoneMatch::Any) => false,
        Some(header::IfNoneMatch::Items(ref items)) => {
            if let Some(some_etag) = etag {
                for item in items {
                    if item.weak_eq(some_etag) {
                        return false;
                    }
                }
            }
            true
        }
        None => true,
    }
}
