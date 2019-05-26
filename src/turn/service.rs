use core::fmt;
use std::net::SocketAddr;

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Arbiter, AsyncContext, Context,
    Handler, MailboxError, Message, WrapFuture,
};
use failure::Fail;
use futures::future::{err, ok, Future};
use rand::{distributions::Alphanumeric, Rng};
use smart_default::*;

use crate::{
    api::control::MemberId,
    conf::Conf,
    media::IceUser,
    turn::repo::{TurnDatabase, TurnDatabaseErr},
};

static TURN_PASS_LEN: usize = 16;

#[allow(clippy::module_name_repetitions)]
/// Manages Turn server credentials.
pub trait TurnAuthService: fmt::Debug + Send {
    fn create_user(
        &self,
        member_id: MemberId,
        policy: UnreachablePolicy,
    ) -> Box<dyn Future<Item = IceUser, Error = TurnServiceErr>>;

    fn delete_user(
        &self,
        user: IceUser,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>>;
}

impl TurnAuthService for Addr<Service> {
    fn create_user(
        &self,
        member_id: u64,
        policy: UnreachablePolicy,
    ) -> Box<Future<Item = IceUser, Error = TurnServiceErr>> {
        Box::new(self.send(CreateIceUser { member_id, policy }).then(
            |r: Result<Result<IceUser, TurnServiceErr>, MailboxError>| match r {
                Ok(Ok(ice)) => Ok(ice),
                Ok(Err(err)) => Err(err),
                Err(err) => Err(TurnServiceErr::from(err)),
            },
        ))
    }

    fn delete_user(
        &self,
        user: IceUser,
    ) -> Box<Future<Item = (), Error = TurnServiceErr>> {
        Box::new(self.send(DeleteIceUser(user)).then(
            |r: Result<Result<(), TurnServiceErr>, MailboxError>| match r {
                Ok(Err(err)) => Err(err),
                Err(err) => Err(TurnServiceErr::from(err)),
                _ => Ok(()),
            },
        ))
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`AuthService`].
type ActFuture<I, E> =
    Box<dyn ActorFuture<Actor = Service, Item = I, Error = E>>;

#[derive(Debug, Fail)]
pub enum TurnServiceErr {
    #[fail(display = "Error accessing TurnAuthRepo: {}", _0)]
    TurnAuthRepoErr(TurnDatabaseErr),
    #[fail(display = "Mailbox error when accessing TurnAuthRepo: {}", _0)]
    MailboxErr(MailboxError),
}

impl From<TurnDatabaseErr> for TurnServiceErr {
    fn from(err: TurnDatabaseErr) -> Self {
        TurnServiceErr::TurnAuthRepoErr(err)
    }
}

impl From<MailboxError> for TurnServiceErr {
    fn from(err: MailboxError) -> Self {
        TurnServiceErr::MailboxErr(err)
    }
}

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

/// [`TurnAuthService`] implementation backed by Redis database.
#[derive(Debug)]
struct Service {
    /// Turn credentials repository.
    turn_db: TurnDatabase,
    /// TurnAuthRepo password.
    db_pass: String,
    /// Turn server address.
    turn_address: SocketAddr,
    /// Turn server static user.
    turn_username: String,
    /// Turn server static user password.
    turn_password: String,
    /// Lazy static [`ICEUser`].
    static_user: Option<IceUser>,
}

/// Create new instance [`TurnAuthService`].
#[allow(clippy::module_name_repetitions)]
pub fn new_turn_auth_service(config: &Conf) -> Box<dyn TurnAuthService> {
    let service = Service {
        turn_db: TurnDatabase::new(config.turn.redis.addr().to_string()),
        db_pass: config.turn.redis.pass.clone(),
        turn_address: config.turn.addr(),
        turn_username: config.turn.user.clone(),
        turn_password: config.turn.pass.clone(),
        static_user: None,
    };

    Box::new(Arbiter::start(|_| service))
}

impl Service {
    /// Generates random alphanumeric string of specified length.
    fn new_password(&self, n: usize) -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(n)
            .collect()
    }

    /// Returns [`ICEUser`] with static credentials.
    fn static_user(&mut self) -> IceUser {
        if self.static_user.is_none() {
            self.static_user.replace(IceUser {
                address: self.turn_address,
                name: self.turn_username.clone(),
                pass: self.turn_password.clone(),
            });
        };

        self.static_user.clone().unwrap()
    }
}

impl Actor for Service {
    type Context = Context<Self>;

    // Init TurnAuthRepo establishing and authenticating connection.
    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.wait(self.turn_db.init(&self.db_pass).into_actor(self))
    }
}

/// Request for delete [`ICEUser`] for [`Member`] from COTURN database.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), TurnServiceErr>")]
struct DeleteIceUser(pub IceUser);

impl Handler<DeleteIceUser> for Service {
    type Result = ActFuture<(), TurnServiceErr>;

    /// Deletes [`ICEUser`] for given [`Member`].
    fn handle(
        &mut self,
        msg: DeleteIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        Box::new(wrap_future(self.turn_db.remove(&msg.0).map_err(Into::into)))
    }
}

/// Creates credentials on Turn server for specified member.
#[derive(Debug, Message)]
#[rtype(result = "Result<IceUser, TurnServiceErr>")]
struct CreateIceUser {
    pub member_id: MemberId,
    pub policy: UnreachablePolicy,
}

impl Handler<CreateIceUser> for Service {
    type Result = ActFuture<IceUser, TurnServiceErr>;

    /// Create and registers [`ICEUser`] for given [`Member`].
    fn handle(
        &mut self,
        msg: CreateIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let ice_user = IceUser {
            address: self.turn_address,
            name: msg.member_id.to_string(),
            pass: self.new_password(TURN_PASS_LEN),
        };

        Box::new(self.turn_db.insert(&ice_user).into_actor(self).then(
            move |result, act, _| {
                wrap_future(match result {
                    Ok(_) => ok(ice_user),
                    Err(e) => match msg.policy {
                        UnreachablePolicy::ReturnErr => err(e.into()),
                        UnreachablePolicy::ReturnStatic => {
                            ok(act.static_user())
                        }
                    },
                })
            },
        ))
    }
}

#[cfg(test)]
pub mod test {
    use futures::future;

    use crate::media::IceUser;

    use super::*;

    #[derive(Debug)]
    struct TurnAuthServiceMock {}

    impl TurnAuthService for TurnAuthServiceMock {
        fn create_user(
            &self,
            _member_id: u64,
            _policy: UnreachablePolicy,
        ) -> Box<Future<Item = IceUser, Error = TurnServiceErr>> {
            Box::new(future::ok(IceUser {
                address: "5.5.5.5:1234".parse().unwrap(),
                name: "username".to_string(),
                pass: "password".to_string(),
            }))
        }

        fn delete_user(
            &self,
            _user: IceUser,
        ) -> Box<Future<Item = (), Error = TurnServiceErr>> {
            Box::new(future::ok(()))
        }
    }

    #[allow(clippy::module_name_repetitions)]
    pub fn new_turn_auth_service_mock() -> Box<dyn TurnAuthService> {
        Box::new(TurnAuthServiceMock {})
    }

}
