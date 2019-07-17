//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.

use std::time::{Duration, Instant};

use actix::{
    fut::wrap_future, ActorFuture, AsyncContext, Context, MailboxError,
    SpawnHandle,
};
use failure::Fail;
use futures::{
    future::{self, join_all, Either},
    Future,
};
use hashbrown::HashMap;

use medea_client_api_proto::Event;

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, ClosedReason, EventMessage, RpcConnection,
            RpcConnectionClosed,
        },
        control::{Member, MemberId},
    },
    log::prelude::*,
    media::IceUser,
    signalling::{
        room::{ActFuture, RoomError},
        Room, RoomId,
    },
    turn::{TurnAuthService, TurnServiceErr, UnreachablePolicy},
};

#[derive(Fail, Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum ParticipantServiceErr {
    #[fail(display = "TurnService Error in ParticipantService: {}", _0)]
    TurnServiceErr(TurnServiceErr),
    #[fail(
        display = "Mailbox error when accessing ParticipantService: {}",
        _0
    )]
    MailBoxErr(MailboxError),
}

impl From<TurnServiceErr> for ParticipantServiceErr {
    fn from(err: TurnServiceErr) -> Self {
        ParticipantServiceErr::TurnServiceErr(err)
    }
}

impl From<MailboxError> for ParticipantServiceErr {
    fn from(err: MailboxError) -> Self {
        ParticipantServiceErr::MailBoxErr(err)
    }
}

/// Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
/// stores [`Members`] and associated [`RpcConnection`]s, handles
/// [`RpcConnection`] authorization, establishment, message sending.
#[derive(Debug)]
pub struct ParticipantService {
    /// [`Room`]s id from which this [`ParticipantService`] was created.
    room_id: RoomId,

    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

    /// Service for managing authorization on Turn server.
    turn: Box<dyn TurnAuthService>,

    /// Established [`RpcConnection`]s of [`Member`]s in this [`Room`].
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    connections: HashMap<MemberId, Box<dyn RpcConnection>>,

    /// Timeout for close [`RpcConnection`] after receiving
    /// [`RpcConnectionClosed`] message.
    reconnect_timeout: Duration,

    /// Stores [`RpcConnection`] drop tasks.
    /// If [`RpcConnection`] is lost, [`Room`] waits for connection_timeout
    /// before dropping it irrevocably in case it gets reestablished.
    drop_connection_tasks: HashMap<MemberId, SpawnHandle>,
}

impl ParticipantService {
    pub fn new(
        room_id: RoomId,
        members: HashMap<MemberId, Member>,
        turn: Box<dyn TurnAuthService>,
        reconnect_timeout: Duration,
    ) -> Self {
        Self {
            room_id,
            members,
            turn,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
        }
    }

    /// Lookup [`Member`] by provided id and credentials. Returns
    /// [`Err(AuthorizationError::MemberNotExists)`] if lookup by [`MemberId`]
    /// failed. Returns [`Err(AuthorizationError::InvalidCredentials)`] if
    /// [`Member`] was found, but incorrect credentials was provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: MemberId,
        credentials: &str,
    ) -> Result<&Member, AuthorizationError> {
        match self.members.get(&member_id) {
            Some(ref member) => {
                if member.credentials.eq(credentials) {
                    Ok(member)
                } else {
                    Err(AuthorizationError::InvalidCredentials)
                }
            }
            None => Err(AuthorizationError::MemberNotExists),
        }
    }

    pub fn get_member(&self, member_id: MemberId) -> Option<&Member> {
        self.members.get(&member_id)
    }

    pub fn take_member(&mut self, member_id: MemberId) -> Option<Member> {
        self.members.remove(&member_id)
    }

    pub fn insert_member(&mut self, member: Member) {
        self.members.insert(member.id, member);
    }

    /// Checks if [`Member`] has **active** [`RcpConnection`].
    pub fn member_has_connection(&self, member_id: MemberId) -> bool {
        self.connections.contains_key(&member_id)
            && !self.drop_connection_tasks.contains_key(&member_id)
    }

    /// Send [`Event`] to specified remote [`Member`].
    pub fn send_event_to_member(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        match self.connections.get(&member_id) {
            Some(conn) => Either::A(
                conn.send_event(EventMessage::from(event))
                    .map_err(move |_| RoomError::UnableToSendEvent(member_id)),
            ),
            None => Either::B(future::err(RoomError::ConnectionNotExists(
                member_id,
            ))),
        }
    }

    /// Saves provided [`RpcConnection`], registers [`ICEUser`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) -> ActFuture<(), ParticipantServiceErr> {
        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for member {}", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            Box::new(wrap_future(connection.close().then(|_| Ok(()))))
        } else {
            Box::new(
                wrap_future(self.turn.create(
                    member_id,
                    self.room_id,
                    UnreachablePolicy::ReturnErr,
                ))
                .map_err(|err, _: &mut Room, _| {
                    ParticipantServiceErr::from(err)
                })
                .and_then(
                    move |ice: IceUser, room: &mut Room, _| {
                        if let Some(mut member) =
                            room.participants.take_member(member_id)
                        {
                            member.ice_user.replace(ice);
                            room.participants.insert_member(member);
                            room.participants
                                .connections
                                .insert(member_id, con);
                        };
                        wrap_future(future::ok(()))
                    },
                ),
            )
        }
    }

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If [`ClosedReason::Lost`], then creates delayed task that emits
    /// [`ClosedReason::Closed`].
    // TODO: Don't close the room. It is being closed atm, because we have
    //       no way to handle absence of RpcConnection.
    pub fn connection_closed(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        reason: &ClosedReason,
    ) {
        let closed_at = Instant::now();
        match reason {
            ClosedReason::Closed => {
                self.connections.remove(&member_id);
                ctx.spawn(wrap_future(
                    self.delete_ice_user(member_id).map_err(|err| {
                        error!("Error deleting IceUser {:?}", err)
                    }),
                ));
            }
            ClosedReason::Lost => {
                self.drop_connection_tasks.insert(
                    member_id,
                    ctx.run_later(self.reconnect_timeout, move |_, ctx| {
                        info!(
                            "Member {} connection lost at {:?}. Room will be \
                             stopped.",
                            member_id, closed_at
                        );
                        ctx.notify(RpcConnectionClosed {
                            member_id,
                            reason: ClosedReason::Closed,
                        })
                    }),
                );
            }
        }
    }

    /// Deletes [`IceUser`] associated with provided [`Member`].
    fn delete_ice_user(
        &mut self,
        member_id: MemberId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
        match self.members.remove(&member_id) {
            Some(mut member) => {
                let delete_fut = match member.ice_user.take() {
                    Some(ice_user) => self.turn.delete(vec![ice_user]),
                    None => Box::new(future::ok(())),
                };
                self.members.insert(member_id, member);

                delete_fut
            }
            None => Box::new(future::ok(())),
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s and
    /// deletes all [`IceUser`]s.
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> impl Future<Item = (), Error = ()> {
        // canceling all drop_connection_tasks
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

        // closing all RpcConnection's
        let mut close_fut = self.connections.drain().fold(
            vec![],
            |mut futures, (_, mut connection)| {
                futures.push(connection.close());
                futures
            },
        );

        // deleting all IceUsers
        let remove_ice_users = Box::new({
            let mut room_users = Vec::with_capacity(self.members.len());

            self.members.iter_mut().for_each(|(_, data)| {
                if let Some(ice_user) = data.ice_user.take() {
                    room_users.push(ice_user);
                }
            });
            self.turn
                .delete(room_users)
                .map_err(|err| error!("Error removing IceUsers {:?}", err))
        });
        close_fut.push(remove_ice_users);

        join_all(close_fut).map(|_| ())
    }
}
