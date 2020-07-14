//! [`PeerConnectionStateEventsHandler`] implementation for [`Room`].

use actix::{fut, Handler, Message, WeakAddr};
use chrono::{DateTime, Utc};
use medea_client_api_proto::PeerId;

use crate::{
    media::{peer::NegotiationSubscriber, Peer, PeerStateMachine, Stable},
    signalling::{
        peers::PeerConnectionStateEventsHandler,
        room::{ActFuture, RoomError},
        Room,
    },
};

impl PeerConnectionStateEventsHandler for WeakAddr<Room> {
    /// Upgrades [`WeakAddr`] of the [`Room`] and sends [`PeerStarted`]
    /// message to [`Room`] [`Addr`].
    fn peer_started(&self, peer_id: PeerId) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(PeerStarted(peer_id));
        }
    }

    /// Upgrades [`WeakAddr`] of the [`Room`] and sends [`PeerStopped`]
    /// message to [`Room`] [`Addr`].
    fn peer_stopped(&self, peer_id: PeerId, at: DateTime<Utc>) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(PeerStopped { peer_id, at })
        }
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

impl NegotiationSubscriber for WeakAddr<Room> {
    /// Upgrades [`WeakAddr`] and if it successful then sends to the
    /// upgraded [`Addr`] [`NegotiationNeeded`] [`Message`].
    ///
    /// If [`WeakAddr`] upgrade fails then nothing will be done.
    fn negotiation_needed(&self, peer_id: PeerId) {
        if let Some(addr) = self.upgrade() {
            addr.do_send(NegotiationNeeded(peer_id));
        }
    }
}

/// [`Message`] which indicates that [`Peer`] with a provided [`PeerId`] should
/// be renegotiated.
///
/// If provided [`Peer`] or it's partner [`Peer`] will be not [`Stable`] then
/// nothing should be done.
#[derive(Message, Clone, Debug, Copy)]
#[rtype(result = "Result<(), RoomError>")]
pub struct NegotiationNeeded(pub PeerId);

impl Handler<NegotiationNeeded> for Room {
    type Result = ActFuture<Result<(), RoomError>>;

    /// Starts negotiation for the [`Peer`] with provided [`PeerId`].
    ///
    /// Sends [`Event::PeerCreated`] if this [`Peer`] unknown for the remote
    /// side.
    ///
    /// Sends [`Event::TrackApplied`] if this [`Peer`] known for the remote
    /// side.
    ///
    /// If this [`Peer`] or it's partner not [`Stable`] then nothing will be
    /// done.
    fn handle(
        &mut self,
        msg: NegotiationNeeded,
        _: &mut Self::Context,
    ) -> Self::Result {
        let peer_id = msg.0;
        actix_try!(self.peers.update_peer_tracks(peer_id));

        // Make sure that both peers are in stable state, if that is not the
        // case then we just skip this iteration, and wait for next
        // proc.
        let peer: Peer<Stable> =
            if let Ok(peer) = self.peers.take_inner_peer(msg.0) {
                peer
            } else {
                return Box::new(fut::ok(()));
            };
        let is_partner_stable = match self
            .peers
            .map_peer_by_id(peer.partner_peer_id(), PeerStateMachine::is_stable)
        {
            Ok(r) => r,
            Err(e) => {
                self.peers.add_peer(peer);

                return Box::new(fut::err(e));
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
            Box::new(fut::ok(()))
        }
    }
}
