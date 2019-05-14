use std::net::SocketAddr;

#[cfg(test)]
use actix::actors::mocker::Mocker;
use actix::{
    Actor, ActorFuture, AsyncContext, Context, Handler, Message, WrapFuture,
};
use futures::future::{self, Either, Future};
use hashbrown::HashMap;
use rand::{distributions::Alphanumeric, rngs::OsRng, Rng};
use smart_default::*;

use crate::{
    api::control::MemberId, conf::Conf, media::ICEUser,
    signalling::ice_user_repo::IceUsersRepository,
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

/// Request for delete [`ICEUser`] for [`Member`] from COTURN database.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct DeleteIceUser(pub MemberId);

#[cfg(not(test))]
#[allow(clippy::module_name_repetitions)]
pub type AuthCoturn = AuthService;
#[cfg(test)]
pub type AuthCoturn = Mocker<AuthService>;

/// Service for managing users of COTURN server.
pub struct AuthService {
    /// Address of actor for handle Redis commands.
    coturn_db: IceUsersRepository,

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
    pub fn new(config: &Conf, coturn_db: IceUsersRepository) -> Self {
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

    /// Returns [`ICEUser`] for given [`Member`] with dynamic created
    /// credentials.
    fn create_user(&self, member_id: MemberId) -> ICEUser {
        ICEUser {
            address: self.turn_address,
            name: member_id.to_string(),
            pass: self.new_password(),
        }
    }

    /// Returns [`ICEUser`] with static credentials.
    fn static_user(&self) -> ICEUser {
        ICEUser {
            address: self.turn_address,
            name: self.turn_username.clone(),
            pass: self.turn_password.clone(),
        }
    }
}

/// [`Actor`] implementation that provides an ergonomic way
/// to interact with [`AuthService`].
impl Actor for AuthService {
    type Context = Context<Self>;

    // TODO: authorize after reconnect to Redis
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.coturn_db.init(&self.db_pass).into_actor(self))
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
        let ice_user = self.create_user(msg.member_id);
        let policy_result = match msg.policy {
            RedisUnreachablePolicy::ReturnErr => Err(()),
            RedisUnreachablePolicy::ReturnStatic => Ok(self.static_user()),
        };

        Box::new(
            self.coturn_db
                .insert(&ice_user)
                .then(move |res| match res {
                    Ok(_) => Ok(ice_user),
                    Err(_) => policy_result,
                })
                .into_actor(self)
                .map(move |user, serv, _| {
                    serv.ice_users.insert(msg.member_id, user);
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

impl Handler<DeleteIceUser> for AuthService {
    type Result = ActFuture<(), ()>;

    /// Delete [`ICEUser`] for given [`Member`] from COTURN database.
    fn handle(
        &mut self,
        msg: DeleteIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = match self.ice_users.remove(&msg.0) {
            Some(ice_user) => Either::A(self.coturn_db.remove(&ice_user)),
            None => Either::B(future::ok(())),
        };
        Box::new(fut.into_actor(self))
    }
}

#[cfg(test)]
pub mod test {
    use std::any::Any;

    use crate::media::ICEUser;

    use super::*;
    use actix::Addr;

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
