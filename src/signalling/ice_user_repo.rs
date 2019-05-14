use actix::Addr;
use actix_redis::{Command, RedisActor};
use crypto::{digest::Digest, md5::Md5};
use futures::future::{self, Future};
use redis_async::resp::RespValue;

use crate::{log::prelude::*, media::ICEUser};

pub struct IceUsersRepository(Addr<RedisActor>);

impl IceUsersRepository {
    pub fn new(redis: Addr<RedisActor>) -> Self {
        Self(redis)
    }

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

    pub fn insert(
        &mut self,
        ice_user: &ICEUser,
    ) -> impl Future<Item = (), Error = ()> {
        debug!("Store ICE user: {:?}", ice_user);
        let key = format!("turn/realm/medea/user/{}/key", ice_user.name);
        let value = format!("{}:medea:{}", ice_user.name, ice_user.pass);
        let mut hasher = Md5::new();
        hasher.input_str(&value);
        let result = hasher.result_str();
        Box::new(
            self.0
                .send(Command(resp_array!["SET", key, result]))
                .map_err(|err| error!("Redis service unreachable: {}", err))
                .and_then(|res| {
                    match res {
                        Ok(RespValue::SimpleString(ref x)) if x == "OK" => {
                            return future::ok(())
                        }
                        Ok(RespValue::Error(err)) => {
                            error!("Redis error: {}", err)
                        }
                        Err(err) => error!("Redis service error: {}", err),
                        _ => (),
                    };
                    future::err(())
                }),
        )
    }

    pub fn remove(
        &mut self,
        ice_user: &ICEUser,
    ) -> impl Future<Item = (), Error = ()> {
        debug!("Delete ICE user: {:?}", ice_user);
        let key = format!("turn/realm/medea/user/{}/key", ice_user.name);
        Box::new(
            self.0
                .send(Command(resp_array!["DEL", key]))
                .map_err(|err| error!("Redis service unreachable: {}", err))
                .then(|_| future::ok(())),
        )
    }
}
