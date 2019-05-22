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

use crate::api::control::element::Element;
use crate::signalling::room::{ConnectPeers, CreatePeer};
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

    control_signalling_members: HashMap<String, MemberId>,

    responder_awaiting_connection: HashMap<String, Vec<MemberId>>,
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
            responder_awaiting_connection: HashMap::new(),
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

    /// Stores provided [`RpcConnection`] for given [`Member`] in the [`Room`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) {
        use std::convert::TryFrom;
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
            // TODO: Think about deletion
            debug!("Connected member: {}", member_id);
            debug!(
                "Current awaiters: {:?}",
                self.responder_awaiting_connection
            );

            // TODO: Very need serious refactor
            let connected_member_pipeline =
                self.members.get(&member_id).unwrap().clone();
            connected_member_pipeline
                .spec
                .pipeline
                .iter()
                .filter_map(|(connected_element_id, connected_element)| {
                    match connected_element {
                        Element::WebRtcPlayEndpoint { spec } => Some(spec),
                        _ => None,
                    }
                })
                .cloned()
                .for_each(|connected_play_endpoint| {
                    let responder_signalling_id = self
                        .control_signalling_members
                        .get(&connected_play_endpoint.src.member_id)
                        .unwrap();
                    let is_responder_connected =
                        self.connections.get(responder_signalling_id).is_some();

                    let this_name = self
                        .members
                        .get(&member_id)
                        .unwrap()
                        .control_id
                        .clone();

                    if let Some(awaiters) =
                        self.responder_awaiting_connection.get(&this_name)
                    {
                        awaiters.iter().for_each(|a| {
                            ctx.notify(CreatePeer {
                                first_member_pipeline: self
                                    .members
                                    .get(a)
                                    .unwrap()
                                    .spec
                                    .pipeline
                                    .clone(),
                                second_member_pipeline: self
                                    .members
                                    .get(&member_id)
                                    .unwrap()
                                    .spec
                                    .pipeline
                                    .clone(),
                                second_signalling_id: member_id,
                                first_signalling_id: *a,
                            });
                        });
                        self.responder_awaiting_connection.remove(&this_name);
                    };

                    if is_responder_connected {
                        ctx.notify(CreatePeer {
                            first_member_pipeline: self
                                .members
                                .get(&member_id)
                                .unwrap()
                                .spec
                                .pipeline
                                .clone(),
                            second_member_pipeline: self
                                .members
                                .get(&responder_signalling_id)
                                .unwrap()
                                .spec
                                .pipeline
                                .clone(),
                            first_signalling_id: member_id,
                            second_signalling_id: *responder_signalling_id,
                        });
                    } else {
                        match self
                            .responder_awaiting_connection
                            .get_mut(&connected_play_endpoint.src.member_id)
                        {
                            Some(awaiter) => {
                                awaiter.push(member_id);
                            }
                            None => {
                                self.responder_awaiting_connection.insert(
                                    connected_play_endpoint
                                        .src
                                        .member_id
                                        .clone(),
                                    vec![member_id],
                                );
                            }
                        }
                    }
                });

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
