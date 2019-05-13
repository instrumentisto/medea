use std::net::SocketAddr;

#[cfg(test)]
use actix::actors::mocker::Mocker;
use actix::{
    Actor, ActorFuture, Addr, AsyncContext, Context, Handler, Message,
    WrapFuture,
};
use actix_redis::{Command, RedisActor};
use crypto::{digest::Digest, md5::Md5};
use futures::future::{self, Future};
use hashbrown::HashMap;
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use redis_async::resp::RespValue;
use smart_default::*;

use crate::{
    api::control::MemberId, conf::Conf, log::prelude::*, media::ICEUser,
};

#[derive(Debug, SmartDefault)]
pub enum RedisUnreachablePolicy {
    #[default]
    ReturnErr,
    ReturnStatic,
}

/// Request for create [`ICEUser`] for [`Member`] on COTURN server.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct CreateIceUser {
    pub member_id: MemberId,
    pub policy: RedisUnreachablePolicy,
}

/// Request for obtain [`ICEUser`] for [`Member`].
#[derive(Debug, Message)]
#[rtype(result = "Result<ICEUser, ()>")]
pub struct GetIceUser(pub MemberId);

#[cfg(not(test))]
#[allow(clippy::module_name_repetitions)]
pub type AuthCoturn = AuthService;
#[cfg(test)]
pub type AuthCoturn = Mocker<AuthService>;

/// Service for managing users of COTURN server.
pub struct AuthService {
    /// Address of actor for handle Redis commands.
    coturn_db: Addr<RedisActor>,

    /// Credentials to authorize remote Web Client on TURN server.
    ice_users: HashMap<MemberId, ICEUser>,

    /// Password for authorize on Redis server.
    db_pass: String,

    /// Address COTURN server.
    turn_address: SocketAddr,

    /// Static username for connection COTURN server.
    turn_username: String,

    /// Static password for connection COTURN server.
    turn_password: String,
}

impl AuthService {
    /// Create new instance [`AuthService`].
    pub fn new(config: &Conf, coturn_db: Addr<RedisActor>) -> Self {
        Self {
            coturn_db,
            db_pass: config.redis.pass.clone(),
            ice_users: HashMap::new(),
            turn_address: config.turn.get_addr(),
            turn_username: config.turn.user.clone(),
            turn_password: config.turn.pass.clone(),
        }
    }

    /// Generates credentials for authorize remote Web Client on COTURN server.
    fn new_password(&self) -> String {
        OsRng.sample_iter(&Alphanumeric).take(16).collect()
    }

    /// Create [`ICEUser`] with dynamic created credentials.
    /// If COTURN database unreachable and given policy is [`ReturnStatic`]
    /// returns static ICE user.
    fn create_user(
        &self,
        member_id: MemberId,
        policy: &RedisUnreachablePolicy,
    ) -> impl Future<Item = ICEUser, Error = ()> {
        let ice_user = ICEUser {
            address: self.turn_address,
            name: member_id.to_string(),
            pass: self.new_password(),
        };
        let policy_result = match policy {
            RedisUnreachablePolicy::ReturnErr => Err(()),
            RedisUnreachablePolicy::ReturnStatic => Ok(ICEUser {
                address: self.turn_address,
                name: self.turn_username.clone(),
                pass: self.turn_password.clone(),
            }),
        };

        self.store_user(ice_user).then(move |res| match res {
            Ok(user) => Ok(user),
            Err(_) => policy_result,
        })
    }

    /// Store [`ICEUser`] credential in COTURN database.
    fn store_user(
        &self,
        ice_user: ICEUser,
    ) -> impl Future<Item = ICEUser, Error = ()> {
        let key = format!("turn/realm/medea/user/{}/key", ice_user.name);
        let value = format!("{}:medea:{}", ice_user.name, ice_user.pass);
        let mut hasher = Md5::new();
        hasher.input_str(&value);
        let result = hasher.result_str();
        Box::new(
            self.coturn_db
                .send(Command(resp_array!["SET", key, result]))
                .map_err(|err| error!("Redis service unreachable: {}", err))
                .and_then(|res| {
                    match res {
                        Ok(RespValue::SimpleString(ref x)) if x == "OK" => {
                            return future::ok(ice_user)
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

    /// Delete [`ICEUser`] from COTURN database.
    pub fn delete(&self, _member_id: MemberId) {}
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`AuthService`].
impl Actor for AuthService {
    type Context = Context<Self>;

    // TODO: authorize after reconnect to Redis
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(
            self.coturn_db
                .send(Command(resp_array!["AUTH", &self.db_pass]))
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
                .into_actor(self),
        )
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`AuthService`].
type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = AuthService, Item = I, Error = E>>;

impl Handler<CreateIceUser> for AuthService {
    type Result = ActFuture<(), ()>;

    /// Create [`ICEUser`] for given [`Member`] and store its in COTURN
    /// database.
    fn handle(
        &mut self,
        msg: CreateIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let member_id = msg.member_id;
        Box::new(
            self.create_user(msg.member_id, &msg.policy)
                .into_actor(self)
                .map(move |ice_user, serv, _| {
                    serv.ice_users.insert(member_id, ice_user);
                }),
        )
    }
}

impl Handler<GetIceUser> for AuthService {
    type Result = ActFuture<ICEUser, ()>;

    /// Returns [`ICEUser`] for [`Member`].
    fn handle(
        &mut self,
        msg: GetIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        Box::new(
            future::done(self.ice_users.get(&msg.0).cloned().ok_or(()))
                .into_actor(self),
        )
    }
}

#[cfg(test)]
pub mod test {
    use std::any::Any;

    use crate::media::ICEUser;

    use super::*;

    pub fn create_service() -> Addr<AuthCoturn> {
        AuthCoturn::create(|_ctx| {
            AuthCoturn::mock({
                let handler =
                    |a: Box<Any>, _b: &mut Context<AuthCoturn>| -> Box<Any> {
                        if let Ok(_out) = a.downcast::<GetIceUser>() {
                            Box::new(Some(Result::<_, ()>::Ok(ICEUser {
                                address: "5.5.5.5:1234".parse().unwrap(),
                                name: "username".to_string(),
                                pass: "password".to_string(),
                            })))
                        } else {
                            Box::new(Some(Result::<_, ()>::Ok(())))
                        }
                    };
                Box::new(handler)
            })
        })
    }
}
