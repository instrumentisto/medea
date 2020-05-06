//! [`Observable`] implementation of the [`RoomSnapshotAccessor`] which will be
//! used in the Jason for the `Room`'s real state updating.

use std::{cell::RefCell, rc::Rc};

use futures::Stream;
use medea_client_api_proto::{snapshots::room::RoomSnapshotAccessor, PeerId};
use medea_reactive::collections::ObservableHashMap;

use super::ObservablePeerSnapshot;

/// Reactive snapshot of the state for the `Room`.
#[derive(Debug)]
pub struct ObservableRoomSnapshot {
    /// All `Peer`s of this `Room`.
    pub peers: ObservableHashMap<PeerId, Rc<RefCell<ObservablePeerSnapshot>>>,
}

impl ObservableRoomSnapshot {
    /// Returns new empty [`ObservableRoomSnapshot`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns [`Stream`] to which will be sent reference to the newly created
    /// [`ObservablePeerSnapshot`]s.
    pub fn on_peer_created(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<ObservablePeerSnapshot>>)> {
        self.peers.on_insert()
    }

    /// Returns [`Stream`] to which will be sent references to the removed
    /// [`ObservablePeerSnapshot`]s.
    pub fn on_peer_removed(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<ObservablePeerSnapshot>>)> {
        self.peers.on_remove()
    }
}

impl Default for ObservableRoomSnapshot {
    fn default() -> Self {
        Self {
            peers: ObservableHashMap::new(),
        }
    }
}

impl RoomSnapshotAccessor for ObservableRoomSnapshot {
    type Peer = ObservablePeerSnapshot;

    fn insert_peer(&mut self, peer_id: PeerId, peer: Self::Peer) {
        self.peers.insert(peer_id, Rc::new(RefCell::new(peer)));
    }

    fn remove_peer(&mut self, peer_id: PeerId) {
        self.peers.remove(&peer_id);
    }

    /// Does nothing if [`PeerSnapshot`] with a provided [`PeerId`] not found.
    fn update_peer<F>(&mut self, peer_id: PeerId, update_fn: F)
    where
        F: FnOnce(Option<&mut Self::Peer>),
    {
        if let Some(peer) = self.peers.get(&peer_id) {
            (update_fn)(Some(&mut peer.borrow_mut()));
        } else {
            (update_fn)(None);
        }
    }
}
