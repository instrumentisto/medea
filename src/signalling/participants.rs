//! Participant is ['Member'] with ['RpcConnection']. ['ParticipantService']
//! stores ['Members'] and associated ['RpcConnection']s, handles
//! ['RpcConnection'] authorization, establishment, message sending.

use actix::{AsyncContext, Context, SpawnHandle};
use hashbrown::HashMap;

use futures::{
    future::{self, join_all, Either},
    Future,
};

use std::time::{Duration, Instant};

use crate::{
    api::{
        client::rpc_connection::{
            AuthorizationError, ClosedReason, RpcConnection,
            RpcConnectionClosed,
        },
        control::{Member, MemberId},
        protocol::Event,
    },
    log::prelude::*,
    signalling::{
        room::{CloseRoom, RoomError},
        Room,
    },
};

/// Participant is ['Member'] with ['RpcConnection']. ['ParticipantService']
/// stores ['Members'] and associated ['RpcConnection']s, handles
/// ['RpcConnection'] authorization, establishment, message sending.
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
    /// If [`RpcConnection`] is lost, ['Room'] waits for connection_timeout
    /// before dropping it irrevocably in case it gets reestablished.
    drop_connection_tasks: HashMap<MemberId, SpawnHandle>,
}

impl ParticipantService {
    pub fn new(
        members: HashMap<MemberId, Member>,
        reconnect_timeout: Duration,
    ) -> Self {
        Self {
            members,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
        }
    }

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

    /// Send [`Event`] to specified remote [`Member`].
    pub fn send_event_to_member(
        &mut self,
        member_id: MemberId,
        event: Event,
    ) -> impl Future<Item = (), Error = RoomError> {
        match self.connections.get(&member_id) {
            Some(conn) => Either::A(
                conn.send_event(event)
                    .map_err(move |_| RoomError::UnableSendEvent(member_id)),
            ),
            None => Either::B(future::err(RoomError::ConnectionNotExists(
                member_id,
            ))),
        }
    }

    /// If ['ClosedReason::Closed'], then removes [`RpcConnection`] associated
    /// with specified user [`Member`] from the storage and closes the room.
    /// If ['ClosedReason::Lost'], then creates delayed task that emits
    /// ['ClosedReason::Closed'].
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

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) -> impl Future<Item = (), Error = ()> {
        let mut fut = Either::A(future::ok(()));
        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for member {}", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            fut = Either::B(connection.close());
        } else {
            self.connections.insert(member_id, con);
        }

        fut
    }

    /// Cancels all connection close tasks, closes all ['RpcConnection']s.
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
