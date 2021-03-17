//! [`PeerConnectionStateEventsHandler`] implementation for [`Room`].

use actix::{Handler, Message, StreamHandler, WeakAddr};
use chrono::{DateTime, Utc};
use medea_client_api_proto::{
    ConnectionQualityScore, Event, MemberId, NegotiationRole, PeerId,
    TrackUpdate,
};

use crate::{
    api::control::callback::{MediaDirection, MediaType},
    log::prelude::*,
    media::{peer::PeerUpdatesSubscriber, Peer, PeerStateMachine, Stable},
    signalling::{
        peers::{
            PeerConnectionStateEventsHandler, PeersMetricsEvent,
            PeersMetricsEventHandler,
        },
        room::RoomError,
        Room,
    },
};

impl Room {
    /// Sends [`Event::PeerCreated`] specified [`Peer`]. That [`Peer`] state
    /// will be changed to a [`WaitLocalSdp`] state.
    ///
    /// [`WaitLocalSdp`]: crate::media::peer::WaitLocalSdp
    fn send_peer_created(&self, peer_id: PeerId) -> Result<(), RoomError> {
        let peer: Peer<Stable> = self.peers.take_inner_peer(peer_id)?;
        let partner_peer: Peer<Stable> =
            match self.peers.take_inner_peer(peer.partner_peer_id()) {
                Ok(p) => p,
                Err(e) => {
                    self.peers.add_peer(peer);
                    return Err(e);
                }
            };

        let member_id = peer.member_id();
        let ice_servers = if let Some(ice_servers) = peer.ice_servers_list() {
            ice_servers
        } else {
            self.peers.add_peer(peer);
            self.peers.add_peer(partner_peer);
            return Err(RoomError::NoTurnCredentials(member_id));
        };

        let peer = peer.start_as_offerer();
        let partner_peer = partner_peer.start_as_answerer();

        let peer_created = Event::PeerCreated {
            peer_id: peer.id(),
            negotiation_role: NegotiationRole::Offerer,
            tracks: peer.new_tracks(),
            ice_servers,
            force_relay: peer.is_force_relayed(),
        };

        self.peers.add_peer(peer);
        self.peers.add_peer(partner_peer);

        self.members.send_event_to_member(member_id, peer_created)
    }
}

impl PeerConnectionStateEventsHandler for WeakAddr<Room> {
    /// Upgrades [`WeakAddr`] of the [`Room`] and notifies [`Room`] that
    /// [`Peer`] with the provided `peer_id` has started.
    fn peer_started(&self, peer_id: PeerId) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(PeerStarted(peer_id));
        }
    }

    /// Upgrades [`WeakAddr`] of the [`Room`] and notifies [`Room`] that
    /// [`Peer`] with the provided `peer_id` has stopped.
    fn peer_stopped(&self, peer_id: PeerId, at: DateTime<Utc>) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(PeerStopped { peer_id, at })
        }
    }
}

impl StreamHandler<PeersMetricsEvent> for Room {
    /// Dispatches received [`PeersMetricsEvent`] with [`Room`]'s
    /// [`PeersMetricsEventHandler`] implementation.
    fn handle(&mut self, event: PeersMetricsEvent, _: &mut Self::Context) {
        if let Err(err) = event.dispatch_with(self) {
            error!("Error handling PeersMetricsEvent: {:?}", err);
        }
    }
}

impl PeersMetricsEventHandler for Room {
    type Output = Result<(), RoomError>;

    /// Does nothing atm.
    fn on_no_traffic_flow(
        &mut self,
        _: PeerId,
        _: DateTime<Utc>,
        _: MediaType,
        _: MediaDirection,
    ) -> Self::Output {
        Ok(())
    }

    /// Does nothing atm.
    fn on_traffic_flows(
        &mut self,
        _: PeerId,
        _: MediaType,
        _: MediaDirection,
    ) -> Self::Output {
        Ok(())
    }

    /// Sends received [`ConnectionQualityScore`] to member.
    fn on_quality_meter_update(
        &mut self,
        member_id: MemberId,
        partner_member_id: MemberId,
        quality_score: ConnectionQualityScore,
    ) -> Self::Output {
        self.members.send_event_to_member(
            member_id,
            Event::ConnectionQualityUpdated {
                partner_member_id,
                quality_score,
            },
        )
    }

    /// Schedules ICE restart and commits scheduled changes.
    fn on_peer_connection_failed(&mut self, peer_id: PeerId) -> Self::Output {
        debug!("PeerConnection failed [peer_id = {}].", peer_id);
        self.peers.map_peer_by_id_mut(peer_id, |peer| {
            peer.as_changes_scheduler().restart_ice();
        })?;
        self.peers.commit_scheduled_changes(peer_id)?;

        Ok(())
    }
}

/// Message which indicates that `Peer` with provided [`PeerId`] has started.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct PeerStarted(pub PeerId);

/// Message which indicates that `Peer` with provided [`PeerId`] has stopped.
#[derive(Debug, Message)]
#[rtype(result = "()")]
struct PeerStopped {
    /// ID of the `Peer` which traffic was stopped.
    peer_id: PeerId,

    /// [`DateTime`] when this `Peer` was stopped.
    at: DateTime<Utc>,
}

impl Handler<PeerStarted> for Room {
    type Result = ();

    fn handle(
        &mut self,
        _: PeerStarted,
        _: &mut Self::Context,
    ) -> Self::Result {
        // TODO: Implement PeerStarted logic.
    }
}

impl Handler<PeerStopped> for Room {
    type Result = ();

    fn handle(
        &mut self,
        _: PeerStopped,
        _: &mut Self::Context,
    ) -> Self::Result {
        // TODO: Implement PeerStopped logic.
    }
}

impl PeerUpdatesSubscriber for WeakAddr<Room> {
    /// Upgrades [`WeakAddr`] and if it's successful then notifies [`Room`] that
    /// the provided [`Peer`] must be negotiated.
    ///
    /// If [`WeakAddr`] upgrade fails then nothing will be done.
    #[inline]
    fn negotiation_needed(&self, peer_id: PeerId) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(NegotiationNeeded(peer_id));
        }
    }

    /// Upgrades [`WeakAddr`] and if it's successful then notifies [`Room`] that
    /// the provided [`TrackUpdate`]s were forcibly (without negotiation)
    /// applied to the [`Peer`].
    ///
    /// If [`WeakAddr`] upgrade fails then nothing will be done.
    #[inline]
    fn force_update(&self, peer_id: PeerId, changes: Vec<TrackUpdate>) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(ForceUpdate(peer_id, changes));
        }
    }
}

/// [`Message`] which indicates that [`Peer`] with a provided [`PeerId`] should
/// be updated with provided [`TrackUpdate`]s without negotiation.
///
/// Can be done in any [`Peer`] state.
#[derive(Message, Clone, Debug)]
#[rtype(result = "Result<(), RoomError>")]
pub struct ForceUpdate(PeerId, Vec<TrackUpdate>);

impl Handler<ForceUpdate> for Room {
    type Result = Result<(), RoomError>;

    /// Gets [`MemberId`] of the provided [`Peer`] and sends all provided
    /// [`TrackUpdate`]s to this [`MemberId`] with `negotiation_role: None`.
    fn handle(
        &mut self,
        msg: ForceUpdate,
        _: &mut Self::Context,
    ) -> Self::Result {
        let member_id = self
            .peers
            .map_peer_by_id(msg.0, PeerStateMachine::member_id)?;
        self.members.send_event_to_member(
            member_id,
            Event::TracksApplied {
                peer_id: msg.0,
                updates: msg.1,
                negotiation_role: None,
            },
        )
    }
}

/// [`Message`] which indicates that [`Peer`] with a provided [`PeerId`] should
/// be renegotiated.
///
/// If provided [`Peer`] or it's partner [`Peer`] are not in a [`Stable`] state,
/// then nothing should be done.
#[derive(Message, Clone, Debug, Copy)]
#[rtype(result = "Result<(), RoomError>")]
pub struct NegotiationNeeded(pub PeerId);

impl Handler<NegotiationNeeded> for Room {
    type Result = Result<(), RoomError>;

    /// Starts negotiation for the [`Peer`] with provided [`PeerId`].
    ///
    /// Sends [`Event::PeerCreated`] if this [`Peer`] unknown for the remote
    /// side.
    ///
    /// Sends [`Event::TracksApplied`] if this [`Peer`] known for the remote
    /// side.
    ///
    /// If this [`Peer`] or it's partner not [`Stable`] then forcible
    /// track changes will be committed.
    ///
    /// [`Event::PeerCreated`]: medea_client_api_proto::Event::PeerCreated
    /// [`Event::TracksApplied`]: medea_client_api_proto::Event::TracksApplied
    fn handle(
        &mut self,
        msg: NegotiationNeeded,
        _: &mut Self::Context,
    ) -> Self::Result {
        let peer_id = msg.0;
        self.peers.update_peer_tracks(peer_id)?;

        // Make sure that both peers are in stable state, if that is not the
        // case then we just skip this iteration, and wait for next
        // proc.
        let peer: Peer<Stable> =
            if let Ok(peer) = self.peers.take_inner_peer(msg.0) {
                peer
            } else {
                return Ok(());
            };
        let is_partner_stable = match self
            .peers
            .map_peer_by_id(peer.partner_peer_id(), PeerStateMachine::is_stable)
        {
            Ok(r) => r,
            Err(e) => {
                self.peers.add_peer(peer);

                return Err(e);
            }
        };
        let is_known_to_remote = peer.is_known_to_remote();
        self.peers.add_peer(peer);

        if is_partner_stable {
            if is_known_to_remote {
                self.send_tracks_applied(peer_id)
            } else {
                self.send_peer_created(peer_id)
            }
        } else {
            Ok(())
        }
    }
}
