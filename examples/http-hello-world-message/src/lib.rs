use wstd::http::{Body, Request, Response, StatusCode};

use wit_wasmcloud_messaging_bindgen::generate_wit_nats_proxy_from_wit;

generate_wit_nats_proxy_from_wit!(
    world: "outer-space",
    bindings_world: "hello",
);

#[wstd::http_server]
async fn main(req: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    match req.uri().path_and_query().unwrap().as_str() {
        "/" => home(req).await,
        _ => home(req).await,
    }
}

async fn home(req: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    // Return a simple response with a string body
    let path = req.uri().path_and_query().expect("no query?").path();
    let _res = match handle_nats(path.into()) {
        Ok(str) => str,
        Err(_str) => return Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Not found\n".into())
        .unwrap()),
    };
    Ok(Response::new(format!("Hello {} !\n", path).into()))
}

async fn not_found(_req: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body("Not found\n".into())
        .unwrap())
}
