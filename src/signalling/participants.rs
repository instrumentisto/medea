//! Participant is [`Member`] with [`RpcConnection`]. [`ParticipantService`]
//! stores [`Member`]s and associated [`RpcConnection`]s, handles
//! [`RpcConnection`] authorization, establishment, message sending, Turn
//! credentials management.
//!
//! [`Member`]: crate::signalling::elements::Member
//! [`RpcConnection`]: crate::api::client::rpc_connection::RpcConnection
//! [`ParticipantService`]: crate::signalling::participants::ParticipantService

use std::{collections::HashMap, time::Instant};

use actix::{
    fut::wrap_future, AsyncContext, Context, ContextFutureSpawner as _,
    SpawnHandle,
};
use derive_more::Display;
use failure::Fail;
use futures::future::{self, FutureExt as _, LocalBoxFuture};
use medea_client_api_proto::{
    CloseDescription, CloseReason, Credential, Event, MemberId, RoomId,
};

use crate::{
    api::{
        client::rpc_connection::{
            ClosedReason, RpcConnection, RpcConnectionClosed,
        },
        control::{
            refs::{Fid, ToEndpoint, ToMember},
            MemberSpec, RoomSpec,
        },
    },
    conf::Rpc as RpcConf,
    log::prelude::*,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
            parse_members, Member, MembersLoadError,
        },
        room::RoomError,
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
    /// [`Member`]: crate::signalling::elements::Member
    // TODO: Replace Box<dyn RpcConnection>> with enum,
    //       as the set of all possible RpcConnection types is not closed.
    connections: HashMap<MemberId, Box<dyn RpcConnection>>,

    /// Stores [`RpcConnection`] drop tasks.
    /// If [`RpcConnection`] is lost, [`Room`] waits for `connect_timeout`
    /// before dropping it irrevocably in case it gets reestablished.
    drop_connection_tasks: HashMap<MemberId, SpawnHandle>,

    /// Default values for the RPC connection settings.
    ///
    /// If nothing provided into [`Member`] element spec then this values will
    /// be used.
    rpc_conf: RpcConf,
}

impl ParticipantService {
    /// Creates new [`ParticipantService`] from [`RoomSpec`].
    ///
    /// # Errors
    ///
    /// Errors with [`MembersLoadError`] if [`RoomSpec`] transformation fails.
    pub fn new(
        room_spec: &RoomSpec,
        context: &AppContext,
    ) -> Result<Self, MembersLoadError> {
        Ok(Self {
            room_id: room_spec.id().clone(),
            members: parse_members(room_spec, context.config.rpc)?,
            connections: HashMap::new(),
            drop_connection_tasks: HashMap::new(),
            rpc_conf: context.config.rpc,
        })
    }

    /// Lookups [`Member`] by provided [`MemberId`].
    ///
    /// ## Errors
    ///
    /// With [`ParticipantServiceErr::ParticipantNotFound`] if [`Member`] lookup
    /// failed.
    pub fn get_member_by_id(
        &self,
        id: &MemberId,
    ) -> Result<Member, ParticipantServiceErr> {
        self.members.get(id).cloned().ok_or_else(|| {
            ParticipantServiceErr::ParticipantNotFound(
                self.get_fid_to_member(id.clone()),
            )
        })
    }

    /// Generates [`Fid`] which point to some [`Member`] in this
    /// [`ParticipantService`]'s [`Room`].
    ///
    /// __Note__ this function don't check presence of [`Member`] in
    /// [`ParticipantService`].
    #[inline]
    #[must_use]
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
        self.members.get(id).cloned().ok_or_else(|| {
            ParticipantServiceErr::ParticipantNotFound(
                self.get_fid_to_member(id.clone()),
            )
        })
    }

    /// Returns all [`Member`] from this [`ParticipantService`].
    #[inline]
    #[must_use]
    pub fn members(&self) -> HashMap<MemberId, Member> {
        self.members.clone()
    }

    /// Lookups [`Member`] by provided [`MemberId`] and credentials.
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::AuthorizationError`] if lookup by [`MemberId`]
    /// fails or if [`Member`] was found, but incorrect credentials were
    /// provided.
    pub fn get_member_by_id_and_credentials(
        &self,
        member_id: &MemberId,
        credentials: &Credential,
    ) -> Result<Member, RoomError> {
        #[allow(clippy::map_err_ignore)]
        let member = self
            .get_member_by_id(member_id)
            .map_err(|_| RoomError::AuthorizationError)?;
        if member.verify_credentials(credentials) {
            Ok(member)
        } else {
            Err(RoomError::AuthorizationError)
        }
    }

    /// Checks if [`Member`] has __active__ [`RpcConnection`].
    #[inline]
    #[must_use]
    pub fn member_has_connection(&self, member_id: &MemberId) -> bool {
        self.connections.contains_key(member_id)
            && !self.drop_connection_tasks.contains_key(member_id)
    }

    /// Sends [`Event`] to specified remote [`Member`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::ConnectionNotExists`] if unable to find
    /// [`RpcConnection`] with specified [`Member`].
    pub fn send_event_to_member(
        &self,
        member_id: MemberId,
        event: Event,
    ) -> Result<(), RoomError> {
        self.connections.get(&member_id).map_or(
            Err(RoomError::ConnectionNotExists(member_id)),
            |conn| {
                conn.send_event(self.room_id.clone(), event);
                Ok(())
            },
        )
    }

    /// Saves provided [`RpcConnection`].
    /// If [`Member`] already has any other [`RpcConnection`],
    /// then it will be closed.
    pub fn connection_established(
        &mut self,
        ctx: &mut Context<Room>,
        member_id: MemberId,
        conn: Box<dyn RpcConnection>,
    ) -> LocalBoxFuture<'static, Result<Member, ParticipantServiceErr>> {
        let member = match self.get_member_by_id(&member_id) {
            Err(err) => {
                return Box::pin(future::err(err));
            }
            Ok(member) => member,
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
            Box::pin(
                connection
                    .close(
                        self.room_id.clone(),
                        CloseDescription::new(CloseReason::Reconnected),
                    )
                    .map(move |_| Ok(member)),
            )
        } else {
            self.insert_connection(member_id, conn);
            Box::pin(future::ok(member))
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

    /// Creates delayed task that emits [`ClosedReason::Closed`].
    pub fn connection_lost(
        &mut self,
        member_id: MemberId,
        ctx: &mut Context<Room>,
    ) {
        let lost_at = Instant::now();
        if let Ok(member) = self.get_member_by_id(&member_id) {
            self.drop_connection_tasks.insert(
                member_id.clone(),
                ctx.run_later(member.get_reconnect_timeout(), move |_, ctx| {
                    info!(
                        "Member [id = {}] connection lost at {:?}.",
                        member_id, lost_at,
                    );
                    ctx.notify(RpcConnectionClosed {
                        member_id,
                        reason: ClosedReason::Closed { normal: false },
                    })
                }),
            );
        }
    }

    /// Cancels all connection close tasks, closes all [`RpcConnection`]s and
    /// deletes all [`IceUser`]s.
    ///
    /// [`IceUser`]: crate::turn::ice_user::IceUser
    pub fn drop_connections(
        &mut self,
        ctx: &mut Context<Room>,
    ) -> LocalBoxFuture<'static, ()> {
        // canceling all drop_connection_tasks
        self.drop_connection_tasks.drain().for_each(|(_, handle)| {
            ctx.cancel_future(handle);
        });

        let room_id = self.room_id.clone();
        // closing all RpcConnection's
        let close_rpc_connections =
            future::join_all(self.connections.drain().fold(
                Vec::new(),
                |mut futs, (_, mut connection)| {
                    futs.push(connection.close(
                        room_id.clone(),
                        CloseDescription::new(CloseReason::Finished),
                    ));
                    futs
                },
            ));

        close_rpc_connections.map(drop).boxed_local()
    }

    /// Deletes a [`Member`] by its ID from this [`ParticipantService`].
    pub fn delete_member(&mut self, member_id: &MemberId) {
        self.members.remove(member_id);
    }

    /// Closes [`RpcConnection`] with [`Member`] with the provided [`MemberId`].
    pub fn close_member_connection(
        &mut self,
        member_id: &MemberId,
        close_reason: CloseReason,
        ctx: &mut Context<Room>,
    ) {
        if let Some(drop) = self.drop_connection_tasks.remove(member_id) {
            ctx.cancel_future(drop);
        }

        if let Some(mut conn) = self.connections.remove(member_id) {
            wrap_future::<_, Room>(conn.close(
                self.room_id.clone(),
                CloseDescription::new(close_reason),
            ))
            .spawn(ctx);
        }
    }

    /// Inserts given [`Member`] into [`ParticipantService`].
    pub fn insert_member(&mut self, id: MemberId, member: Member) {
        self.members.insert(id, member);
    }

    /// Returns [`Iterator`] over [`MemberId`] and [`Member`] which this
    /// [`ParticipantService`] stores.
    pub fn iter_members(&self) -> impl Iterator<Item = (&MemberId, &Member)> {
        self.members.iter()
    }

    /// Creates new [`Member`] in this [`ParticipantService`].
    ///
    /// This function will check that new [`Member`]'s ID is not present in
    /// [`ParticipantService`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::MemberAlreadyExists`] if [`Member`] with
    /// provided [`MemberId`] already exists in [`ParticipantService`].
    pub fn create_member(
        &mut self,
        id: MemberId,
        spec: &MemberSpec,
    ) -> Result<(), RoomError> {
        if self.get_member_by_id(&id).is_ok() {
            return Err(RoomError::MemberAlreadyExists(
                self.get_fid_to_member(id),
            ));
        }
        let signalling_member = Member::new(
            id.clone(),
            spec.credentials().clone(),
            self.room_id.clone(),
            spec.idle_timeout().unwrap_or(self.rpc_conf.idle_timeout),
            spec.reconnect_timeout()
                .unwrap_or(self.rpc_conf.reconnect_timeout),
            spec.ping_interval().unwrap_or(self.rpc_conf.ping_interval),
        );

        signalling_member.set_callback_urls(spec);

        for (id, publish) in spec.publish_endpoints() {
            let signalling_publish = WebRtcPublishEndpoint::new(
                id.clone(),
                publish.p2p,
                signalling_member.downgrade(),
                publish.force_relay,
                publish.audio_settings,
                publish.video_settings,
            );
            signalling_member.insert_src(signalling_publish);
        }

        for (id, play) in spec.play_endpoints() {
            let partner_member = self.get_member(&play.src.member_id)?;
            let src = partner_member
                .get_src_by_id(&play.src.endpoint_id)
                .ok_or_else(|| {
                    MemberError::EndpointNotFound(
                        partner_member.get_fid_to_endpoint(
                            play.src.endpoint_id.clone().into(),
                        ),
                    )
                })?;

            let sink = WebRtcPlayEndpoint::new(
                id.clone(),
                play.src.clone(),
                src.downgrade(),
                signalling_member.downgrade(),
                play.force_relay,
            );

            signalling_member.insert_sink(sink);
        }

        // This is needed for atomicity.
        for (_, sink) in signalling_member.sinks() {
            let src = sink.src();
            src.add_sink(sink.downgrade());
        }

        self.insert_member(id, signalling_member);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::{
        api::control::{member::Credential, pipeline::Pipeline},
        conf::Conf,
    };

    use super::*;

    pub fn empty_participants_service() -> ParticipantService {
        let room_spec = RoomSpec {
            id: RoomId::from("test"),
            pipeline: Pipeline::new(HashMap::new()),
        };
        let ctx = AppContext::new(
            Conf::default(),
            crate::turn::new_turn_auth_service_mock(),
        );

        ParticipantService::new(&room_spec, &ctx).unwrap()
    }

    /// Tests that when no RPC settings is provided in the `Member` element
    /// spec, default RPC settings from config will be used.
    #[test]
    fn use_conf_when_no_rpc_settings_in_member_spec() {
        let mut members = empty_participants_service();

        let test_member_spec = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            Credential::Plain("w/e".into()),
            None,
            None,
            None,
            None,
            None,
        );

        let test_member_id = MemberId::from("test-member");
        members
            .create_member(test_member_id.clone(), &test_member_spec)
            .unwrap();

        let test_member = members.get_member_by_id(&test_member_id).unwrap();
        let default_rpc_conf = Conf::default().rpc;

        assert_eq!(
            test_member.get_ping_interval(),
            default_rpc_conf.ping_interval
        );
        assert_eq!(
            test_member.get_idle_timeout(),
            default_rpc_conf.idle_timeout
        );
        assert_eq!(
            test_member.get_reconnect_timeout(),
            default_rpc_conf.reconnect_timeout
        );
    }

    /// Tests that when RPC settings is provided in the `Member` element spec,
    /// this RPC settings will be used.
    #[test]
    fn use_rpc_settings_from_member_spec() {
        let mut members = empty_participants_service();

        let idle_timeout = Duration::from_secs(60);
        let ping_interval = Duration::from_secs(61);
        let reconnect_timeout = Duration::from_secs(62);

        let test_member_spec = MemberSpec::new(
            Pipeline::new(HashMap::new()),
            Credential::Plain("w/e".into()),
            None,
            None,
            Some(idle_timeout),
            Some(reconnect_timeout),
            Some(ping_interval),
        );

        let test_member_id = MemberId::from("test-member");
        members
            .create_member(test_member_id.clone(), &test_member_spec)
            .unwrap();

        let test_member = members.get_member_by_id(&test_member_id).unwrap();
        assert_eq!(test_member.get_ping_interval(), ping_interval);
        assert_eq!(test_member.get_idle_timeout(), idle_timeout);
        assert_eq!(test_member.get_reconnect_timeout(), reconnect_timeout);
    }
}
