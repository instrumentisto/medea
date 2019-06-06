//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending.

use std::{
    convert::TryFrom,
    time::{Duration, Instant},
};

use actix::{fut::wrap_future, AsyncContext, Context, SpawnHandle};
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
        control::{
            Member, MemberId, MemberSpec, RoomSpec, TryFromElementError,
        },
    },
    log::prelude::*,
    signalling::{
        room::{CloseRoom, RoomError},
        Room,
    },
};

/// Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
/// stores [`Members`] and associated [`RpcConnection`]s, handles
/// [`RpcConnection`] authorization, establishment, message sending.
#[derive(Debug)]
pub struct ParticipantService {
    /// [`Member`]s which currently are present in this [`Room`].
    members: HashMap<MemberId, Member>,

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
    /// Create new [`ParticipantService`] from [`RoomSpec`].
    pub fn new(
        room_spec: &RoomSpec,
        reconnect_timeout: Duration,
    ) -> Result<Self, TryFromElementError> {
        let mut members = HashMap::new();
        for (control_id, element) in &room_spec.spec.pipeline {
            let member_spec = MemberSpec::try_from(element.clone())?;
            let member_id = MemberId(control_id.clone());

            members.insert(
                member_id.clone(),
                Member::new(member_id, member_spec, room_spec)?,
            );
        }

        debug!("Created room with {:?} members.", members);

        Ok(Self {
            members,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
        })
    }

    /// Lookup [`Member`] by provided id.
    pub fn get_member_by_id(&self, id: &MemberId) -> Option<&Member> {
        self.members.get(id)
    }

    /// Lookup [`Member`] by provided id and credentials. Returns
    /// [`Err(AuthorizationError::MemberNotExists)`] if lookup by [`MemberId`]
    /// failed. Returns [`Err(AuthorizationError::InvalidCredentials)`] if
    /// [`Member`] was found, but incorrect credentials was provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &MemberId,
        credentials: &str,
    ) -> Result<&Member, AuthorizationError> {
        match self.members.get(member_id) {
            Some(ref member) => {
                if member.credentials().eq(credentials) {
                    Ok(member)
                } else {
                    Err(AuthorizationError::InvalidCredentials)
                }
            }
            None => Err(AuthorizationError::MemberNotExists),
        }
    }

    /// Checks if [`Member`] has **active** [`RcpConnection`].
    pub fn member_has_connection(&self, member_id: &MemberId) -> bool {
        self.connections.contains_key(member_id)
            && !self.drop_connection_tasks.contains_key(member_id)
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

    /// If [`ClosedReason::Closed`], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If [`ClosedReason::Lost`], then creates delayed task that emits
    /// [`ClosedReason::Closed`].
    // TODO: Dont close the room. It is being closed atm, because we have
    //      no way to handle absence of RtcPeerConnection when.
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
                ctx.notify(CloseRoom {})
            }
            ClosedReason::Lost => {
                self.drop_connection_tasks.insert(
                    member_id.clone(),
                    ctx.run_later(self.reconnect_timeout, move |_, ctx| {
                        info!(
                            "Member {} connection lost at {:?}. Room will be \
                             stopped.",
                            &member_id, closed_at
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

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: &MemberId,
        con: Box<dyn RpcConnection>,
    ) {
        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(member_id) {
            debug!("Closing old RpcConnection for member {}", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(member_id)
            {
                ctx.cancel_future(handler);
            }
            ctx.spawn(wrap_future(connection.close()));
        } else {
            debug!("Connected member: {}", member_id);

            self.connections.insert(member_id.clone(), con);
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s.
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> impl Future<Item = (), Error = ()> {
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

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
