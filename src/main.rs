#![feature(proc_macro, generators)]

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate futures_await as futures;
extern crate gotham;
extern crate handlebars_gotham;
extern crate hyper;
extern crate mime;
extern crate tokio;

use futures::prelude::*;
use tokio::prelude::*;

use gotham::handler::HandlerError;
use gotham::http::response::create_response;
use gotham::middleware::session::{NewSessionMiddleware, SessionData};
use gotham::pipeline::new_pipeline;
use gotham::pipeline::single::single_pipeline;
use gotham::router::builder::*;
use gotham::router::Router;
use gotham::state::State;
use handlebars_gotham::{DirectorySource, HandlebarsEngine, Template};
use hyper::{Method, Response, StatusCode};
use serde_json::{Map, Value};

type HandlerResult = Result<(State, Response), (State, HandlerError)>;

#[derive(Default, Serialize, Deserialize)]
struct MySession {
    counter: usize,
}

fn router() -> Router {
    let hbse = HandlebarsEngine::new(vec![Box::new(DirectorySource::new("./templates/", ".hbs"))]);
    hbse.reload().unwrap();

    let sessions = NewSessionMiddleware::default().with_session_type::<MySession>();
    let sessions = sessions.insecure(); // For non-HTTPS server

    let pipeline = new_pipeline().add(hbse).add(sessions).build();
    let (chain, pipelines) = single_pipeline(pipeline);
    build_router(chain, pipelines, |route| {
        route.get_or_head("/").to(index);
        route.get_or_head("/counter").to(counter);
    })
}

#[async(boxed)]
fn index(state: State) -> HandlerResult {
    let mut state = state;
    let method = state.borrow::<Method>().clone();
    if method == Method::Get {
        state.put(Template::new("index", &json!({})));
    }

    let response = create_response(&state, StatusCode::Ok, None);

    Ok((state, response))
}

#[async(boxed)]
fn counter(state: State) -> HandlerResult {
    let mut state = state;
    let method = state.borrow::<Method>().clone();
    if method == Method::Get {
        let counter = {
            let mut session = state.borrow_mut::<SessionData<MySession>>();
            session.counter += 1;
            session.counter
        };
        state.put(Template::new("counter", &json!({ "counter": counter, })));
    }

    let response = create_response(&state, StatusCode::Ok, None);

    Ok((state, response))
}

fn main() {
    let addr = "127.0.0.1:7878";
    gotham::start(addr, router());
}
