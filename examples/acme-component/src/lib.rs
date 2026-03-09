use wit_wasmcloud_messaging_bindgen::generate_wit_nats_consumer_proxy_from_wit;
use wstd::http::{Body, Request, Response, StatusCode};

generate_wit_nats_consumer_proxy_from_wit!(
    world: "acme-world-no-serde",
    bindings_world: "acme-world-no-serde",
);

use crate::acme::app::external_function::ExternalInput;

#[wstd::http_server]
async fn main(req: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    let path = req.uri().path();
    let name = path.trim_start_matches('/');
    let name = if name.is_empty() { "world" } else { name };

    match handle_nats(ExternalInput {
        name: name.to_string(),
    }) {
        Ok(msg) => Ok(Response::new(format!("{}\n", msg).into())),
        Err(err) => Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(format!("proxy error: {}\n", err).into())
            .unwrap()),
    }
}
