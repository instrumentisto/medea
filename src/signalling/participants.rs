//! [`Member`] is member with [`RpcConnection`]. [`MemberService`]
//! stores [`Member`]s and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

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
        control::{MemberId, RoomId, RoomSpec},
    },
    log::prelude::*,
    media::IceUser,
    signalling::{
        control::{parse_participants, Member, MembersLoadError},
        room::{ActFuture, RoomError},
        Room,
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
    #[fail(display = "Participant with Id [{}] was not found", _0)]
    ParticipantNotFound(MemberId),
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

/// [`Member`] is member of [`Room`] with [`RpcConnection`].
/// [`ParticipantService`] stores [`Member`]s and associated
/// [`RpcConnection`]s, handles [`RpcConnection`] authorization, establishment,
/// message sending.
#[derive(Debug)]
pub struct ParticipantService {
    /// [`Room`]s id from which this [`ParticipantService`] was created.
    room_id: RoomId,

    /// [`Member`]s which currently are present in this [`Room`].
    participants: HashMap<MemberId, Rc<Member>>,

    /// Service for managing authorization on Turn server.
    turn: Box<dyn TurnAuthService>,

    /// Established [`RpcConnection`]s of [`Members`]s in this [`Room`].
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
    /// Create new [`ParticipantService`] from [`RoomSpec`].
    pub fn new(
        room_spec: &RoomSpec,
        reconnect_timeout: Duration,
        turn: Box<dyn TurnAuthService>,
    ) -> Result<Self, MembersLoadError> {
        Ok(Self {
            room_id: room_spec.id().clone(),
            participants: parse_participants(room_spec)?,
            turn,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
        })
    }

    /// Lookup [`Member`] by provided id.
    pub fn get_participant_by_id(&self, id: &MemberId) -> Option<Rc<Member>> {
        self.participants.get(id).cloned()
    }

    /// Lookup [`Member`] by provided id and credentials. Returns
    /// [`Err(AuthorizationError::MemberNotExists)`] if lookup by
    /// [`MemberId`] failed. Returns
    /// [`Err(AuthorizationError::InvalidCredentials)`] if [`Member`]
    /// was found, but incorrect credentials was provided.
    pub fn get_participant_by_id_and_credentials(
        &self,
        participant_id: &MemberId,
        credentials: &str,
    ) -> Result<Rc<Member>, AuthorizationError> {
        match self.get_participant_by_id(participant_id) {
            Some(participant) => {
                if participant.credentials().eq(credentials) {
                    Ok(participant.clone())
                } else {
                    Err(AuthorizationError::InvalidCredentials)
                }
            }
            None => Err(AuthorizationError::MemberNotExists),
        }
    }

    /// Checks if [`Member`] has **active** [`RcpConnection`].
    pub fn participant_has_connection(
        &self,
        participant_id: &MemberId,
    ) -> bool {
        self.connections.contains_key(participant_id)
            && !self.drop_connection_tasks.contains_key(participant_id)
    }

    /// Send [`Event`] to specified remote [`Member`].
    pub fn send_event_to_participant(
        &mut self,
        participant_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        match self.connections.get(&participant_id) {
            Some(conn) => {
                Either::A(conn.send_event(EventMessage::from(event)).map_err(
                    move |_| RoomError::UnableToSendEvent(participant_id),
                ))
            }
            None => Either::B(future::err(RoomError::ConnectionNotExists(
                participant_id,
            ))),
        }
    }

    /// Saves provided [`RpcConnection`], registers [`ICEUser`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        participant_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) -> ActFuture<Rc<Member>, ParticipantServiceErr> {
        let participant = match self.get_participant_by_id(&participant_id) {
            None => {
                return Box::new(wrap_future(future::err(
                    ParticipantServiceErr::ParticipantNotFound(participant_id),
                )));
            }
            Some(participant) => participant,
        };

        // lookup previous participant connection
        if let Some(mut connection) = self.connections.remove(&participant_id) {
            debug!(
                "Closing old RpcConnection for participant {}",
                participant_id
            );

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) =
                self.drop_connection_tasks.remove(&participant_id)
            {
                ctx.cancel_future(handler);
            }
            Box::new(wrap_future(
                connection.close().then(move |_| Ok(participant)),
            ))
        } else {
            Box::new(
                wrap_future(self.turn.create(
                    participant_id.clone(),
                    self.room_id.clone(),
                    UnreachablePolicy::ReturnErr,
                ))
                .map_err(|err, _: &mut Room, _| {
                    ParticipantServiceErr::from(err)
                })
                .and_then(
                    move |ice: IceUser, room: &mut Room, _| {
                        room.participants
                            .insert_connection(participant_id.clone(), con);
                        participant.replace_ice_user(ice);

                        wrap_future(future::ok(participant))
                    },
                ),
            )
        }
    }

    /// Insert new [`RpcConnection`] into this [`ParticipantService`].
    fn insert_connection(
        &mut self,
        participant_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) {
        self.connections.insert(participant_id, conn);
    }

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the
    /// room. If [`ClosedReason::Lost`], then creates delayed task that
    /// emits [`ClosedReason::Closed`].
    // TODO: Dont close the room. It is being closed atm, because we have
    //      no way to handle absence of RpcConnection.
    pub fn connection_closed(
        &mut self,
        ctx: &mut Context<Room>,
        participant_id: MemberId,
        reason: &ClosedReason,
    ) {
        let closed_at = Instant::now();
        match reason {
            ClosedReason::Closed => {
                self.connections.remove(&participant_id);

                ctx.spawn(wrap_future(
                    self.delete_ice_user(&participant_id).map_err(|err| {
                        error!("Error deleting IceUser {:?}", err)
                    }),
                ));
                // ctx.notify(CloseRoom {})
            }
            ClosedReason::Lost => {
                self.drop_connection_tasks.insert(
                    participant_id.clone(),
                    ctx.run_later(self.reconnect_timeout, move |_, ctx| {
                        info!(
                            "Member {} connection lost at {:?}. Room will be \
                             stopped.",
                            &participant_id, closed_at
                        );
                        ctx.notify(RpcConnectionClosed {
                            member_id: participant_id,
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
        participant_id: &MemberId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
        match self.get_participant_by_id(&participant_id) {
            Some(participant) => match participant.take_ice_user() {
                Some(ice_user) => self.turn.delete(vec![ice_user]),
                None => Box::new(future::ok(())),
            },
            None => Box::new(future::ok(())),
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s,
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
            let mut room_users = Vec::with_capacity(self.participants.len());

            self.participants.iter().for_each(|(_, data)| {
                if let Some(ice_user) = data.take_ice_user() {
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
