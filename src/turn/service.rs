use core::fmt;
use std::net::SocketAddr;

use actix::{
    fut::wrap_future, Actor, ActorFuture, Addr, Arbiter, Context, Handler,
    MailboxError, Message, WrapFuture,
};
use bb8::RunError;
use failure::Fail;
use futures::future::{err, ok, Future};
use rand::{distributions::Alphanumeric, Rng};
use redis::ConnectionInfo;
use smart_default::*;

use crate::turn::repo::TurnDatabaseInsertableUser;
use crate::{
    api::control::MemberId,
    conf::Conf,
    media::IceUser,
    signalling::RoomId,
    turn::repo::{TurnDatabase, TurnDatabaseErr},
};

static TURN_PASS_LEN: usize = 16;

#[allow(clippy::module_name_repetitions)]
/// Manages Turn server credentials.
pub trait TurnAuthService: fmt::Debug + Send {
    /// Generates and registers Turn credentials.
    fn create(
        &self,
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> Box<dyn Future<Item = IceUser, Error = TurnServiceErr>>;

    /// Deletes provided Turn credentials.
    fn delete(
        &self,
        user: IceUser,
        room_id: RoomId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>>;

    /// Deletes [`users`] from redis with provided [`RoomId`].
    fn delete_batch(
        &self,
        room_id: RoomId,
        users: Vec<u64>,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>>;
}

impl TurnAuthService for Addr<Service> {
    /// Sends [`CreateIceUser`] to [`Service`].
    fn create(
        &self,
        member_id: u64,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> Box<Future<Item = IceUser, Error = TurnServiceErr>> {
        Box::new(
            self.send(CreateIceUser {
                member_id,
                room_id,
                policy,
            })
            .then(
                |r: Result<Result<IceUser, TurnServiceErr>, MailboxError>| {
                    match r {
                        Ok(Ok(ice)) => Ok(ice),
                        Ok(Err(err)) => Err(err),
                        Err(err) => Err(TurnServiceErr::from(err)),
                    }
                },
            ),
        )
    }

    /// Sends [`DeleteIceUser`] to [`Service`].
    fn delete(
        &self,
        user: IceUser,
        room_id: RoomId,
    ) -> Box<Future<Item = (), Error = TurnServiceErr>> {
        let delete_user = TurnDatabaseInsertableUser {
            ice_user: user,
            room_id,
        };
        Box::new(self.send(DeleteIceUser(delete_user)).then(
            |r: Result<Result<(), TurnServiceErr>, MailboxError>| match r {
                Ok(Err(err)) => Err(err),
                Err(err) => Err(TurnServiceErr::from(err)),
                _ => Ok(()),
            },
        ))
    }

    /// Sends [`DeleteRoom`] to [`Service`].
    fn delete_batch(
        &self,
        room_id: RoomId,
        users: Vec<MemberId>,
    ) -> Box<Future<Item = (), Error = TurnServiceErr>> {
        Box::new(self.send(DeleteMultipleUsers { room_id, users }).then(
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
    #[fail(display = "Timeout exceeded while trying to insert/delete IceUser")]
    TimedOut,
}

impl From<TurnDatabaseErr> for TurnServiceErr {
    fn from(err: TurnDatabaseErr) -> Self {
        TurnServiceErr::TurnAuthRepoErr(err)
    }
}

impl From<bb8::RunError<TurnDatabaseErr>> for TurnServiceErr {
    fn from(err: bb8::RunError<TurnDatabaseErr>) -> Self {
        match err {
            RunError::User(error) => TurnServiceErr::TurnAuthRepoErr(error),
            RunError::TimedOut => TurnServiceErr::TimedOut,
        }
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
pub fn new_turn_auth_service(
    config: &Conf,
) -> Result<Box<dyn TurnAuthService>, TurnServiceErr> {
    let turn_db = TurnDatabase::new(ConnectionInfo {
        addr: Box::new(redis::ConnectionAddr::Tcp(
            config.turn.db.redis.ip.to_string(),
            config.turn.db.redis.port,
        )),
        db: config.turn.db.redis.db_number,
        passwd: if config.turn.db.redis.pass.is_empty() {
            None
        } else {
            Some(config.turn.db.redis.pass.clone())
        },
    })?;

    let service = Service {
        turn_db,
        db_pass: config.turn.db.redis.pass.clone(),
        turn_address: config.turn.addr(),
        turn_username: config.turn.user.clone(),
        turn_password: config.turn.db.redis.pass.clone(),
        static_user: None,
    };

    Ok(Box::new(Arbiter::start(|_| service)))
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
}

/// Request for delete [`ICEUser`] for [`Member`] from Turn database.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), TurnServiceErr>")]
struct DeleteIceUser(pub TurnDatabaseInsertableUser);

impl Handler<DeleteIceUser> for Service {
    type Result = Box<dyn Future<Item = (), Error = TurnServiceErr>>;

    /// Deletes provided [`TurnDatabaseInsertableUser`] from [`TurnDatabase`].
    fn handle(
        &mut self,
        msg: DeleteIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        Box::new(self.turn_db.remove(&msg.0).map_err(TurnServiceErr::from))
    }
}

/// Creates credentials on Turn server for specified member.
#[derive(Debug, Message)]
#[rtype(result = "Result<IceUser, TurnServiceErr>")]
struct CreateIceUser {
    pub member_id: MemberId,
    pub room_id: RoomId,
    pub policy: UnreachablePolicy,
}

impl Handler<CreateIceUser> for Service {
    type Result = ActFuture<IceUser, TurnServiceErr>;

    /// Generates [`IceUser`] with saved Turn address, provided [`MemberId`] and
    /// random password. Inserts created [`IceUser`] into [`TurnDatabase`].
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

        let turn_db_user = TurnDatabaseInsertableUser {
            ice_user,
            room_id: msg.room_id,
        };

        Box::new(self.turn_db.insert(&turn_db_user).into_actor(self).then(
            move |result, act, _| {
                wrap_future(match result {
                    Ok(_) => ok(turn_db_user.ice_user),
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

/// Deletes all users from given room in redis.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), TurnServiceErr>")]
struct DeleteMultipleUsers {
    room_id: RoomId,
    users: Vec<u64>,
}

impl Handler<DeleteMultipleUsers> for Service {
    type Result = ActFuture<(), TurnServiceErr>;

    /// Deletes all users with provided [`RoomId`]
    fn handle(
        &mut self,
        msg: DeleteMultipleUsers,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        Box::new(
            self.turn_db
                .remove_users(msg.room_id, &msg.users)
                .map_err(TurnServiceErr::from)
                .into_actor(self),
        )
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
        fn create(
            &self,
            _: u64,
            _: RoomId,
            _: UnreachablePolicy,
        ) -> Box<Future<Item = IceUser, Error = TurnServiceErr>> {
            Box::new(future::ok(IceUser {
                address: "5.5.5.5:1234".parse().unwrap(),
                name: "username".to_string(),
                pass: "password".to_string(),
            }))
        }

        fn delete(
            &self,
            _: IceUser,
            _: RoomId,
        ) -> Box<Future<Item = (), Error = TurnServiceErr>> {
            Box::new(future::ok(()))
        }

        fn delete_batch(
            &self,
            _: RoomId,
            _: Vec<u64>,
        ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
            Box::new(future::ok(()))
        }
    }

    #[allow(clippy::module_name_repetitions)]
    pub fn new_turn_auth_service_mock() -> Box<dyn TurnAuthService> {
        Box::new(TurnAuthServiceMock {})
    }

}
