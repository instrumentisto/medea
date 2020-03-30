//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Member`]s and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.
//!
//! [`Member`]: crate::signalling::elements::member::Member
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
//! [`ParticipantService`]: crate::signalling::participants::ParticipantService

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use actix::{
    fut::wrap_future, AsyncContext, Context, ContextFutureSpawner as _,
    SpawnHandle,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{
    self, FutureExt as _, LocalBoxFuture, TryFutureExt as _,
};
use medea_client_api_proto::{CloseDescription, CloseReason, Event};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, ClosedReason, RpcConnection,
            RpcConnectionClosed,
        },
        control::{
            refs::{Fid, ToEndpoint, ToMember},
            MemberId, RoomId, RoomSpec,
        },
    },
    log::prelude::*,
    signalling::{
        elements::{
            member::MemberError, parse_members, Member, MembersLoadError,
        },
        room::{ActFuture, RoomError},
        Room,
    },
    AppContext,
};

#[derive(Debug, Display, Fail)]
pub enum ParticipantServiceErr {
    /// [`Member`] with provided [`Fid`] not found.
    #[display(fmt = "Participant [id = {}] not found", _0)]
    ParticipantNotFound(Fid<ToMember>),

    /// [`Endpoint`] with provided URI not found.
    ///
    /// [`Endpoint`]: crate::signalling::elements::endpoints::Endpoint
    #[display(fmt = "Endpoint [id = {}] not found.", _0)]
    EndpointNotFound(Fid<ToEndpoint>),

    /// Some error happened in [`Member`].
    MemberError(MemberError),
}

impl From<MemberError> for ParticipantServiceErr {
    fn from(err: MemberError) -> Self {
        Self::MemberError(err)
    }
}

/// Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
/// stores [`Member`]s and associated [`RpcConnection`]s, handles
/// [`RpcConnection`] authorization, establishment, message sending.
#[derive(Debug)]
pub struct ParticipantService {
    /// [`Room`]s id from which this [`ParticipantService`] was created.
    room_id: RoomId,

    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

    /// Established [`RpcConnection`]s of [`Member`]s in this [`Room`].
    ///
    /// [`Member`]: crate::signalling::elements::member::Member
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    connections: HashMap<MemberId, Box<dyn RpcConnection>>,

    /// Stores [`RpcConnection`] drop tasks.
    /// If [`RpcConnection`] is lost, [`Room`] waits for `connect_timeout`
    /// before dropping it irrevocably in case it gets reestablished.
    drop_connection_tasks: HashMap<MemberId, SpawnHandle>,

    /// Duration, after which the server deletes the client session if
    /// the remote RPC client does not reconnect after it is idle.
    rpc_reconnect_timeout: Duration,
}

impl ParticipantService {
    /// Creates new [`ParticipantService`] from [`RoomSpec`].
    ///
    /// # Errors
    ///
    /// Errors with [`MemberLoadError`] if [`RoomSpec`] transformation fails.
    pub fn new(
        room_spec: &RoomSpec,
        context: &AppContext,
    ) -> Result<Self, MembersLoadError> {
        Ok(Self {
            room_id: room_spec.id().clone(),
            members: parse_members(room_spec)?,
            connections: HashMap::new(),
            drop_connection_tasks: HashMap::new(),
            rpc_reconnect_timeout: context.config.rpc.reconnect_timeout,
        })
    }

    /// Lookups [`Member`] by provided [`MemberId`].
    pub fn get_member_by_id(&self, id: &MemberId) -> Option<Member> {
        self.members.get(id).cloned()
    }

    /// Generates [`Fid`] which point to some [`Member`] in this
    /// [`ParticipantService`]'s [`Room`].
    ///
    /// __Note__ this function don't check presence of [`Member`] in
    /// [`ParticipantService`].
    pub fn get_fid_to_member(&self, member_id: MemberId) -> Fid<ToMember> {
        Fid::<ToMember>::new(self.room_id.clone(), member_id)
    }

    /// Lookups [`Member`] by [`MemberId`].
    ///
    /// # Errors
    ///
    /// Errors with [`ParticipantServiceErr::ParticipantNotFound`] if no
    /// [`Member`] was found.
    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Result<Member, ParticipantServiceErr> {
        self.members.get(id).cloned().map_or(
            Err(ParticipantServiceErr::ParticipantNotFound(
                self.get_fid_to_member(id.clone()),
            )),
            Ok,
        )
    }

    /// Returns all [`Member`] from this [`ParticipantService`].
    pub fn members(&self) -> HashMap<MemberId, Member> {
        self.members.clone()
    }

    /// Lookups [`Member`] by provided [`MemberId`] and credentials.
    ///
    /// # Errors
    ///
    /// Errors with [`AuthorizationError::MemberNotExists`] if lookup by
    /// [`MemberId`] fails.
    ///
    /// Errors with [`AuthorizationError::InvalidCredentials`] if [`Member`]
    /// was found, but incorrect credentials were provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &MemberId,
        credentials: &str,
    ) -> Result<Member, AuthorizationError> {
        let member = self
            .get_member_by_id(member_id)
            .ok_or(AuthorizationError::MemberNotExists)?;
        if member.credentials() == credentials {
            Ok(member)
        } else {
            Err(AuthorizationError::InvalidCredentials)
        }
    }

    /// Checks if [`Member`] has __active__ [`RpcConnection`].
    pub fn member_has_connection(&self, member_id: &MemberId) -> bool {
        self.connections.contains_key(member_id)
            && !self.drop_connection_tasks.contains_key(member_id)
    }

    /// Sends [`Event`] to specified remote [`Member`].
    pub fn send_event_to_member(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> LocalBoxFuture<'static, Result<(), RoomError>> {
        if let Some(conn) = self.connections.get(&member_id) {
            conn.send_event(event)
                .map_err(move |_| RoomError::UnableToSendEvent(member_id))
                .boxed_local()
        } else {
            future::err(RoomError::ConnectionNotExists(member_id)).boxed_local()
        }
    }

    /// Saves provided [`RpcConnection`], registers [`IceUser`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) -> ActFuture<Result<Member, ParticipantServiceErr>> {
        let member = match self.get_member_by_id(&member_id) {
            None => {
                return Box::new(wrap_future(future::err(
                    ParticipantServiceErr::ParticipantNotFound(
                        self.get_fid_to_member(member_id),
                    ),
                )));
            }
            Some(member) => member,
        };

        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for member [id = {}]", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            self.insert_connection(member_id, conn);
            Box::new(wrap_future(
                connection
                    .close(CloseDescription::new(CloseReason::Reconnected))
                    .map(move |_| Ok(member)),
            ))
        } else {
            self.insert_connection(member_id, conn);
            Box::new(wrap_future(future::ok(member)))
        }
    }

    /// Inserts new [`RpcConnection`] into this [`ParticipantService`].
    fn insert_connection(
        &mut self,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) {
        self.connections.insert(member_id, conn);
    }

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If [`ClosedReason::Lost`], then creates delayed task that emits
    /// [`ClosedReason::Closed`].
    pub fn connection_closed(
        &mut self,
        member_id: MemberId,
        reason: &ClosedReason,
        ctx: &mut Context<Room>,
    ) {
        let closed_at = Instant::now();
        match reason {
            ClosedReason::Closed { .. } => {
                debug!("Connection for member [id = {}] removed.", member_id);
                self.connections.remove(&member_id);
                // TODO: we have no way to handle absence of RpcConnection right
                //       now.
            }
            ClosedReason::Lost => {
                self.drop_connection_tasks.insert(
                    member_id.clone(),
                    ctx.run_later(self.rpc_reconnect_timeout, move |_, ctx| {
                        info!(
                            "Member [id = {}] connection lost at {:?}.",
                            member_id, closed_at,
                        );
                        ctx.notify(RpcConnectionClosed {
                            member_id,
                            reason: ClosedReason::Closed { normal: false },
                        })
                    }),
                );
            }
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s and
    /// deletes all [`IceUser`]s.
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> LocalBoxFuture<'static, ()> {
        // canceling all drop_connection_tasks
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

        // closing all RpcConnection's
        let close_rpc_connections =
            future::join_all(self.connections.drain().fold(
                vec![],
                |mut futs, (_, mut connection)| {
                    futs.push(
                        connection.close(CloseDescription::new(
                            CloseReason::Finished,
                        )),
                    );
                    futs
                },
            ));

        close_rpc_connections.map(|_| ()).boxed_local()
    }

    /// Deletes [`Member`] from [`ParticipantService`], removes this user from
    /// [`TurnAuthService`], closes RPC connection with him and removes drop
    /// connection task.
    ///
    /// [`TurnAuthService`]: crate::turn::service::TurnAuthService
    pub fn delete_member(
        &mut self,
        member_id: &MemberId,
        ctx: &mut Context<Room>,
    ) {
        if let Some(drop) = self.drop_connection_tasks.remove(member_id) {
            ctx.cancel_future(drop);
        }

        if let Some(mut conn) = self.connections.remove(member_id) {
            wrap_future::<_, Room>(
                conn.close(CloseDescription::new(CloseReason::Evicted)),
            )
            .spawn(ctx);
        }

        self.members.remove(member_id);
    }

    /// Inserts given [`Member`] into [`ParticipantService`].
    pub fn insert_member(&mut self, id: MemberId, member: Member) {
        self.members.insert(id, member);
    }

    /// Returns [`Iterator`] over [`MemberId`] and [`Member`] which this
    /// [`ParticipantRepository`] stores.
    pub fn iter_members(&self) -> impl Iterator<Item = (&MemberId, &Member)> {
        self.members.iter()
    }
}
