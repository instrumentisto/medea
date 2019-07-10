//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Members`] and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.

use std::{
    rc::Rc,
    sync::Arc,
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
        control::{
            endpoints::{
                WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
                WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
            },
            grpc::protos::control::Error as ErrorProto,
            local_uri::LocalUri,
            MemberId, MemberSpec, RoomId, RoomSpec, WebRtcPlayId,
            WebRtcPublishId,
        },
    },
    log::prelude::*,
    media::IceUser,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
            parse_members, Member, MembersLoadError,
        },
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
    #[fail(display = "Participant [id = {}] not found", _0)]
    ParticipantNotFound(LocalUri),
    #[fail(display = "Endpoint [id = {}] not found.", _0)]
    EndpointNotFound(LocalUri),
    #[fail(display = "Participant [id = {}] already exists.", _0)]
    ParticipantAlreadyExists(LocalUri),
    #[fail(display = "Endpoint [id = {}] already exists.", _0)]
    EndpointAlreadyExists(LocalUri),
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

impl From<MemberError> for ParticipantServiceErr {
    fn from(err: MemberError) -> Self {
        match err {
            MemberError::EndpointNotFound(e) => {
                ParticipantServiceErr::EndpointNotFound(e)
            }
        }
    }
}

impl Into<ErrorProto> for &ParticipantServiceErr {
    fn into(self) -> ErrorProto {
        let mut error = ErrorProto::new();
        match &self {
            ParticipantServiceErr::EndpointNotFound(id) => {
                error.set_element(id.to_string());
                error.set_code(0); // TODO
                error.set_status(404);
                error.set_text(self.to_string());
            }
            ParticipantServiceErr::ParticipantNotFound(id) => {
                error.set_element(id.to_string());
                error.set_code(0); // TODO
                error.set_status(404);
                error.set_text(self.to_string());
            }
            ParticipantServiceErr::ParticipantAlreadyExists(id) => {
                error.set_element(id.to_string());
                error.set_code(0); // TODO
                error.set_status(400);
                error.set_text(self.to_string());
            }
            ParticipantServiceErr::EndpointAlreadyExists(id) => {
                error.set_element(id.to_string());
                error.set_code(0); // TODO
                error.set_status(400);
                error.set_text(self.to_string());
            }
            _ => {
                error.set_element(String::new());
                error.set_code(0); // TODO
                error.set_status(500);
                error.set_text(format!(
                    "Unknow ParticipantService error. {:?}",
                    self
                ));
            }
        }
        error
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
    members: HashMap<MemberId, Rc<Member>>,

    /// Service for managing authorization on Turn server.
    turn: Arc<Box<dyn TurnAuthService + Send + Sync>>,

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
        turn: Arc<Box<dyn TurnAuthService + Send + Sync>>,
    ) -> Result<Self, MembersLoadError> {
        Ok(Self {
            room_id: room_spec.id().clone(),
            members: parse_members(room_spec)?,
            turn,
            connections: HashMap::new(),
            reconnect_timeout,
            drop_connection_tasks: HashMap::new(),
        })
    }

    /// Lookup [`Member`] by provided id.
    pub fn get_member_by_id(&self, id: &MemberId) -> Option<Rc<Member>> {
        self.members.get(id).cloned()
    }

    fn get_local_uri(&self, member_id: MemberId) -> LocalUri {
        LocalUri::new(Some(self.room_id.clone()), Some(member_id), None)
    }

    pub fn get_member(
        &self,
        id: &MemberId,
    ) -> Result<Rc<Member>, ParticipantServiceErr> {
        self.members.get(id).cloned().map_or(
            Err(ParticipantServiceErr::ParticipantNotFound(
                self.get_local_uri(id.clone()),
            )),
            Ok,
        )
    }

    pub fn members(&self) -> HashMap<MemberId, Rc<Member>> {
        self.members.clone()
    }

    /// Lookup [`Member`] by provided id and credentials. Returns
    /// [`Err(AuthorizationError::MemberNotExists)`] if lookup by
    /// [`MemberId`] failed. Returns
    /// [`Err(AuthorizationError::InvalidCredentials)`] if [`Member`]
    /// was found, but incorrect credentials was provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &MemberId,
        credentials: &str,
    ) -> Result<Rc<Member>, AuthorizationError> {
        match self.get_member_by_id(member_id) {
            Some(member) => {
                if member.credentials().eq(credentials) {
                    Ok(member.clone())
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

    /// Saves provided [`RpcConnection`], registers [`ICEUser`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        con: Box<dyn RpcConnection>,
    ) -> ActFuture<Rc<Member>, ParticipantServiceErr> {
        let member = match self.get_member_by_id(&member_id) {
            None => {
                return Box::new(wrap_future(future::err(
                    ParticipantServiceErr::ParticipantNotFound(
                        self.get_local_uri(member_id),
                    ),
                )));
            }
            Some(member) => member,
        };

        // lookup previous member connection
        if let Some(mut connection) = self.connections.remove(&member_id) {
            debug!("Closing old RpcConnection for participant {}", member_id);

            // cancel RpcConnection close task, since connection is
            // reestablished
            if let Some(handler) = self.drop_connection_tasks.remove(&member_id)
            {
                ctx.cancel_future(handler);
            }
            Box::new(wrap_future(connection.close().then(move |_| Ok(member))))
        } else {
            Box::new(
                wrap_future(self.turn.create(
                    member_id.clone(),
                    self.room_id.clone(),
                    UnreachablePolicy::ReturnErr,
                ))
                .map_err(|err, _: &mut Room, _| {
                    ParticipantServiceErr::from(err)
                })
                .and_then(
                    move |ice: IceUser, room: &mut Room, _| {
                        room.members.insert_connection(member_id.clone(), con);
                        member.replace_ice_user(ice);

                        wrap_future(future::ok(member))
                    },
                ),
            )
        }
    }

    /// Insert new [`RpcConnection`] into this [`ParticipantService`].
    fn insert_connection(
        &mut self,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) {
        self.connections.insert(member_id, conn);
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
        member_id: MemberId,
        reason: &ClosedReason,
    ) {
        let closed_at = Instant::now();
        match reason {
            ClosedReason::Closed => {
                self.connections.remove(&member_id);

                ctx.spawn(wrap_future(
                    self.delete_ice_user(&member_id).map_err(|err| {
                        error!("Error deleting IceUser {:?}", err)
                    }),
                ));
                // ctx.notify(CloseRoom {})
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

    /// Deletes [`IceUser`] associated with provided [`Member`].
    fn delete_ice_user(
        &mut self,
        member_id: &MemberId,
    ) -> Box<dyn Future<Item = (), Error = TurnServiceErr>> {
        match self.get_member_by_id(&member_id) {
            Some(member) => match member.take_ice_user() {
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
            let mut room_users = Vec::with_capacity(self.members.len());

            self.members.iter().for_each(|(_, data)| {
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

    pub fn delete_member(
        &mut self,
        member_id: &MemberId,
        ctx: &mut Context<Room>,
    ) {
        if let Some(drop) = self.drop_connection_tasks.remove(member_id) {
            ctx.cancel_future(drop);
        }

        if let Some(mut conn) = self.connections.remove(member_id) {
            ctx.spawn(wrap_future(conn.close()));
        }

        if let Some(member) = self.members.remove(member_id) {
            if let Some(ice_user) = member.take_ice_user() {
                let delete_ice_user_fut = self
                    .turn
                    .delete(vec![ice_user])
                    .map_err(|err| error!("Error removing IceUser {:?}", err));
                ctx.spawn(wrap_future(delete_ice_user_fut));
            }
        }
    }

    pub fn create_member(
        &mut self,
        id: MemberId,
        spec: MemberSpec,
    ) -> Result<(), ParticipantServiceErr> {
        if self.members.get(&id).is_some() {
            return Err(ParticipantServiceErr::ParticipantAlreadyExists(
                self.get_local_uri(id),
            ));
        }
        let signalling_member = Rc::new(Member::new(
            id.clone(),
            spec.credentials().to_string(),
            self.room_id.clone(),
        ));

        for (id, publish) in spec.publish_endpoints() {
            let signalling_publish = Rc::new(WebRtcPublishEndpoint::new(
                id.clone(),
                publish.p2p.clone(),
                Vec::new(),
                Rc::downgrade(&signalling_member),
            ));
            signalling_member.insert_src(signalling_publish);
        }

        for (id, play) in spec.play_endpoints() {
            let partner_member = self.get_member(&play.src.member_id)?;
            let src = partner_member.get_src(&play.src.endpoint_id)?;

            let sink = Rc::new(WebRtcPlayEndpoint::new(
                id.clone(),
                play.src.clone(),
                Rc::downgrade(&src),
                Rc::downgrade(&signalling_member),
            ));

            signalling_member.insert_sink(sink);
        }

        // This is needed for atomicity.
        for (_, sink) in signalling_member.sinks() {
            let src = sink.src();
            src.add_sink(Rc::downgrade(&sink));
        }

        self.members.insert(id, signalling_member);

        Ok(())
    }

    pub fn create_sink_endpoint(
        &mut self,
        member_id: MemberId,
        endpoint_id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
    ) -> Result<(), ParticipantServiceErr> {
        let member = self.get_member(&member_id)?;
        if member.get_sink_by_id(&endpoint_id).is_some() {
            return Err(ParticipantServiceErr::EndpointAlreadyExists(
                member.get_local_uri(endpoint_id.to_string()),
            ));
        }

        let partner_member = self.get_member(&spec.src.member_id)?;
        let src = partner_member.get_src(&spec.src.endpoint_id)?;

        let sink = Rc::new(WebRtcPlayEndpoint::new(
            endpoint_id,
            spec.src,
            Rc::downgrade(&src),
            Rc::downgrade(&member),
        ));

        src.add_sink(Rc::downgrade(&sink));
        member.insert_sink(sink);

        Ok(())
    }

    pub fn create_src_endpoint(
        &mut self,
        member_id: MemberId,
        endpoint_id: WebRtcPublishId,
        spec: WebRtcPublishEndpointSpec,
    ) -> Result<(), ParticipantServiceErr> {
        let member = self.get_member(&member_id)?;

        if member.get_src_by_id(&endpoint_id).is_some() {
            return Err(ParticipantServiceErr::EndpointAlreadyExists(
                member.get_local_uri(endpoint_id.to_string()),
            ));
        }

        let src = Rc::new(WebRtcPublishEndpoint::new(
            endpoint_id,
            spec.p2p,
            Vec::new(),
            Rc::downgrade(&member),
        ));

        member.insert_src(src);

        Ok(())
    }
}
