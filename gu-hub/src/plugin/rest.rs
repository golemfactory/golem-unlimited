use actix_web::Scope;
use actix_web::http;
use actix_web::HttpRequest;
use actix_web::Responder;
use futures::future::Future;

pub fn scope<S: 'static>(scope: Scope<S>) -> Scope<S> {
    scope.route("", http::Method::GET, list_scope).route(
        "/{pluginName}/{fileName}",
        http::Method::GET,
        file_scope,
    )
}

fn list_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}



fn file_scope<S>(_r: HttpRequest<S>) -> impl Responder {
    unimplemented!();
    ""
}