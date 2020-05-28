//! Implementation of the [`CommandHandler`] for the [`Room`] and related
//! definitions.

use std::collections::HashMap;

use actix::WrapFuture as _;
use medea_client_api_proto::{
    CommandHandler, Event, IceCandidate, PeerId, PeerMetrics, TrackId,
    TrackPatch,
};

use crate::{
    log::prelude::*,
    media::{
        New, Peer, PeerError, PeerStateMachine, WaitLocalHaveRemote,
        WaitLocalSdp, WaitRemoteSdp,
    },
};

use super::{ActFuture, Room, RoomError};
use crate::{
    api::control::callback::MediaType,
    signalling::elements::endpoints::Endpoint,
};

impl CommandHandler for Room {
    type Output = Result<ActFuture<Result<(), RoomError>>, RoomError>;

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`New`] and will be
    /// changed to [`WaitLocalHaveRemote`].
    fn on_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
        mids: HashMap<TrackId, String>,
    ) -> Self::Output {
        let mut from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        from_peer.set_mids(mids)?;

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<New> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = to_peer.ice_servers_list().ok_or_else(|| {
            RoomError::NoTurnCredentials(to_member_id.clone())
        })?;

        let event = Event::PeerCreated {
            peer_id: to_peer.id(),
            sdp_offer: Some(sdp_offer),
            tracks: to_peer.tracks(),
            ice_servers,
            force_relay: to_peer.is_force_relayed(),
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalHaveRemote`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    ///
    /// [`Stable`]: crate::media::peer::Stable
    fn on_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
    ) -> Self::Output {
        let from_peer: Peer<WaitLocalHaveRemote> =
            self.peers.take_inner_peer(from_peer_id)?;

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<WaitRemoteSdp> =
            self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_answer.clone());
        let to_peer = to_peer.set_remote_sdp(&sdp_answer);

        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer_id,
            sdp_answer,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`New`].
    fn on_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Self::Output {
        // TODO: add E2E test
        if candidate.candidate.is_empty() {
            warn!("Empty candidate from Peer: {}, ignoring", from_peer_id);
            return Ok(Box::new(actix::fut::ok(())));
        }

        let from_peer = self.peers.get_peer_by_id(from_peer_id)?;
        if let PeerStateMachine::New(_) = from_peer {
            return Err(PeerError::WrongState(
                from_peer_id,
                "Not New",
                format!("{}", from_peer),
            )
            .into());
        }

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer = self.peers.get_peer_by_id(to_peer_id)?;
        if let PeerStateMachine::New(_) = to_peer {
            return Err(PeerError::WrongState(
                to_peer_id,
                "Not New",
                format!("{}", to_peer),
            )
            .into());
        }

        let to_member_id = to_peer.member_id();
        let event = Event::IceCandidateDiscovered {
            peer_id: to_peer_id,
            candidate,
        };

        Ok(Box::new(
            self.members
                .send_event_to_member(to_member_id, event)
                .into_actor(self),
        ))
    }

    /// Provides received [`PeerMetrics::RtcStats`] into [`PeerMetricsService`].
    fn on_add_peer_connection_metrics(
        &mut self,
        peer_id: PeerId,
        metrics: PeerMetrics,
    ) -> Self::Output {
        match metrics {
            PeerMetrics::RtcStats(stats) => {
                self.peers.add_stats(peer_id, stats);
            }
            PeerMetrics::IceConnectionState(_)
            | PeerMetrics::PeerConnectionState(_) => {
                // TODO: implemented in ice-restart
            }
        }

        Ok(Box::new(actix::fut::ok(())))
    }

    /// Sends [`Event::TracksUpdated`] with data from the received
    /// [`Command::UpdateTracks`].
    ///
    /// Updates [`Peer`]'s [`MediaTrack`]s with provided [`TrackPatch`]s.
    ///
    /// Updates [`Endpoint`]s mute state accordingly to updated [`Peer`].
    ///
    /// Updates [`PeerTracks`] of the [`PeerMetricsService`] by updated
    /// [`Peer`].
    fn on_update_tracks(
        &mut self,
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatch>,
    ) -> Self::Output {
        debug!("Updating Peer [id = {}] tracks.", peer_id);
        if let Ok(peer) = self.peers.get_peer_by_id(peer_id) {
            tracks_patches
                .iter()
                .for_each(|patch| peer.update_track(patch));
            let member_id = peer.member_id();

            for weak_endpoint in peer.endpoints() {
                if let Some(endpoint) = weak_endpoint.upgrade() {
                    if let Endpoint::WebRtcPublishEndpoint(publish) = endpoint {
                        if peer.is_senders_muted(MediaType::Audio) {
                            publish.mute_audio();
                        } else {
                            publish.unmute_audio();
                        }
                        if peer.is_senders_muted(MediaType::Video) {
                            publish.mute_video();
                        } else {
                            publish.unmute_video();
                        }
                    }
                }
            }

            self.peers.update_peer_tracks(peer_id);

            Ok(Box::new(
                self.members
                    .send_event_to_member(
                        member_id,
                        Event::TracksUpdated {
                            peer_id,
                            tracks_patches,
                        },
                    )
                    .into_actor(self),
            ))
        } else {
            Ok(Box::new(actix::fut::ok(())))
        }
    }
}
