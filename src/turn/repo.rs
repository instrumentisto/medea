//! Abstraction over remote Redis database used to store Turn server
//! credentials.
use actix::{Addr, MailboxError};
use actix_redis::{Command, RedisActor};
use crypto::{digest::Digest, md5::Md5};
use failure::Fail;
use futures::future::Future;
use redis_async::{resp::RespValue, resp_array};

use crate::{log::prelude::*, media::IceUser};

#[derive(Fail, Debug)]
pub enum TurnDatabaseErr {
    #[fail(display = "RedisActor is unreachable: {}", _0)]
    MailboxError(MailboxError),
    #[fail(display = "Redis returned error: {}", _0)]
    RedisError(actix_redis::Error),
    #[fail(display = "Redis returned unknown answer {:?}", _0)]
    UnknownAnswer(RespValue),
}

impl From<MailboxError> for TurnDatabaseErr {
    fn from(err: MailboxError) -> Self {
        TurnDatabaseErr::MailboxError(err)
    }
}

impl From<actix_redis::Error> for TurnDatabaseErr {
    fn from(err: actix_redis::Error) -> Self {
        TurnDatabaseErr::RedisError(err)
    }
}

impl From<RespValue> for TurnDatabaseErr {
    fn from(err: RespValue) -> Self {
        TurnDatabaseErr::UnknownAnswer(err)
    }
}

// Abstraction over remote Redis database used to store Turn server
// credentials.
#[allow(clippy::module_name_repetitions)]
pub struct TurnDatabase(Addr<RedisActor>);

//TODO: Auth after reconnect.
impl TurnDatabase {
    pub fn new<S: Into<String>>(addr: S) -> Self {
        Self(RedisActor::start(addr))
    }

    /// Connects and authenticates connection with remote Redis database.
    pub fn init(&self, db_pass: &str) -> impl Future<Item = (), Error = ()> {
        self.0
            .send(Command(resp_array!["AUTH", db_pass]))
            .map_err(|err| error!("Redis service unreachable: {}", err))
            .map(|res| {
                match res {
                    Ok(RespValue::SimpleString(ref x)) if x == "OK" => {
                        info!("Redis authenticate success.")
                    }
                    Ok(RespValue::Error(err)) => {
                        error!("Redis authenticate filed: {}", err)
                    }
                    Err(err) => error!("Redis service error: {}", err),
                    _ => (),
                };
            })
    }

    /// Inserts provided [`IceUser`] into remote Redis database.
    pub fn insert(
        &mut self,
        ice_user: &IceUser,
    ) -> impl Future<Item = (), Error = TurnDatabaseErr> {
        debug!("Store ICE user: {:?}", ice_user);
        let key = format!("turn/realm/medea/user/{}/key", ice_user.name);
        let value = format!("{}:medea:{}", ice_user.name, ice_user.pass);
        let mut hasher = Md5::new();
        hasher.input_str(&value);
        let result = hasher.result_str();
        Box::new(
            self.0
                .send(Command(resp_array!["SET", key, result]))
                .map_err(TurnDatabaseErr::MailboxError)
                .and_then(Self::parse_redis_answer),
        )
    }

    /// Deletes provided [`IceUser`] from remote Redis database.
    pub fn remove(
        &mut self,
        ice_user: &IceUser,
    ) -> impl Future<Item = (), Error = TurnDatabaseErr> {
        debug!("Delete ICE user: {:?}", ice_user);
        let key = format!("turn/realm/medea/user/{}/key", ice_user.name);
        Box::new(
            self.0
                .send(Command(resp_array!["DEL", key]))
                .map_err(Into::into)
                .and_then(Self::parse_redis_answer),
        )
    }

    /// Parse result from raw Redis answer.
    fn parse_redis_answer(
        result: Result<RespValue, actix_redis::Error>,
    ) -> Result<(), TurnDatabaseErr> {
        match result {
            Ok(resp) => {
                if let RespValue::SimpleString(ref answer) = resp {
                    if answer == "OK" {
                        return Ok(());
                    }
                }
                Err(TurnDatabaseErr::from(resp))
            }
            Err(err) => Err(TurnDatabaseErr::from(err)),
        }
    }
}

impl std::fmt::Debug for TurnDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "TurnAuthRepo {{RedisActor {{connected:{}}}}}",
            self.0.connected()
        )
    }
}
