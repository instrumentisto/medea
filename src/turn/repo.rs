//! Abstraction over remote Redis database used to store Turn server
//! credentials.
use std::time::Duration;

use bb8::{Pool, RunError};
use bb8_redis::{RedisConnectionManager, RedisPool};
use crypto::{digest::Digest, md5::Md5};
use failure::Fail;
use futures::future::Future;
use redis::{ConnectionInfo, RedisError};
use tokio::prelude::*;

use crate::{log::prelude::*, media::IceUser};
use std::{cell::RefCell, rc::Rc};

#[derive(Fail, Debug)]
pub enum TurnDatabaseErr {
    #[fail(display = "Redis returned error: {}", _0)]
    RedisError(RedisError),
}

impl From<RedisError> for TurnDatabaseErr {
    fn from(err: RedisError) -> Self {
        TurnDatabaseErr::RedisError(err)
    }
}

// Abstraction over remote Redis database used to store Turn server
// credentials.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct TurnDatabase {
    pool: RedisPool,
    info: ConnectionInfo,
}

impl TurnDatabase {
    /// New TurnDatabase
    pub fn new<S: Into<ConnectionInfo> + Clone>(
        connection_timeout: Duration,
        connection_info: S,
    ) -> Result<Self, TurnDatabaseErr> {
        let client = redis::Client::open(connection_info.clone().into())?;
        let connection_manager = RedisConnectionManager::new(client)?;

        // Its safe to unwrap here, since this err comes directly from mio and
        // means that mio doesnt have bindings for this target, which wont
        // happen.
        let mut runtime = tokio::runtime::Runtime::new()
            .expect("Unable to create a runtime in TurnDatabase");
        let pool = runtime.block_on(future::lazy(move || {
            Pool::builder()
                .connection_timeout(connection_timeout)
                .build(connection_manager)
        }))?;
        let redis_pool = RedisPool::new(pool);

        Ok(Self {
            pool: redis_pool,
            info: connection_info.into(),
        })
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
        users: &[Rc<RefCell<IceUser>>],
    ) -> impl Future<Item = (), Error = bb8::RunError<TurnDatabaseErr>> {
        debug!("Remove ICE users: {:?}", users);
        let mut delete_keys = Vec::with_capacity(users.len());

        for user in users {
            delete_keys.push(format!(
                "turn/realm/medea/user/{}/key",
                user.borrow().user()
            ));
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
