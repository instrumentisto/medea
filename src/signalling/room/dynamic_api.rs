//! Handlers for messages sent via [Control API], i.e. dynamic [`Room`] pipeline
//! mutations.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::collections::{HashMap, HashSet};

use actix::{
    fut, ActorFuture as _, AsyncContext as _, Context,
    ContextFutureSpawner as _, Handler, Message, WrapFuture as _,
};
use medea_client_api_proto::{Event, Mid, PeerId};
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        endpoints::{
            WebRtcPlayEndpoint as WebRtcPlayEndpointSpec,
            WebRtcPublishEndpoint as WebRtcPublishEndpointSpec,
        },
        refs::StatefulFid,
        EndpointId, EndpointSpec, MemberId, MemberSpec, WebRtcPlayId,
        WebRtcPublishId,
    },
    log::prelude::*,
    media::{Peer, RenegotiationReason, Stable},
    signalling::{
        elements::{
            endpoints::webrtc::{WebRtcPlayEndpoint, WebRtcPublishEndpoint},
            member::MemberError,
        },
        room::ActFuture,
    },
    utils::actix_try_join_all,
};

use super::{Room, RoomError};

impl Room {
    /// Deletes [`Member`] from this [`Room`] by [`MemberId`].
    fn delete_member(&mut self, member_id: &MemberId, ctx: &mut Context<Self>) {
        debug!(
            "Deleting Member [id = {}] in Room [id = {}].",
            member_id, self.id
        );
        if let Some(member) = self.members.get_member_by_id(member_id) {
            let peers: HashSet<PeerId> = member
                .sinks()
                .values()
                .filter_map(WebRtcPlayEndpoint::peer_id)
                .chain(
                    member
                        .srcs()
                        .values()
                        .flat_map(WebRtcPublishEndpoint::peer_ids),
                )
                .collect();

            // Send PeersRemoved to `Member`s which have related to this
            // `Member` `Peer`s.
            self.remove_peers(&member.id(), &peers, ctx);

            self.members.delete_member(member_id, ctx);

            debug!(
                "Member [id = {}] deleted from Room [id = {}].",
                member_id, self.id
            );
        }
    }

    fn delete_src_endpoint(
        &mut self,
        src: &WebRtcPublishEndpoint,
    ) -> HashSet<(MemberId, PeerId)> {
        let mut affected_peers = HashSet::new();
        for sink in src.sinks() {
            affected_peers.extend(self.delete_sink_endpoint(&sink));
        }

        affected_peers
    }

    fn delete_sink_endpoint(
        &mut self,
        sink_endpoint: &WebRtcPlayEndpoint,
    ) -> HashSet<(MemberId, PeerId)> {
        let member = sink_endpoint.owner();
        let mut affected_peers = HashSet::new();
        let src_endpoint = sink_endpoint.src();
        if let Some(sink_peer_id) = sink_endpoint.peer_id() {
            let mut sink_peer: Peer<Stable> =
                self.peers.take_inner_peer(sink_peer_id).unwrap();
            let mut src_peer: Peer<Stable> = self
                .peers
                .take_inner_peer(sink_peer.partner_peer_id())
                .unwrap();

            let tracks_to_remove =
                src_endpoint.get_tracks_ids_by_peer_id(src_peer.id());
            sink_peer.remove_receivers(tracks_to_remove.clone());
            src_peer.remove_senders(tracks_to_remove);

            if sink_peer.is_empty() && src_peer.is_empty() {
                member.peers_removed(&[sink_peer_id]);
                affected_peers.insert((sink_peer.member_id(), sink_peer_id));
                affected_peers.insert((src_peer.member_id(), src_peer.id()));
            } else {
                let sink_peer = sink_peer
                    .start_renegotiation(RenegotiationReason::TracksRemoved);
                affected_peers.insert((sink_peer.member_id(), sink_peer_id));
                self.peers.add_peer(sink_peer);
                self.peers.add_peer(src_peer);
            }
        }

        affected_peers
    }

    /// Deletes endpoint from this [`Room`] by ID.
    fn delete_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: EndpointId,
        ctx: &mut Context<Self>,
    ) {
        if let Some(member) = self.members.get_member_by_id(member_id) {
            let play_id = endpoint_id.into();
            let affected_peers =
                if let Some(sink_endpoint) = member.take_sink(&play_id) {
                    self.delete_sink_endpoint(&sink_endpoint)
                } else {
                    let publish_id = String::from(play_id).into();

                    if let Some(src_endpoint) = member.take_src(&publish_id) {
                        self.delete_src_endpoint(&src_endpoint)
                    } else {
                        HashSet::new()
                    }
                };

            let mut removed_peers: HashMap<MemberId, HashSet<PeerId>> =
                HashMap::new();
            let mut removed_tracks: HashMap<
                MemberId,
                HashMap<PeerId, HashSet<Mid>>,
            > = HashMap::new();
            for (member_id, peer_id) in affected_peers {
                if let Ok(peer) = self.peers.get_peer_by_id(peer_id) {
                    removed_tracks
                        .entry(member_id)
                        .or_default()
                        .entry(peer_id)
                        .or_default()
                        .extend(peer.removed_tracks_mids());
                } else {
                    removed_peers.entry(member_id).or_default().insert(peer_id);
                };
            }

            let mut events = HashMap::new();

            for (member_id, peer_ids) in removed_peers {
                events.insert(
                    member_id,
                    Event::PeersRemoved {
                        // TODO: PeersRemoved HashSet
                        peer_ids: peer_ids.into_iter().collect(),
                    },
                );
            }

            for (member_id, remove_tracks) in removed_tracks {
                for (updated_peer_id, removed_mid) in remove_tracks {
                    events.insert(
                        member_id.clone(),
                        Event::TracksRemoved {
                            peer_id: updated_peer_id,
                            sdp_offer: None,
                            mids: removed_mid,
                        },
                    );
                }
            }

            let mut futs = Vec::new();
            for (member_id, event) in events {
                debug!(
                    "Event {:?} will be sent to the {} for Endpoint delete.",
                    event, member_id
                );
                futs.push(
                    self.members
                        .send_event_to_member(member_id, event)
                        .into_actor(self),
                );
            }
            ctx.spawn(actix_try_join_all(futs).then(|res, this, _| {
                debug!("Delete Endpoint task was finished!");
                res.unwrap();
                async {}.into_actor(this)
            }));
        }

        // debug!(
        //     "Endpoint [id = {}] removed in Member [id = {}] from Room [id = \
        //      {}].",
        //     endpoint_id, member_id, self.id
        // );
    }

    /// Creates new [`WebRtcPlayEndpoint`] in specified [`Member`].
    ///
    /// This function will check that new [`WebRtcPublishEndpoint`]'s ID is not
    /// present in [`ParticipantService`].
    ///
    /// Returns [`RoomError::EndpointAlreadyExists`] when
    /// [`WebRtcPublishEndpoint`]'s ID already presented in [`Member`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] was not found in [`ParticipantService`].
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
    /// present in [`ParticipantService`].
    ///
    /// # Errors
    ///
    /// Errors with [`RoomError::EndpointAlreadyExists`] if
    /// [`WebRtcPlayEndpoint`]'s ID already presented in [`Member`].
    ///
    /// Errors with [`RoomError::ParticipantServiceErr`] if [`Member`] with
    /// provided [`MemberId`] doesn't exist.
    fn create_sink_endpoint(
        &mut self,
        member_id: &MemberId,
        endpoint_id: WebRtcPlayId,
        spec: WebRtcPlayEndpointSpec,
        ctx: &mut Context<Self>,
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

        let src_member = src.owner();
        let sink_member = sink.owner();

        if let Some((src_peer_id, _)) = self
            .peers
            .get_peers_between_members(&src_member.id(), &sink_member.id())
        {
            self.peers.add_sink(src_peer_id, sink.clone());

            let renegotiate_peer =
                self.peers.start_renegotiation(src_peer_id)?;
            let renegotiate_peer_id = renegotiate_peer.id();
            let renegotiate_member_id = renegotiate_peer.member_id();
            let tracks_to_apply = renegotiate_peer.get_new_tracks();

            ctx.spawn(
                self.members
                    .send_event_to_member(
                        renegotiate_member_id,
                        Event::TracksAdded {
                            peer_id: renegotiate_peer_id,
                            sdp_offer: None,
                            tracks: tracks_to_apply,
                        },
                    )
                    .into_actor(self)
                    .then(move |_, this, _| async {}.into_actor(this)),
            );
        }

        debug!(
            "Created WebRtcPlayEndpoint [id = {}] for Member [id = {}] in \
             Room [id = {}].",
            sink.id(),
            member_id,
            self.id
        );

        member.insert_sink(sink);

        if self.members.member_has_connection(member_id) {
            Ok(Box::new(self.init_member_connections(&member)))
        } else {
            Ok(Box::new(actix::fut::ok(())))
        }
    }

    /// Removes [`Peer`]s and call [`Room::member_peers_removed`] for every
    /// [`Member`].
    ///
    /// This will delete [`Peer`]s from [`PeerRepository`] and send
    /// [`Event::PeersRemoved`] event to [`Member`].
    fn remove_peers<'a, Peers: IntoIterator<Item = &'a PeerId>>(
        &mut self,
        member_id: &MemberId,
        peer_ids_to_remove: Peers,
        ctx: &mut Context<Self>,
    ) {
        debug!("Remove peers.");
        self.peers
            .remove_peers(&member_id, peer_ids_to_remove)
            .into_iter()
            .for_each(|(member_id, peers)| {
                self.member_peers_removed(
                    peers.into_iter().map(|p| p.id()).collect(),
                    member_id,
                    ctx,
                )
                .map(|_, _, _| ())
                .spawn(ctx);
            });
    }
}

impl Into<proto::Room> for &Room {
    fn into(self) -> proto::Room {
        let pipeline = self
            .members
            .members()
            .into_iter()
            .map(|(id, member)| (id.to_string(), member.into()))
            .collect();
        proto::Room {
            id: self.id().to_string(),
            pipeline,
        }
    }
}

impl Into<proto::Element> for &Room {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::Room(self.into())),
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
                    warn!("Found Fid<IsRoomId> while deleting __from__ Room.")
                }
            }
        }
        member_ids.into_iter().for_each(|fid| {
            self.delete_member(&fid.member_id(), ctx);
        });
        endpoint_ids.into_iter().for_each(|fid| {
            let (_, member_id, endpoint_id) = fid.take_all();
            self.delete_endpoint(&member_id, endpoint_id, ctx);
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
        ctx: &mut Self::Context,
    ) -> Self::Result {
        match msg.spec {
            EndpointSpec::WebRtcPlay(endpoint) => {
                match self.create_sink_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    endpoint,
                    ctx,
                ) {
                    Ok(fut) => Box::new(fut),
                    Err(e) => Box::new(fut::err(e)),
                }
            }
            EndpointSpec::WebRtcPublish(endpoint) => {
                if let Err(e) = self.create_src_endpoint(
                    &msg.member_id,
                    msg.endpoint_id.into(),
                    &endpoint,
                ) {
                    Box::new(fut::err(e))
                } else {
                    Box::new(fut::ok(()))
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
    type Result = ();

    fn handle(&mut self, _: Close, ctx: &mut Self::Context) {
        for id in self.members.members().keys() {
            self.delete_member(id, ctx);
        }
        self.members
            .drop_connections(ctx)
            .into_actor(self)
            .wait(ctx);
    }
}
