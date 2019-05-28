//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending.

use std::{
    convert::TryFrom,
    sync::Arc,
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
        control::{Member, MemberId, MemberSpec, RoomSpec},
    },
    log::prelude::*,
    signalling::{
        room::{CloseRoom, CreatePeer, RoomError},
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

    /// Stores [`Member`]s which wait connection of another [`Member`].
    members_waiting_connection: HashMap<MemberId, Vec<Member>>,
}

impl ParticipantService {
    pub fn new(room_spec: &RoomSpec, reconnect_timeout: Duration) -> Self {
        let members = room_spec
            .spec
            .pipeline
            .iter()
            .map(|(control_id, entity)| {
                let member_spec = MemberSpec::try_from(entity.clone()).unwrap();

                (
                    control_id.clone(),
                    Member {
                        id: control_id.clone(),
                        spec: Arc::new(member_spec),
                    },
                )
            })
            .collect();
        debug!("Created room with {:?} users.", members);

        Self {
            members,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
            members_waiting_connection: HashMap::new(),
        }
    }

    /// Lookup [`Member`] by provided id and credentials. Returns
    /// [`Err(AuthorizationError::MemberNotExists)`] if lookup by [`MemberId`]
    /// failed. Returns [`Err(AuthorizationError::InvalidCredentials)`] if
    /// [`Member`] was found, but incorrect credentials was provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &str,
        credentials: &str,
    ) -> Result<&Member, AuthorizationError> {
        match self.members.get(member_id) {
            Some(ref member) => {
                if member.spec.credentials.eq(credentials) {
                    Ok(member)
                } else {
                    Err(AuthorizationError::InvalidCredentials)
                }
            }
            None => Err(AuthorizationError::MemberNotExists),
        }
    }

    /// Checks if [`Member`] has **active** [`RcpConnection`].
    pub fn member_has_connection(&self, member_id: &str) -> bool {
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

    /// Create [`Peer`]s between waiting [`Member`] and connected.
    ///
    /// Returns all control ID of members which we connected.
    fn connect_waiting_members(
        &self,
        member: &Member,
        ctx: &mut Context<Room>,
    ) -> Vec<String> {
        let mut added_member = Vec::new();
        if let Some(waiters) = self.members_waiting_connection.get(&member.id) {
            for waiter in waiters {
                added_member.push(waiter.id.clone());

                ctx.notify(CreatePeer {
                    caller: member.clone(),
                    responder: waiter.clone(),
                });
            }
        }
        added_member
    }

    /// Interconnect [`Peer`]s of members based on [`MemberSpec`].
    fn create_and_interconnect_members_peers(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: &str,
    ) {
        let connected_member = if let Some(m) = self.members.get(member_id) {
            m
        } else {
            warn!("Connected a non-existent member with id {}!", member_id);
            // Maybe better return error here?
            return;
        };
        let connected_member_play_endpoints =
            connected_member.spec.get_play_endpoints();

        let added_waiting_members =
            self.connect_waiting_members(&connected_member, ctx);
        self.members_waiting_connection.remove(member_id);

        for connected_member_endpoint in connected_member_play_endpoints {
            // Skip members which waiting for us because we added them before.
            if added_waiting_members
                .contains(&connected_member_endpoint.src.member_id)
            {
                continue;
            }

            let responder_id = &connected_member_endpoint.src.member_id;

            let is_responder_connected =
                self.member_has_connection(responder_id);
            if is_responder_connected {
                let responder = if let Some(m) = self.members.get(responder_id)
                {
                    m
                } else {
                    warn!(
                        "Member with id {} not found, but this member has \
                         connection!",
                        responder_id
                    );
                    continue;
                };

                ctx.notify(CreatePeer {
                    caller: responder.clone(),
                    responder: connected_member.clone(),
                });
            } else if let Some(m) =
                self.members_waiting_connection.get_mut(responder_id)
            {
                m.push(connected_member.clone());
            } else {
                self.members_waiting_connection.insert(
                    responder_id.clone(),
                    vec![connected_member.clone()],
                );
            }
        }
    }

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    /// Create and interconnect all necessary [`Member`]'s [`Peer`].
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: &str,
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

            self.create_and_interconnect_members_peers(ctx, member_id);

            self.connections.insert(member_id.to_string(), con);
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
