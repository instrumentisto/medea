//! Handlers for messages sent via [Control API], i.e. dynamic [`Room`] pipeline
//! mutations.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{collections::{HashMap, HashSet}, convert::TryInto as _};

use actix::{
    ActorFuture as _, AsyncContext, AtomicResponse, Context, Handler, Message,
    WrapFuture as _,
};
use medea_client_api_proto::{CloseReason, MemberId};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        callback::OnLeaveReason,
        endpoints::{
            WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
            WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
        },
        refs::StatefulFid,
        EndpointId, EndpointSpec, MemberSpec, RoomSpec, WebRtcPlayId,
        WebRtcPublishId,
    },
    log::prelude::*,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
        },
        room_service::Sids,
        peers::PeerChange,
        room::ActFuture,
    },
};

use super::{Room, RoomError};

impl Room {
    /// Deletes [`Member`] from this [`Room`] by [`MemberId`].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    fn delete_member(&mut self, member_id: &MemberId, ctx: &mut Context<Self>) {
        debug!(
            "Deleting Member [id = {}] in Room [id = {}].",
            member_id, self.id
        );
        if self.members.get_member_by_id(member_id).is_ok() {
            self.disconnect_member(
                member_id,
                CloseReason::Evicted,
                None, /* No need to callback, since delete is initiated by
                       * Control Service. */
                ctx,
            );
            self.members.delete_member(member_id);
            debug!(
                "Member [id = {}] deleted from Room [id = {}].",
                member_id, self.id
            );
        }
    }

    /// Deletes endpoint from this [`Room`] by ID.
    ///
    /// Starts a renegotiation process for the affected [`Peer`]s if required.
    ///
    /// Deletes its [`Peer`] if the deleted endpoint is the last one associated
    /// with it.
    fn delete_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: EndpointId,
    ) -> Result<(), RoomError> {
        if let Ok(member) = self.members.get_member_by_id(member_id) {
            let play_id = endpoint_id.into();
            let changeset =
                if let Some(sink_endpoint) = member.remove_sink(&play_id) {
                    self.peers.delete_sink_endpoint(&sink_endpoint)?
                } else {
                    let publish_id = String::from(play_id).into();

                    if let Some(src_endpoint) = member.remove_src(&publish_id) {
                        self.peers.delete_src_endpoint(&src_endpoint)?
                    } else {
                        HashSet::new()
                    }
                };

            {
                let mut removed_peers: HashMap<_, Vec<_>> = HashMap::new();
                let mut discard_updates = HashSet::new();
                let mut updated_peers = HashSet::new();
                for change in changeset {
                    match change {
                        PeerChange::Removed(member_id, peer_id) => {
                            removed_peers
                                .entry(member_id)
                                .or_default()
                                .push(peer_id);
                            discard_updates.insert(peer_id);
                        }
                        PeerChange::Updated(peer_id) => {
                            updated_peers.insert(peer_id);
                        }
                    }
                }
                for updated_peer_id in updated_peers {
                    self.peers.commit_scheduled_changes(updated_peer_id)?;
                }

                for (member_id, peer_ids) in removed_peers {
                    self.send_peers_removed(&member_id, peer_ids);
                }
            };
        }

        Ok(())
    }

    /// Creates new [`WebRtcPlayEndpoint`] in specified [`Member`].
    ///
    /// This function will check that new [`WebRtcPublishEndpoint`]'s ID is not
    /// present in [`ParticipantService`][1].
    ///
    /// Returns [`RoomError::EndpointAlreadyExists`] when
    /// [`WebRtcPublishEndpoint`]'s ID already presented in [`Member`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] was not found in [`ParticipantService`][1].
    ///
    /// [`Member`]: crate::signalling::elements::Member
    /// [1]: crate::signalling::participants::ParticipantService
    fn create_src_endpoint(
        &mut self,
        member_id: &MemberId,
        publish_id: WebRtcPublishId,
        spec: &WebRtcPublishEndpointSpec,
    ) -> Result<(), RoomError> {
        let member = self.members.get_member(&member_id)?;

        let is_member_have_this_src_id =
            member.get_src_by_id(&publish_id).is_some();

        let play_id = String::from(publish_id).into();
        let is_member_have_this_sink_id =
            member.get_sink_by_id(&play_id).is_some();

        if is_member_have_this_sink_id || is_member_have_this_src_id {
            return Err(RoomError::EndpointAlreadyExists(
                member.get_fid_to_endpoint(play_id.into()),
            ));
        }

        let endpoint = WebRtcPublishEndpoint::new(
            String::from(play_id).into(),
            spec.p2p,
            member.downgrade(),
            spec.force_relay,
            spec.audio_settings,
            spec.video_settings,
        );

        debug!(
            "Create WebRtcPublishEndpoint [id = {}] for Member [id = {}] in \
             Room [id = {}]",
            endpoint.id(),
            member_id,
            self.id
        );

        member.insert_src(endpoint);

        Ok(())
    }

    /// Creates new [`WebRtcPlayEndpoint`] in specified [`Member`].
    ///
    /// This function will check that new [`WebRtcPlayEndpoint`]'s ID is not
    /// present in [`ParticipantService`][1].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::EndpointAlreadyExists`] if
    /// [`WebRtcPlayEndpoint`]'s ID already presented in [`Member`].
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] doesn't exist.
    ///
    /// [`Member`]: crate::signalling::elements::Member
    /// [1]: crate::signalling::participants::ParticipantService
    fn create_sink_endpoint(
        &mut self,
        ctx: &mut Context<Self>,
        member_id: &MemberId,
        endpoint_id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
    ) -> Result<(), RoomError> {
        let member = self.members.get_member(&member_id)?;

        let is_member_have_this_sink_id =
            member.get_sink_by_id(&endpoint_id).is_some();

        let publish_id = String::from(endpoint_id).into();
        let is_member_have_this_src_id =
            member.get_src_by_id(&publish_id).is_some();
        if is_member_have_this_sink_id || is_member_have_this_src_id {
            return Err(RoomError::EndpointAlreadyExists(
                member.get_fid_to_endpoint(publish_id.into()),
            ));
        }

        let partner_member = self.members.get_member(&spec.src.member_id)?;
        let src = partner_member
            .get_src_by_id(&spec.src.endpoint_id)
            .ok_or_else(|| {
                MemberError::EndpointNotFound(
                    partner_member.get_fid_to_endpoint(
                        spec.src.endpoint_id.clone().into(),
                    ),
                )
            })?;

        let sink = WebRtcPlayEndpoint::new(
            String::from(publish_id).into(),
            spec.src,
            src.downgrade(),
            member.downgrade(),
            spec.force_relay,
        );

        src.add_sink(sink.downgrade());

        debug!(
            "Created WebRtcPlayEndpoint [id = {}] for Member [id = {}] in \
             Room [id = {}].",
            sink.id(),
            member_id,
            self.id
        );

        member.insert_sink(sink);

        if self.members.member_has_connection(&member_id) {
            let member_id = member_id.clone();
            ctx.spawn(
                self.init_member_connections(&member).map(
                    move |res, this, ctx| {
                        if let Err(e) = res {
                            error!(
                                "Failed to interconnect Members, because {}",
                                e,
                            );
                            this.disconnect_member(
                                &member_id,
                                CloseReason::InternalError,
                                Some(OnLeaveReason::Kicked),
                                ctx,
                            );
                        }
                    },
                ),
            );
        }

        Ok(())
    }
}

impl From<&Room> for proto::Room {
    fn from(room: &Room) -> Self {
        let pipeline = room
            .members
            .members()
            .into_iter()
            .map(|(id, member)| (id.to_string(), member.into()))
            .collect();
        Self {
            id: room.id().to_string(),
            pipeline,
        }
    }
}

impl From<&Room> for proto::Element {
    #[inline]
    fn from(room: &Room) -> Self {
        Self {
            el: Some(proto::element::El::Room(room.into())),
        }
    }
}

// TODO: Tightly coupled with protobuf.
//       We should name this method GetElements, that will return some
//       intermediate DTO, that will be serialized at the caller side.
//       But lets leave it as it is for now.

/// Message for serializing this [`Room`] and [`Room`]'s elements to protobuf
/// spec.
#[derive(Message)]
#[rtype(result = "Result<HashMap<StatefulFid, proto::Element>, RoomError>")]
pub struct SerializeProto(pub Vec<StatefulFid>);

impl Handler<SerializeProto> for Room {
    type Result = Result<HashMap<StatefulFid, proto::Element>, RoomError>;

    fn handle(
        &mut self,
        msg: SerializeProto,
        _: &mut Self::Context,
    ) -> Self::Result {
        let mut serialized: HashMap<StatefulFid, proto::Element> =
            HashMap::new();
        for fid in msg.0 {
            match &fid {
                StatefulFid::Room(room_fid) => {
                    if room_fid.room_id() == &self.id {
                        let current_room: proto::Element = (&*self).into();
                        serialized.insert(fid, current_room);
                    } else {
                        return Err(RoomError::WrongRoomId(
                            fid,
                            self.id.clone(),
                        ));
                    }
                }
                StatefulFid::Member(member_fid) => {
                    let member =
                        self.members.get_member(member_fid.member_id())?;
                    serialized.insert(fid, member.into());
                }
                StatefulFid::Endpoint(endpoint_fid) => {
                    let member =
                        self.members.get_member(endpoint_fid.member_id())?;
                    let endpoint = member.get_endpoint_by_id(
                        endpoint_fid.endpoint_id().to_string(),
                    )?;
                    serialized.insert(fid, endpoint.into());
                }
            }
        }

        Ok(serialized)
    }
}

/// Signal for deleting elements from this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Delete(pub Vec<StatefulFid>);

impl Handler<Delete> for Room {
    type Result = ();

    fn handle(&mut self, msg: Delete, ctx: &mut Self::Context) {
        let mut member_ids = Vec::new();
        let mut endpoint_ids = Vec::new();
        for id in msg.0 {
            match id {
                StatefulFid::Member(member_fid) => {
                    member_ids.push(member_fid);
                }
                StatefulFid::Endpoint(endpoint_fid) => {
                    endpoint_ids.push(endpoint_fid);
                }
                StatefulFid::Room(_) => {
                    warn!("Found Fid<IsRoomId> while deleting __from__ Room.");
                }
            }
        }
        member_ids.into_iter().for_each(|fid| {
            self.delete_member(&fid.member_id(), ctx);
        });
        endpoint_ids.into_iter().for_each(|fid| {
            let (_, member_id, endpoint_id) = fid.take_all();
            if let Err(e) = self.delete_endpoint(&member_id, endpoint_id) {
                error!("Error while deleting endpoint: {:?}", e);
            }
        });
    }
}

/// Signal for applying [`MemberSpec`] in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<Sids, RoomError>")]
pub struct ApplyMember(pub MemberId, pub MemberSpec);

impl Handler<ApplyMember> for Room {
    type Result = Result<Sids, RoomError>;

    fn handle(
        &mut self,
        msg: ApplyMember,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        let mut sids = Sids::new();
        if let Ok(member) = self.members.get_member(&msg.0) {
            for (id, endpoint) in msg.1.publish_endpoints() {
                if member.get_src_by_id(&id).is_none() {
                    self.create_src_endpoint(&msg.0, id, endpoint)?;
                }
            }
            for (id, endpoint) in msg.1.play_endpoints() {
                if member.get_sink_by_id(&id).is_none() {
                    self.create_sink_endpoint(
                        ctx,
                        &msg.0,
                        id,
                        endpoint.clone(),
                    )?;
                }
            }
            for id in member.srcs_ids() {
                if msg.1.get_publish_endpoint_by_id(id.clone()).is_none() {
                    self.delete_endpoint(&msg.0, id.into());
                }
            }
            for id in member.sinks_ids() {
                if msg.1.get_play_endpoint_by_id(id.clone()).is_none() {
                    self.delete_endpoint(&msg.0, id.into());
                }
            }
        } else {
            sids = self.members.create_members(&[(&msg.0, msg.1)])?;
        }
        Ok(sids)
    }
}

/// Signal for applying [`RoomSpec`] in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<Sids, RoomError>")]
pub struct Apply(pub RoomSpec);

impl Handler<Apply> for Room {
    type Result = Result<Sids, RoomError>;

    fn handle(&mut self, msg: Apply, ctx: &mut Self::Context) -> Self::Result {
        let mut create_src_endpoint = Vec::new();
        let mut create_sink_endpoint = Vec::new();
        let mut create_members = Vec::new();
        for (id, element) in &msg.0.pipeline {
            let spec: MemberSpec = element.try_into()?;
            if let Ok(member) = self.members.get_member(&id) {
                for (src_id, src) in spec.publish_endpoints() {
                    if member.get_src_by_id(&src_id).is_none() {
                        create_src_endpoint.push((
                            id,
                            src_id.clone(),
                            src.clone(),
                        ));
                    }
                }

                for (sink_id, sink) in spec.play_endpoints() {
                    if member.get_sink_by_id(&sink_id).is_none() {
                        create_sink_endpoint.push((
                            id,
                            sink_id.clone(),
                            sink.clone(),
                        ));
                    }
                }

                for (src_id, _) in member.srcs() {
                    if spec.get_publish_endpoint_by_id(src_id.clone()).is_none()
                    {
                        self.delete_endpoint(&id, src_id.into());
                    }
                }

                for (sink_id, _) in member.sinks() {
                    if spec.get_play_endpoint_by_id(sink_id.clone()).is_none() {
                        self.delete_endpoint(&id, sink_id.into());
                    }
                }
            } else {
                create_members.push((id, spec));
            }
        }

        let sids = self.members.create_members(&create_members)?;

        for (id, src_id, src) in create_src_endpoint {
            self.create_src_endpoint(id, src_id, &src)?;
        }
        for (id, sink_id, sink) in create_sink_endpoint {
            self.create_sink_endpoint(ctx, &id, sink_id, sink)?;
        }

        for id in self.members.members_ids() {
            if !msg.0.pipeline.contains_key(&id) {
                self.delete_member(&id, ctx);
            }
        }

        Ok(sids)
    }
}

/// Signal for creating new `Member` in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<Sids, RoomError>")]
pub struct CreateMember(pub MemberId, pub MemberSpec);

impl Handler<CreateMember> for Room {
    type Result = Result<Sids, RoomError>;

    fn handle(
        &mut self,
        msg: CreateMember,
        _: &mut Self::Context,
    ) -> Self::Result {
        let CreateMember(id, spec) = msg;
        let sids = self.members.create_members(&[(&id, spec)])?;
        debug!("Member [id = {}] created in Room [id = {}].", id, self.id);
        Ok(sids)
    }
}

/// Signal for creating new `Endpoint` from [`EndpointSpec`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateEndpoint {
    pub member_id: MemberId,
    pub endpoint_id: EndpointId,
    pub spec: EndpointSpec,
}

impl Handler<CreateEndpoint> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateEndpoint,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg.spec {
            EndpointSpec::WebRtcPlay(endpoint) => {
                self.create_sink_endpoint(
                    ctx,
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    endpoint,
                )?;
            }
            EndpointSpec::WebRtcPublish(endpoint) => {
                self.create_src_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    &endpoint,
                )?;
            }
        }
        Ok(())
    }
}

/// Signal for closing this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Close;

impl Handler<Close> for Room {
    type Result = AtomicResponse<Self, ()>;

    fn handle(&mut self, _: Close, ctx: &mut Self::Context) -> Self::Result {
        for id in self.members.members().keys() {
            self.delete_member(id, ctx);
        }
        AtomicResponse::new(Box::pin(
            self.members.drop_connections(ctx).into_actor(self),
        ))
    }
}
