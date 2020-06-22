//! Implementation of the [`CommandHandler`] for the [`Room`] and related
//! definitions.

use std::collections::HashMap;

use actix::WrapFuture as _;
use futures::{
    future::{self, LocalBoxFuture},
    FutureExt,
};
use medea_client_api_proto::{
    CommandHandler, Event, IceCandidate, PeerConnectionState, PeerId,
    PeerMetrics, TrackId, TrackPatch,
};

use crate::{
    log::prelude::*,
    media::{
        peer::Stable, Peer, WaitLocalHaveRemote, WaitLocalSdp, WaitRemoteSdp,
    },
};

use super::{ActFuture, Room, RoomError};

impl Room {
    /// Updates specified [`Peer`] connection state.
    ///
    /// Initiates ICE restart if new connection state is
    /// [`PeerConnectionState::Failed`], previous connection state is
    /// [`PeerConnectionState::Connected`] or
    /// [`PeerConnectionState::Disconnected`] and connected [`Peer`] connection
    /// state is [`PeerConnectionState::Connected`] or
    /// [`PeerConnectionState::Disconnected`].
    pub fn update_peer_connection_state(
        &mut self,
        peer_id: PeerId,
        new_state: PeerConnectionState,
    ) -> LocalBoxFuture<'static, Result<(), RoomError>> {
        use PeerConnectionState as PeerConState;

        let peer = match self.peers.get_peer_by_id(peer_id) {
            Ok(peer) => peer,
            Err(err) => return future::err(err).boxed_local(),
        };

        let old_state: PeerConnectionState = peer.connection_state();

        // check whether state really changed
        if let (PeerConState::Failed, PeerConState::Disconnected) =
            (old_state, new_state)
        {
            // Failed => Disconnected is still Failed
            return future::ok(()).boxed_local();
        } else {
            peer.set_connection_state(new_state);
        }

        // maybe init ICE restart
        match new_state {
            PeerConState::Failed => match old_state {
                PeerConState::Connected | PeerConState::Disconnected => {
                    let connected_peer_state: PeerConnectionState =
                        match self.peers.get_peer_by_id(peer.partner_peer_id())
                        {
                            Ok(peer) => peer.connection_state(),
                            Err(err) => return future::err(err).boxed_local(),
                        };

                    if let PeerConState::Failed = connected_peer_state {
                        match self.peers.take_inner_peer::<Stable>(peer_id) {
                            Ok(peer) => {
                                let member_id = peer.member_id();
                                self.peers.add_peer(peer.start_renegotiation());

                                self.members.send_event_to_member(
                                    member_id,
                                    Event::RenegotiationStarted {
                                        peer_id,
                                        ice_restart: true,
                                    },
                                )
                            }
                            Err(err) => future::err(err).boxed_local(),
                        }
                    } else {
                        future::ok(()).boxed_local()
                    }
                }
                _ => future::ok(()).boxed_local(),
            },
            _ => future::ok(()).boxed_local(),
        }
    }
}

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
        senders_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        let mut from_peer: Peer<WaitLocalSdp> =
            self.peers.take_inner_peer(from_peer_id)?;
        from_peer.set_mids(mids)?;
        from_peer.update_senders_statuses(senders_statuses);

        let to_peer_id = from_peer.partner_peer_id();
        let to_peer: Peer<Stable> = self.peers.take_inner_peer(to_peer_id)?;

        let from_peer = from_peer.set_local_sdp(sdp_offer.clone());
        let to_peer = to_peer.set_remote_sdp(sdp_offer.clone());

        let to_member_id = to_peer.member_id();
        let ice_servers = to_peer.ice_servers_list().ok_or_else(|| {
            RoomError::NoTurnCredentials(to_member_id.clone())
        })?;

        let event = match from_peer.connection_state() {
            PeerConnectionState::Failed => Event::SdpOfferMade {
                peer_id: to_peer.id(),
                sdp_offer,
            },
            _ => Event::PeerCreated {
                peer_id: to_peer.id(),
                sdp_offer: Some(sdp_offer),
                tracks: to_peer.tracks(),
                ice_servers,
                force_relay: to_peer.is_force_relayed(),
            },
        };

        self.peers.add_peer(from_peer);
        self.peers.add_peer(to_peer);

        self.peers.sync_peer_spec(from_peer_id)?;

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
        senders_statuses: HashMap<TrackId, bool>,
    ) -> Self::Output {
        let from_peer: Peer<WaitLocalHaveRemote> =
            self.peers.take_inner_peer(from_peer_id)?;
        from_peer.update_senders_statuses(senders_statuses);

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

        self.peers.sync_peer_spec(from_peer_id)?;

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

        let to_peer_id =
            self.peers.get_peer_by_id(from_peer_id)?.partner_peer_id();
        let to_member_id = self.peers.get_peer_by_id(to_peer_id)?.member_id();
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

    /// Does nothing atm.
    fn on_add_peer_connection_metrics(
        &mut self,
        peer_id: PeerId,
        metrics: PeerMetrics,
    ) -> Self::Output {
        use PeerMetrics as PM;

        Ok(Box::new(
            match metrics {
                PM::IceConnectionState(state) => {
                    self.update_peer_connection_state(peer_id, state.into())
                }
                PM::PeerConnectionState(state) => {
                    self.update_peer_connection_state(peer_id, state)
                }
                PM::RtcStats(_) => future::ok(()).boxed_local(),
            }
            .into_actor(self),
        ))
    }

    /// Sends [`Event::TracksUpdated`] with data from the received
    /// [`Command::UpdateTracks`].
    ///
    /// [`Command::UpdateTracks`]: medea_client_api_proto::Command::UpdateTracks
    fn on_update_tracks(
        &mut self,
        peer_id: PeerId,
        tracks_patches: Vec<TrackPatch>,
    ) -> Self::Output {
        if let Ok(p) = self.peers.get_peer_by_id(peer_id) {
            let member_id = p.member_id();
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
