#![feature(proc_macro, generators)]

extern crate futures_await as futures;
extern crate gotham;
extern crate hyper;
extern crate mime;
extern crate tokio;

use futures::prelude::*;
use tokio::prelude::*;

use gotham::handler::HandlerError;
use gotham::http::response::create_response;
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::State;
use hyper::{Response, StatusCode};

type HandlerResult = Result<(State, Response), (State, HandlerError)>;

fn router() -> Router {
    build_simple_router(|route| {
        route.get_or_head("/").to(index);
    })
}

#[async(boxed)]
fn index(state: State) -> HandlerResult {
    let response = create_response(
        &state,
        StatusCode::Ok,
        Some(("Hello, world!".to_string().into_bytes(), mime::TEXT_PLAIN)),
    );
    Ok((state, response))
}

fn main() {
    let addr = "127.0.0.1:7878";
    gotham::start(addr, router());
}
