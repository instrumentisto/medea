//! Implementation of the [`CommandHandler`] for the [`Room`] and related
//! definitions.

use std::collections::HashMap;

use medea_client_api_proto as proto;
use medea_client_api_proto::{
    CommandHandler, Credential, Event, IceCandidate, MemberId, NegotiationRole,
    PeerId, PeerMetrics, TrackId, TrackPatchCommand,
};

use crate::{
    log::prelude::*,
    media::{Peer, PeerStateMachine, WaitLocalSdp, WaitRemoteSdp},
};

use super::{Room, RoomError};

impl CommandHandler for Room {
    type Output = Result<(), RoomError>;

    #[inline]
    fn on_join_room(&mut self, _: MemberId, _: Credential) -> Self::Output {
        unreachable!("Room can't receive Command::JoinRoom")
    }

    #[inline]
    fn on_leave_room(&mut self, _: MemberId) -> Self::Output {
        unreachable!("Room can't receive Command::LeaveRoom")
    }

    /// Sends [`Event::PeerCreated`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`WaitRemoteSdp`], partners [`Peer`] state must be [`Stable`] and will
    /// be changed to [`WaitLocalSdp`].
    ///
    /// [`Stable`]: crate::media::peer::Stable
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

        let from_member_id = from_peer.member_id();
        let to_member_id = to_peer.member_id();
        let ice_servers = to_peer.ice_servers_list();

        let event = if from_peer.is_known_to_remote() {
            Event::PeerUpdated {
                peer_id: to_peer.id(),
                negotiation_role: Some(NegotiationRole::Answerer(
                    sdp_offer.clone(),
                )),
                updates: to_peer.get_updates(),
            }
        } else {
            Event::PeerCreated {
                peer_id: to_peer.id(),
                negotiation_role: NegotiationRole::Answerer(sdp_offer.clone()),
                tracks: to_peer.new_tracks(),
                ice_servers,
                force_relay: to_peer.is_force_relayed(),
            }
        };

        self.members.send_event_to_member(
            from_member_id,
            Event::LocalDescriptionApplied {
                peer_id: from_peer_id,
                sdp_offer,
            },
        );
        self.members.send_event_to_member(to_member_id, event);

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);
        self.peers.sync_peer_spec(from_peer_id)
    }

    /// Sends [`Event::SdpAnswerMade`] to provided [`Peer`] partner. Provided
    /// [`Peer`] state must be [`WaitLocalSdp`] and will be changed to
    /// [`Stable`], partners [`Peer`] state must be [`WaitRemoteSdp`] and will
    /// be changed to [`Stable`].
    ///
    /// [`Stable`]: crate::media::peer::Stable
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

        let from_member_id = from_peer.member_id();
        let to_member_id = to_peer.member_id();
        let event = Event::SdpAnswerMade {
            peer_id: to_peer.id(),
            sdp_answer: sdp_answer.clone(),
        };

        self.members.send_event_to_member(
            from_member_id,
            Event::LocalDescriptionApplied {
                peer_id: from_peer_id,
                sdp_offer: sdp_answer,
            },
        );
        self.members.send_event_to_member(to_member_id, event);

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);
        self.peers.sync_peer_spec(from_peer_id)
    }

    /// Sends [`Event::IceCandidateDiscovered`] to provided [`Peer`] partner.
    /// Both [`Peer`]s may have any state except [`Stable`].
    ///
    /// Adds [`IceCandidate`] to the [`Peer`].
    ///
    /// [`Stable`]: crate::media::peer::Stable
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

        let peer_id = self
            .peers
            .map_peer_by_id(from_peer_id, PeerStateMachine::partner_peer_id)?;

        self.peers.map_peer_by_id_mut(peer_id, |to_peer| {
            to_peer.add_ice_candidate(candidate.clone());

            self.members.send_event_to_member(
                to_peer.member_id(),
                Event::IceCandidateDiscovered { peer_id, candidate },
            );
        })
    }

    /// Adds new [`Peer`] connection metrics.
    ///
    /// Passes [`PeerMetrics::RtcStats`] to [`PeersService`] for the further
    /// analysis.
    ///
    /// [`PeersService`]: crate::signalling::peers::PeersService
    fn on_add_peer_connection_metrics(
        &mut self,
        peer_id: PeerId,
        metrics: PeerMetrics,
    ) -> Self::Output {
        match metrics {
            PeerMetrics::RtcStats(ref stats) => {
                self.peers.add_stats(peer_id, stats);
            }
            PeerMetrics::PeerConnectionState(state) => {
                self.peers.update_peer_connection_state(peer_id, state);
            }
            PeerMetrics::IceConnectionState(state) => {
                self.peers
                    .update_peer_connection_state(peer_id, state.into());
            }
        }
        Ok(())
    }

    /// Sends [`Event::PeerUpdated`] with data from the received
    /// [`Command::UpdateTracks`].
    ///
    /// Starts renegotiation process.
    ///
    /// [`Command::UpdateTracks`]: medea_client_api_proto::Command::UpdateTracks
    fn on_update_tracks(
        &mut self,
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatchCommand>,
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
            peer.as_changes_scheduler()
                .partner_patch_tracks(tracks_patches);
            if !peer.commit_scheduled_changes()
                && peer.can_forcibly_commit_partner_patches()
            {
                peer.force_commit_partner_changes();
            }
        })?;

        Ok(())
    }

    fn on_synchronize_me(&mut self, _: proto::state::Room) -> Self::Output {
        unreachable!("Room can't receive Command::SynchronizeMe")
    }
}
