#![feature(proc_macro, generators)]

extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate futures_await as futures;
extern crate gotham;
extern crate gotham_session_redis;
extern crate handlebars_gotham;
extern crate hyper;
extern crate mime;
extern crate mysql_async;
extern crate tokio;

mod mysql;

use futures::prelude::*;
use mysql_async::prelude::*;
use tokio::prelude::*;

use gotham::handler::{HandlerError, IntoHandlerError};
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
use std::time::Duration;

type HandlerResult = Result<(State, Response), (State, HandlerError)>;

#[derive(Default, Serialize, Deserialize)]
struct MySession {
    counter: usize,
}

fn router() -> Router {
    let hbse = HandlebarsEngine::new(vec![Box::new(DirectorySource::new("./templates/", ".hbs"))]);
    hbse.reload().unwrap();

    let backend = gotham_session_redis::NewRedisBackend::new(
        "127.0.0.1:6379",
        "todolist:session:",
        Duration::from_secs(365 * 24 * 60 * 60),
    ).unwrap();
    let sessions = NewSessionMiddleware::new(backend).with_session_type::<MySession>();
    let sessions = sessions.insecure(); // For non-HTTPS server

    let db = mysql::NewMysqlMiddleware::new("mysql://qnighy:password@127.0.0.1:3306/todolist");

    let pipeline = new_pipeline().add(hbse).add(sessions).add(db).build();
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
        let counter2 = {
            let pool = state.borrow::<mysql::Pool>().clone();
            let result = await!(count_from_mysql(pool));
            match result {
                Ok(count) => count,
                Err(e) => {
                    eprintln!("DB Error: {}", e);
                    return Err((state, e.into_handler_error()));
                }
            }
        };
        state.put(Template::new(
            "counter",
            &json!({ "counter": counter, "counter2": counter2, }),
        ));
    }

    let response = create_response(&state, StatusCode::Ok, None);

    Ok((state, response))
}

#[async]
fn count_from_mysql(pool: mysql::Pool) -> Result<i32, mysql_async::errors::Error> {
    let conn = await!(pool.get_conn())?;
    let conn = await!(conn.drop_query(
        "CREATE TABLE IF NOT EXISTS counter ( \
         id INT NOT NULL PRIMARY KEY, \
         count INT default '0' \
         ) ENGINE INNODB;"
    ))?;
    let opts = mysql_async::TransactionOptions::new();
    let conn = await!(conn.start_transaction(opts))?;
    let (conn, result): (_, Option<(i32,)>) =
        await!(conn.first_exec("SELECT count FROM counter WHERE id = 1;", ()))?;
    let count = if let Some((count,)) = result {
        count
    } else {
        0
    } + 1;
    eprintln!("count = {}", count);
    let conn = await!(conn.drop_exec(
        "INSERT INTO counter (id, count) VALUES (1, ?) \
         ON DUPLICATE KEY UPDATE count = ?;",
        (count, count)
    ))?;
    let conn = await!(conn.commit())?;
    Ok(count)
}

fn main() {
    let addr = "127.0.0.1:7878";
    gotham::start(addr, router());
}
