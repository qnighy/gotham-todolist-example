use gotham::handler::HandlerFuture;
use gotham::middleware::{Middleware, NewMiddleware};
use gotham::state::{State, StateData};
use mysql_async::{self, Opts};
use std::io;
use std::ops;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use tokio::reactor::Handle;

pub struct NewMysqlMiddleware {
    opts: AssertUnwindSafe<Opts>,
}

impl NewMysqlMiddleware {
    pub fn new<O: Into<Opts>>(opts: O) -> Self {
        Self {
            opts: AssertUnwindSafe(opts.into()),
        }
    }
}

impl NewMiddleware for NewMysqlMiddleware {
    type Instance = MysqlMiddleware;
    fn new_middleware(&self) -> io::Result<Self::Instance> {
        let pool = mysql_async::Pool::new(self.opts.0.clone(), &Handle::current());
        Ok(MysqlMiddleware {
            pool: Arc::new(pool),
        })
    }
}

pub struct MysqlMiddleware {
    pool: Arc<mysql_async::Pool>,
}

#[derive(Clone)]
pub struct Pool {
    pool: Arc<mysql_async::Pool>,
}

impl StateData for Pool {}

impl ops::Deref for Pool {
    type Target = mysql_async::Pool;

    fn deref(&self) -> &Self::Target {
        &self.pool
    }
}

impl Middleware for MysqlMiddleware {
    fn call<Chain>(self, mut state: State, chain: Chain) -> Box<HandlerFuture>
    where
        Chain: FnOnce(State) -> Box<HandlerFuture> + 'static,
    {
        let pool = Pool { pool: self.pool };
        state.put(pool);
        chain(state)
    }
}
