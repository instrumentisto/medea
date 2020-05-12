//! Handlers for [`PeerStarted`] and [`PeerStopped`] messages emitted by [`PeerTrafficWatcher`].
//!
//! [`PeerTrafficWatcher`]: crate::signalling::peers::PeerTrafficWatcher

use actix::Handler;

use crate::signalling::peers::{PeerStarted, PeerStopped};

use super::Room;

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
