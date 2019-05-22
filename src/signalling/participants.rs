//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.

use std::time::{Duration, Instant};

use actix::{
    fut::wrap_future, ActorFuture, Addr, AsyncContext, Context, MailboxError,
    SpawnHandle,
};
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
    signalling::{
        room::{ActFuture, CloseRoom, RoomError},
        Room,
    },
    turn::{
        CreateIceUser, DeleteIceUser, TurnAuthService, TurnServiceErr,
        UnreachablePolicy,
    },
};

#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum ParticipantServiceErr {
    TurnServiceErr(TurnServiceErr),
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
    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

    /// Service for managing authorization on Turn server.
    turn: Addr<TurnAuthService>,

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
        members: HashMap<MemberId, Member>,
        turn: Addr<TurnAuthService>,
        reconnect_timeout: Duration,
    ) -> Self {
        Self {
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
            self.connections.insert(member_id, con);

            Box::new(
                wrap_future(self.turn.send(CreateIceUser {
                    member_id,
                    policy: UnreachablePolicy::default(),
                }))
                .map_err(|err, _: &mut Room, _| {
                    ParticipantServiceErr::from(err)
                })
                .and_then(move |res, room, _| {
                    wrap_future(match res {
                        Ok(ice) => {
                            if let Some(mut member) =
                                room.participants.take_member(member_id)
                            {
                                member.ice_user.replace(ice);
                                room.participants.insert_member(member);
                            };
                            future::ok(())
                        }
                        Err(err) => {
                            future::err(ParticipantServiceErr::from(err))
                        }
                    })
                }),
            )
        }
    }

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If [`ClosedReason::Lost`], then creates delayed task that emits
    /// [`ClosedReason::Closed`].
    // TODO: Dont close the room. It is being closed atm, because we have
    //      no way to handle absence of RtcPeerConnection.
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
                self.delete_ice_user(member_id);
                ctx.notify(CloseRoom {})
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
    fn delete_ice_user(&mut self, member_id: MemberId) {
        if let Some(mut member) = self.members.remove(&member_id) {
            if let Some(ice_user) = member.ice_user.take() {
                self.turn.do_send(DeleteIceUser(ice_user));
            }
            self.members.insert(member_id, member);
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s,
    /// **does not** clears Turn credentials.
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> impl Future<Item = (), Error = ()> {
        // canceling all drop_connection_tasks
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

        // closing all RpcConnection's
        let close_fut = self.connections.drain().fold(
            vec![],
            |mut futures, (_, mut connection)| {
                futures.push(connection.close());
                futures
            },
        );

        join_all(close_fut).map(|_| ())
    }
}
