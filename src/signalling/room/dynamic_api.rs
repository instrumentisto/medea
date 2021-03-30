//! Handlers for messages sent via [Control API], i.e. dynamic [`Room`] pipeline
//! mutations.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::collections::HashMap;

use actix::{
    fut, ActorFuture as _, AsyncContext as _, AtomicResponse, Context, Handler,
    Message, WrapFuture as _,
};
use medea_client_api_proto::{CloseReason, MemberId, PeerId};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        callback::OnLeaveReason,
        endpoints::{
            WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
            WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
        },
        refs::StatefulFid,
        EndpointId, EndpointSpec, MemberSpec, WebRtcPlayId, WebRtcPublishId,
    },
    log::prelude::*,
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
        },
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
    fn delete_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: EndpointId,
    ) {
        let endpoint_id =
            if let Ok(member) = self.members.get_member_by_id(member_id) {
                let play_id = endpoint_id.into();
                if let Some(endpoint) = member.take_sink(&play_id) {
                    if let Some(peer_id) = endpoint.peer_id() {
                        let removed_peers =
                            self.peers.remove_peers(member_id, &[peer_id]);
                        for (member_id, peers) in removed_peers {
                            self.member_peers_removed(
                                peers.into_iter().map(|p| p.id()).collect(),
                                member_id,
                            );
                        }
                    }
                }

                let publish_id = String::from(play_id).into();
                if let Some(endpoint) = member.take_src(&publish_id) {
                    let peer_ids = endpoint.peer_ids();
                    self.remove_peers(member_id, &peer_ids);
                }

                publish_id.into()
            } else {
                endpoint_id
            };

        debug!(
            "Endpoint [id = {}] removed in Member [id = {}] from Room [id = \
             {}].",
            endpoint_id, member_id, self.id
        );
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
        member_id: &MemberId,
        endpoint_id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
    ) -> Result<ActFuture<Result<(), RoomError>>, RoomError> {
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

        Ok(Box::pin(fut::ready(()).map(
            move |_, this: &mut Self, ctx| {
                let member_id = member.id();
                if this.members.member_has_connection(&member_id) {
                    ctx.spawn(this.init_member_connections(&member).map(
                        move |res, this, ctx| {
                            if let Err(e) = res {
                                error!(
                                    "Failed to interconnect Members, because \
                                     {}",
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
                    ));
                }
                Ok(())
            },
        )))
    }

    /// Removes [`Peer`]s and call [`Room::member_peers_removed`] for every
    /// [`Member`].
    ///
    /// This will delete [`Peer`]s from [`Room::peers`] and send
    /// [`Event::PeersRemoved`] event to [`Member`].
    ///
    /// [`Event::PeersRemoved`]: medea_client_api_proto::Event::PeersRemoved
    /// [`Member`]: crate::signalling::elements::Member
    /// [`Peer`]: crate::media::peer::Peer
    fn remove_peers<'a, Peers: IntoIterator<Item = &'a PeerId>>(
        &mut self,
        member_id: &MemberId,
        peer_ids_to_remove: Peers,
    ) {
        debug!("Remove peers.");
        self.peers
            .remove_peers(&member_id, peer_ids_to_remove)
            .into_iter()
            .for_each(|(member_id, peers)| {
                self.member_peers_removed(
                    peers.into_iter().map(|p| p.id()).collect(),
                    member_id,
                );
            });
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
            self.delete_endpoint(&member_id, endpoint_id);
        });
    }
}

/// Signal for creating new `Member` in this [`Room`].
#[derive(Message, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct CreateMember(pub MemberId, pub MemberSpec);

impl Handler<CreateMember> for Room {
    type Result = Result<(), RoomError>;

    fn handle(
        &mut self,
        msg: CreateMember,
        _: &mut Self::Context,
    ) -> Self::Result {
        self.members.create_member(msg.0.clone(), &msg.1)?;
        debug!(
            "Member [id = {}] created in Room [id = {}].",
            msg.0, self.id
        );
        Ok(())
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
    type Result = ActFuture<Result<(), RoomError>>;

    fn handle(
        &mut self,
        msg: CreateEndpoint,
        _: &mut Self::Context,
    ) -> Self::Result {
        match msg.spec {
            EndpointSpec::WebRtcPlay(endpoint) => {
                match self.create_sink_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    endpoint,
                ) {
                    Ok(fut) => Box::pin(fut),
                    Err(e) => Box::pin(fut::err(e)),
                }
            }
            EndpointSpec::WebRtcPublish(endpoint) => {
                if let Err(e) = self.create_src_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    &endpoint,
                ) {
                    Box::pin(fut::err(e))
                } else {
                    Box::pin(fut::ok(()))
                }
            }
        }
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
