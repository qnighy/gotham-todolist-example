extern crate gotham;
extern crate redis_async;
extern crate tokio;

use tokio::prelude::*;

use gotham::middleware::session::{Backend, NewBackend, SessionError, SessionIdentifier};
use redis_async::client::paired::{paired_connect, PairedConnection};
use redis_async::resp::RespValue;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct NewRedisBackend {
    addr: SocketAddr,
    prefix: String,
    ttl: Duration,
}

impl NewRedisBackend {
    pub fn new<A, S>(addr: A, prefix: S, ttl: Duration) -> io::Result<Self>
    where
        A: ToSocketAddrs,
        S: ToString,
    {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        Ok(Self {
            addr: addr,
            prefix: prefix.to_string(),
            ttl: ttl,
        })
    }
}

impl NewBackend for NewRedisBackend {
    type Instance = RedisBackend;
    fn new_backend(&self) -> io::Result<Self::Instance> {
        // Waiting here because it's supposed not to within an event loop.
        let connection = paired_connect(&self.addr).wait().unwrap();
        Ok(RedisBackend {
            connection: connection,
            prefix: self.prefix.clone(),
            ttl: self.ttl,
        })
    }
}

pub struct RedisBackend {
    connection: PairedConnection,
    prefix: String,
    ttl: Duration,
}

impl RedisBackend {
    fn as_key(&self, identifier: &SessionIdentifier) -> Vec<u8> {
        format!("{}{}", self.prefix, identifier.value).into_bytes()
    }
}

impl Backend for RedisBackend {
    fn persist_session(
        &self,
        identifier: SessionIdentifier,
        content: &[u8],
    ) -> Result<(), SessionError> {
        eprintln!("persist_session({:?}, {:?})", identifier, content);
        let key = self.as_key(&identifier);
        let command = RespValue::Array(vec![
            RespValue::BulkString(b"SET".to_vec()),
            RespValue::BulkString(key.clone()),
            RespValue::BulkString(content.to_owned()),
        ]);
        let f = self.connection.send::<()>(command).map_err(|e| {
            eprintln!("persistent_session error: {:?}", e);
        });
        tokio::spawn(f);
        let command = RespValue::Array(vec![
            RespValue::BulkString(b"EXPIRE".to_vec()),
            RespValue::BulkString(key),
            RespValue::BulkString(self.ttl.as_secs().to_string().into_bytes()),
        ]);
        let f = self.connection
            .send::<i64>(command)
            .map(|reply| {
                if reply != 1 {
                    eprintln!("persistent_session expiration error: reply is {}", reply);
                }
            })
            .map_err(|e| {
                eprintln!("persistent_session expiration error: {:?}", e);
            });
        tokio::spawn(f);
        Ok(())
    }
    fn read_session(
        &self,
        identifier: SessionIdentifier,
    ) -> Box<Future<Item = Option<Vec<u8>>, Error = SessionError>> {
        eprintln!("read_session({:?})", identifier);
        let key = self.as_key(&identifier);
        let command = RespValue::Array(vec![
            RespValue::BulkString(b"GET".to_vec()),
            RespValue::BulkString(key),
        ]);
        let f = self.connection
            .send::<Option<Vec<u8>>>(command)
            .map(|x| {
                eprintln!("reply: {:?}", x);
                x
            })
            .map_err(|e| {
                eprintln!("error: {:?}", e);
                SessionError::Backend(format!("Redis error: {}", e))
            });
        Box::new(f)
    }
    fn drop_session(&self, identifier: SessionIdentifier) -> Result<(), SessionError> {
        eprintln!("drop_session({:?})", identifier);
        let key = self.as_key(&identifier);
        let command = RespValue::Array(vec![
            RespValue::BulkString(b"DEL".to_vec()),
            RespValue::BulkString(key),
        ]);
        let f = self.connection.send::<()>(command).map_err(|e| {
            eprintln!("drop_session error: {:?}", e);
        });
        tokio::spawn(f);
        Ok(())
    }
}
