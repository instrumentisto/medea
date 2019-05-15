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
    turn::IceUsersRepository,
};

/// Defines [`TurnAuthService`] behaviour if remote database is unreachable
#[derive(Debug, SmartDefault)]
pub enum UnreachablePolicy {
    /// Error will be propagated if request to db fails cause it is
    /// unreachable.
    #[default]
    ReturnErr,
    /// Static member credentials will be returned if request to db fails cause
    /// it is unreachable.
    ReturnStatic,
}

static TURN_PASS_LEN: usize = 16;

/// Creates credentials on Turn server for specified member.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct CreateIceUser {
    pub member_id: MemberId,
    pub policy: UnreachablePolicy,
}

/// Request to obtain [`ICEUser`] for [`Member`].
#[derive(Debug, Message)]
#[rtype(result = "Result<ICEUser, ()>")]
pub struct GetIceUser(pub MemberId);

/// Deletes specified [`Member`] Turn credentials.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), ()>")]
pub struct DeleteIceUser(pub MemberId);

#[cfg(not(test))]
#[allow(clippy::module_name_repetitions)]
pub type TurnAuthService = Service;
#[cfg(test)]
pub type TurnAuthService = Mocker<Service>;

/// Manages Turn server credentials.
pub struct Service {
    /// Address of actor for handle Redis commands.
    turn_db: IceUsersRepository,
    /// Credentials to authorize remote Web Client on TURN server.
    ice_users: HashMap<MemberId, ICEUser>,
    /// Password for authorize on Redis server.
    db_pass: String,
    /// Turn server address.
    turn_address: SocketAddr,
    /// Turn server static user.
    turn_username: String,
    /// Turn server static user password.
    turn_password: String,
}

impl Service {
    /// Create new instance [`AuthService`].
    pub fn new(config: &Conf) -> Self {
        Self {
            turn_db: IceUsersRepository::new(
                config.redis.get_addr().to_string(),
            ),
            db_pass: config.redis.pass.clone(),
            ice_users: HashMap::new(),
            turn_address: config.turn.get_addr(),
            turn_username: config.turn.user.clone(),
            turn_password: config.turn.pass.clone(),
        }
    }

    /// Generates random alphanumeric string of specified length.
    fn new_password(&self, n: usize) -> String {
        OsRng.sample_iter(&Alphanumeric).take(n).collect()
    }

    /// Returns [`ICEUser`] for given [`Member`] with dynamic created
    /// credentials.
    fn create_user(&self, member_id: MemberId) -> ICEUser {
        ICEUser {
            address: self.turn_address,
            name: member_id.to_string(),
            pass: self.new_password(TURN_PASS_LEN),
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
impl Actor for Service {
    type Context = Context<Self>;

    // TODO: authorize after reconnect to Redis
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.turn_db.init(&self.db_pass).into_actor(self))
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`AuthService`].
type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Service, Item = I, Error = E>>;

impl Handler<CreateIceUser> for Service {
    type Result = ActFuture<(), ()>;

    /// Create and registers [`ICEUser`] for given [`Member`].
    fn handle(
        &mut self,
        msg: CreateIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let ice_user = self.create_user(msg.member_id);
        let policy_result = match msg.policy {
            UnreachablePolicy::ReturnErr => Err(()),
            UnreachablePolicy::ReturnStatic => Ok(self.static_user()),
        };

        Box::new(
            self.turn_db
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

impl Handler<GetIceUser> for Service {
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

impl Handler<DeleteIceUser> for Service {
    type Result = ActFuture<(), ()>;

    /// Deletes [`ICEUser`] for given [`Member`].
    fn handle(
        &mut self,
        msg: DeleteIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let fut = match self.ice_users.remove(&msg.0) {
            Some(ice_user) => Either::A(self.turn_db.remove(&ice_user)),
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

    pub fn dummy() -> Addr<TurnAuthService> {
        TurnAuthService::create(|_ctx| {
            TurnAuthService::mock({
                let handler = |a: Box<Any>,
                               _b: &mut Context<TurnAuthService>|
                 -> Box<Any> {
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
