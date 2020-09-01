//! Implementation of the [`CommandHandler`] for the [`Room`] and related
//! definitions.

use std::collections::HashMap;

use medea_client_api_proto::{
    CommandHandler, Event, IceCandidate, NegotiationRole, PeerId, PeerMetrics,
    TrackId, TrackPatch,
};

use crate::{
    log::prelude::*,
    media::{Peer, PeerStateMachine, WaitLocalSdp, WaitRemoteSdp},
};

use super::{Room, RoomError};

impl CommandHandler for Room {
    type Output = Result<(), RoomError>;

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`Stable`] and will
    /// be changed to [`WaitLocalHaveRemote`].
    fn on_make_sdp_offer(
        &mut self,
        from_peer_id: PeerId,
        sdp_offer: String,
        mids: HashMap<TrackId, String>,
        senders_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        let mut from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        let to_peer: Peer<WaitRemoteSdp> =
            self.peers.take_inner_peer(from_peer.partner_peer_id())?;

        from_peer.set_mids(mids)?;
        from_peer.update_senders_statuses(senders_statuses);

        let from_peer = from_peer.set_local_offer(sdp_offer.clone());
        let to_peer = to_peer.set_remote_offer(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = to_peer.ice_servers_list().ok_or_else(|| {
            RoomError::NoTurnCredentials(to_member_id.clone())
        })?;

        let event = if from_peer.is_known_to_remote() {
            Event::TracksApplied {
                peer_id: to_peer.id(),
                negotiation_role: Some(NegotiationRole::Answerer(sdp_offer)),
                updates: to_peer.get_updates(),
            }
        } else {
            Event::PeerCreated {
                peer_id: to_peer.id(),
                negotiation_role: NegotiationRole::Answerer(sdp_offer),
                tracks: to_peer.new_tracks(),
                ice_servers,
                force_relay: to_peer.is_force_relayed(),
            }
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        self.peers.sync_peer_spec(from_peer_id)?;

        self.members.send_event_to_member(to_member_id, event)
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalHaveRemote`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    fn on_make_sdp_answer(
        &mut self,
        from_peer_id: PeerId,
        sdp_answer: String,
        senders_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        let from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        let to_peer: Peer<WaitRemoteSdp> =
            self.peers.take_inner_peer(from_peer.partner_peer_id())?;

        from_peer.update_senders_statuses(senders_statuses);

        let from_peer = from_peer.set_local_answer(sdp_answer.clone());
        let to_peer = to_peer.set_remote_answer(sdp_answer.clone());

        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer.id(),
            sdp_answer,
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        self.peers.sync_peer_spec(from_peer_id)?;

        self.members.send_event_to_member(to_member_id, event)
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`Stable`].
    fn on_set_ice_candidate(
        &mut self,
        from_peer_id: PeerId,
        candidate: IceCandidate,
    ) -> Self::Output {
        // TODO: add E2E test
        if candidate.candidate.is_empty() {
            warn!("Empty candidate from Peer: {}, ignoring", from_peer_id);
            return Ok(());
        }

        let to_peer_id = self
            .peers
            .map_peer_by_id(from_peer_id, PeerStateMachine::partner_peer_id)?;
        let to_member_id = self
            .peers
            .map_peer_by_id(to_peer_id, PeerStateMachine::member_id)?;
        let event = Event::IceCandidateDiscovered {
            peer_id: to_peer_id,
            candidate,
        };

        self.members.send_event_to_member(to_member_id, event)
    }

    /// Adds new [`Peer`] connection metrics.
    ///
    /// Passes [`PeerMetrics::RtcStats`] to [`PeersService`] for the further
    /// analysis.
    fn on_add_peer_connection_metrics(
        &mut self,
        peer_id: PeerId,
        metrics: PeerMetrics,
    ) -> Self::Output {
        if let PeerMetrics::RtcStats(stats) = &metrics {
            self.peers.add_stats(peer_id, stats);
        }
        Ok(())
    }

    /// Sends [`Event::TracksApplied`] with data from the received
    /// [`Command::UpdateTracks`].
    ///
    /// Starts renegotiation process.
    ///
    /// [`Command::UpdateTracks`]: medea_client_api_proto::Command::UpdateTracks
    fn on_update_tracks(
        &mut self,
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatch>,
    ) -> Self::Output {
        // Note, that we force committing changes to `Member` that send
        // `UpdateTracks` request, so response will be sent immediately,
        // regardless of that `Peer` state, but non-forcibly committing
        // changes to partner `Peer`, so it will be notified of changes only
        // during next negotiation.
        let partner_peer_id =
            self.peers.map_peer_by_id_mut(peer_id, |peer| {
                peer.as_changes_scheduler()
                    .patch_tracks(tracks_patches.clone());
                peer.force_commit_scheduled_changes();
                peer.partner_peer_id()
            })?;
        self.peers.map_peer_by_id_mut(partner_peer_id, |peer| {
            peer.as_changes_scheduler().patch_tracks(tracks_patches);
            peer.commit_scheduled_changes();
        })?;

        Ok(())
    }
}
