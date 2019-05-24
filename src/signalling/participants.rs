//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending.

use std::time::{Duration, Instant};

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
        control::{Member, MemberId},
    },
    media::NewPeer,
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

    /// Stores relation between ID of [`MemberSpec`] and ID of signalling
    /// [`Member`].
    control_signalling_members: HashMap<String, MemberId>,

    /// Stores [`NewPeer`] which wait connection of another [`Member`].
    members_awaiting_connection: HashMap<MemberId, NewPeer>,
}

impl ParticipantService {
    pub fn new(
        members: HashMap<MemberId, Member>,
        control_signalling_members: HashMap<String, MemberId>,
        reconnect_timeout: Duration,
    ) -> Self {
        Self {
            members,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
            control_signalling_members,
            members_awaiting_connection: HashMap::new(),
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

    /// Create [`Peer`]s between waiting [`Member`] and connected.
    ///
    /// Return control ID of waiting [`MemberSpec`].
    fn connect_waiting_if_exist(
        &self,
        member: Member,
        ctx: &mut Context<Room>,
    ) -> Option<String> {
        let mut added_member = None;
        if let Some(awaiter) = self.members_awaiting_connection.get(&member.id)
        {
            added_member = Some(awaiter.control_id.clone());
            let connected_new_peer = NewPeer {
                control_id: member.control_id,
                signalling_id: member.id,
                spec: member.spec,
            };
            let awaiter_new_peer = NewPeer {
                spec: awaiter.spec.clone(),
                control_id: awaiter.control_id.clone(),
                signalling_id: awaiter.signalling_id,
            };

            ctx.notify(CreatePeer {
                caller: connected_new_peer,
                responder: awaiter_new_peer,
            });
        }
        added_member
    }

    /// Interconnect [`Peer`]s of members based on [`MemberSpec`].
    fn create_and_interconnect_members_peers(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
    ) {
        let connected_member = match self.members.get(&member_id) {
            Some(m) => m,
            None => {
                warn!("Connected a non-existent member with id {}!", member_id);
                return;
            }
        };
        let connected_member_play_endpoints =
            connected_member.spec.get_play_endpoints();

        let added_member =
            self.connect_waiting_if_exist(connected_member.clone(), ctx);
        self.members_awaiting_connection.remove(&member_id);

        for connected_member_endpoint in connected_member_play_endpoints {
            if let Some(ref c) = added_member {
                if &connected_member_endpoint.src.member_id == c {
                    continue;
                }
            }

            let responder_member_control_id =
                connected_member_endpoint.src.member_id;
            let responder_member_signalling_id = match self
                .control_signalling_members
                .get(&responder_member_control_id)
            {
                Some(r) => r,
                None => {
                    warn!(
                        "Member with control id '{}' not found! Probably this \
                         is error in spec.",
                        responder_member_control_id
                    );
                    continue;
                }
            };

            let connected_new_peer = NewPeer {
                signalling_id: connected_member.id,
                spec: connected_member.spec.clone(),
                control_id: connected_member.control_id.clone(),
            };

            let is_responder_connected =
                self.member_has_connection(*responder_member_signalling_id);
            if is_responder_connected {
                let responder_new_peer = NewPeer {
                    signalling_id: *responder_member_signalling_id,
                    spec: self
                        .members
                        .get(&responder_member_signalling_id)
                        .unwrap()
                        .spec
                        .clone(),
                    control_id: responder_member_control_id,
                };
                ctx.notify(CreatePeer {
                    caller: responder_new_peer,
                    responder: connected_new_peer,
                });
            } else {
                self.members_awaiting_connection.insert(
                    *responder_member_signalling_id,
                    connected_new_peer,
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
        member_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) {
        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for member {}", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            ctx.spawn(wrap_future(connection.close()));
        } else {
            debug!("Connected member: {}", member_id);

            self.create_and_interconnect_members_peers(ctx, member_id);

            self.connections.insert(member_id, con);
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
