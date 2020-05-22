//! [`PeerConnectionStateEventsHandler`] implementation for [`Room`].

use actix::{Handler, Message, WeakAddr};
use chrono::{DateTime, Utc};
use medea_client_api_proto::PeerId;

use crate::signalling::{peers::PeerConnectionStateEventsHandler, Room};

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
    /// [`PeerId`] of the `Peer` which traffic was stopped.
    peer_id: PeerId,

    /// [`DateTime`] at which this `Peer` was stopped.
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
