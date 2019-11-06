//! Abstraction over remote Redis database used to store Turn server
//! credentials.
use std::time::Duration;

use bb8::{Pool, RunError};
use bb8_redis::{RedisConnectionManager, RedisPool};
use crypto::{digest::Digest, md5::Md5};
use derive_more::Display;
use failure::Fail;
use futures::future::Future;
use redis::{ConnectionInfo, RedisError};
use tokio::prelude::*;

use crate::{log::prelude::*, media::IceUser};

#[derive(Debug, Display, Fail)]
pub enum TurnDatabaseErr {
    #[display(fmt = "Redis returned error: {}", _0)]
    RedisError(RedisError),
}

impl From<RedisError> for TurnDatabaseErr {
    fn from(err: RedisError) -> Self {
        Self::RedisError(err)
    }
}

// Abstraction over remote Redis database used to store Turn server
// credentials.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct TurnDatabase {
    pool: RedisPool,
}

impl TurnDatabase {
    pub fn new<S: Into<ConnectionInfo> + Clone>(
        conn_timeout: Duration,
        conn_info: S,
    ) -> impl Future<Item = Self, Error = TurnDatabaseErr> {
        future::lazy(move || redis::Client::open(conn_info.into()))
            .and_then(RedisConnectionManager::new)
            .and_then(move |conn_mngr| {
                Pool::builder()
                    .connection_timeout(conn_timeout)
                    .build(conn_mngr)
            })
            .map(RedisPool::new)
            .map(|pool| Self { pool })
            .map_err(TurnDatabaseErr::from)
    }

    /// Inserts provided [`IceUser`] into remote Redis database.
    pub fn insert(
        &mut self,
        user: &IceUser,
    ) -> impl Future<Item = (), Error = RunError<TurnDatabaseErr>> {
        debug!("Store ICE user: {:?}", user);

        let key = format!("turn/realm/medea/user/{}/key", user.user());
        let value = format!("{}:medea:{}", user.user(), user.pass());

        let mut hasher = Md5::new();
        hasher.input_str(&value);
        let result = hasher.result_str();

        self.pool.run(|connection| {
            redis::cmd("SET")
                .arg(key)
                .arg(result)
                .query_async(connection)
                .map_err(TurnDatabaseErr::RedisError)
        })
    }

    /// Deletes batch of provided [`IceUser`]s.
    pub fn remove(
        &mut self,
        users: &[IceUser],
    ) -> impl Future<Item = (), Error = bb8::RunError<TurnDatabaseErr>> {
        debug!("Remove ICE users: {:?}", users);
        let mut delete_keys = Vec::with_capacity(users.len());

        for user in users {
            delete_keys
                .push(format!("turn/realm/medea/user/{}/key", user.user()));
        }

        self.pool.run(|connection| {
            redis::cmd("DEL")
                .arg(delete_keys)
                .to_owned()
                .query_async(connection)
                .map_err(TurnDatabaseErr::RedisError)
        })
    }
}
