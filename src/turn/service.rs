//! Implementation of managing [coturn] [TURN] server.
//!
//! [coturn]: https://github.com/coturn/coturn
//! [TURN]: https://webrtcglossary.com/turn/

use std::{fmt, sync::Arc};

use actix::{
    fut, Actor, ActorFuture, Addr, Context, Handler, MailboxError, Message,
    ResponseFuture, WrapFuture as _,
};
use derive_more::{Display, From};
use failure::Fail;
use futures::future::{
    self, FutureExt as _, LocalBoxFuture, TryFutureExt as _,
};
use rand::{distributions::Alphanumeric, Rng};
use redis::ConnectionInfo;

use crate::{
    api::control::{MemberId, RoomId},
    conf,
    media::IceUser,
    turn::repo::{TurnDatabase, TurnDatabaseErr},
};

static TURN_PASS_LEN: usize = 16;

/// Manages Turn server credentials.
pub trait TurnAuthService: fmt::Debug + Send + Sync {
    /// Generates and registers Turn credentials.
    fn create(
        &self,
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> LocalBoxFuture<'static, Result<IceUser, TurnServiceErr>>;

    /// Deletes batch of [`IceUser`]s.
    fn delete(
        &self,
        users: Vec<IceUser>,
    ) -> LocalBoxFuture<'static, Result<(), TurnServiceErr>>;
}

impl TurnAuthService for Addr<Service> {
    /// Sends [`CreateIceUser`] to [`Service`].
    fn create(
        &self,
        member_id: MemberId,
        room_id: RoomId,
        policy: UnreachablePolicy,
    ) -> LocalBoxFuture<'static, Result<IceUser, TurnServiceErr>> {
        let creating = self.send(CreateIceUser {
            member_id,
            room_id,
            policy,
        });
        async {
            match creating.await {
                Ok(Ok(ice)) => Ok(ice),
                Ok(Err(err)) => Err(err),
                Err(err) => Err(err.into()),
            }
        }
        .boxed_local()
    }

    /// Sends `DeleteRoom` to [`Service`].
    fn delete(
        &self,
        users: Vec<IceUser>,
    ) -> LocalBoxFuture<'static, Result<(), TurnServiceErr>> {
        // leave only non static users
        let users: Vec<IceUser> =
            users.into_iter().filter(|u| !u.is_static()).collect();

        if users.is_empty() {
            future::ok(()).boxed_local()
        } else {
            let deleting = self.send(DeleteIceUsers(users));
            async {
                match deleting.await {
                    Ok(Err(err)) => Err(err),
                    Err(err) => Err(err.into()),
                    _ => Ok(()),
                }
            }
            .boxed_local()
        }
    }
}

/// Ergonomic type alias for using [`ActorFuture`] for [`AuthService`].
type ActFuture<T> = Box<dyn ActorFuture<Actor = Service, Output = T>>;

/// Error which can happen in [`TurnAuthService`].
#[derive(Display, Debug, Fail, From)]
pub enum TurnServiceErr {
    #[display(fmt = "Error accessing TurnAuthRepo: {}", _0)]
    TurnAuthRepoErr(TurnDatabaseErr),

    #[display(fmt = "Mailbox error when accessing TurnAuthRepo: {}", _0)]
    MailboxErr(MailboxError),

    #[display(fmt = "Timeout exceeded while trying to insert/delete IceUser")]
    #[from(ignore)]
    TimedOut,
}

/// Defines [`TurnAuthService`] behaviour if remote database is unreachable
#[derive(Debug)]
pub enum UnreachablePolicy {
    /// Error will be propagated if request to db fails cause it is
    /// unreachable.
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
    turn_address: String,

    /// Turn server static user.
    turn_username: String,

    /// Turn server static user password.
    turn_password: String,

    /// Lazy static [`ICEUser`].
    static_user: Option<IceUser>,
}

/// Create new instance [`TurnAuthService`].
pub fn new_turn_auth_service<'a>(
    cf: &conf::Turn,
) -> Result<Arc<dyn TurnAuthService + 'a>, TurnServiceErr> {
    let turn_db = TurnDatabase::new(
        cf.db.redis.connection_timeout,
        ConnectionInfo {
            addr: Box::new(redis::ConnectionAddr::Tcp(
                cf.db.redis.ip.to_string(),
                cf.db.redis.port,
            )),
            db: cf.db.redis.db_number,
            passwd: if cf.db.redis.pass.is_empty() {
                None
            } else {
                Some(cf.db.redis.pass.clone())
            },
        },
    )?;

    let turn_service = Service {
        turn_db,
        db_pass: cf.db.redis.pass.clone(),
        turn_address: cf.addr(),
        turn_username: cf.user.clone(),
        turn_password: cf.pass.clone(),
        static_user: None,
    };

    Ok(Arc::new(turn_service.start()))
}

impl Service {
    /// Generates random alphanumeric string of specified length.
    fn generate_pass(n: usize) -> String {
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(n)
            .collect()
    }

    /// Returns [`ICEUser`] with static credentials.
    fn static_user(&mut self) -> IceUser {
        if self.static_user.is_none() {
            self.static_user.replace(IceUser::new(
                self.turn_address.clone(),
                self.turn_username.clone(),
                self.turn_password.clone(),
            ));
        };

        self.static_user.clone().unwrap()
    }
}

impl Actor for Service {
    type Context = Context<Self>;
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
    type Result = ActFuture<Result<IceUser, TurnServiceErr>>;

    /// Generates [`IceUser`] with saved Turn address, provided [`MemberId`] and
    /// random password. Inserts created [`IceUser`] into [`TurnDatabase`].
    fn handle(
        &mut self,
        msg: CreateIceUser,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        let ice_user = IceUser::build(
            self.turn_address.clone(),
            &msg.room_id,
            &msg.member_id.to_string(),
            Self::generate_pass(TURN_PASS_LEN),
        );

        Box::new(self.turn_db.insert(&ice_user).into_actor(self).then(
            move |result, this, _| match result {
                Ok(_) => fut::ok(ice_user),
                Err(err) => match msg.policy {
                    UnreachablePolicy::ReturnErr => fut::err(err.into()),
                    UnreachablePolicy::ReturnStatic => {
                        fut::ok(this.static_user())
                    }
                },
            },
        ))
    }
}

/// Deletes all users from given room in redis.
#[derive(Debug, Message)]
#[rtype(result = "Result<(), TurnServiceErr>")]
struct DeleteIceUsers(Vec<IceUser>);

impl Handler<DeleteIceUsers> for Service {
    type Result = ResponseFuture<Result<(), TurnServiceErr>>;

    /// Deletes all users with provided [`RoomId`]
    fn handle(
        &mut self,
        msg: DeleteIceUsers,
        _ctx: &mut Self::Context,
    ) -> Self::Result {
        self.turn_db.remove(&msg.0).err_into().boxed_local()
    }
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::media::IceUser;

    use super::*;

    #[derive(Debug)]
    struct TurnAuthServiceMock {}

    impl TurnAuthService for TurnAuthServiceMock {
        fn create(
            &self,
            _: MemberId,
            _: RoomId,
            _: UnreachablePolicy,
        ) -> LocalBoxFuture<'static, Result<IceUser, TurnServiceErr>> {
            async {
                Ok(IceUser::new(
                    "5.5.5.5:1234".parse().unwrap(),
                    "username".into(),
                    "password".into(),
                ))
            }
            .boxed_local()
        }

        fn delete(
            &self,
            _: Vec<IceUser>,
        ) -> LocalBoxFuture<'static, Result<(), TurnServiceErr>> {
            future::ok(()).boxed_local()
        }
    }

    pub fn new_turn_auth_service_mock() -> Arc<dyn TurnAuthService> {
        Arc::new(TurnAuthServiceMock {})
    }
}
