//! [`Observable`] implementation of the [`RoomSnapshotAccessor`] which will be
//! used in the Jason for the `Room`'s real state updating.

use std::{cell::RefCell, rc::Rc};

use futures::Stream;
use medea_client_api_proto::{
    snapshots::{room::RoomSnapshotAccessor, PeerSnapshot, RoomSnapshot},
    Command, PeerId,
};
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

    /// Stores provided intention [`Command`] in the [`RoomSnapshot`].
    ///
    /// When RPC will be reconnected and state will be restored then this user
    /// intentions will be sent.
    #[allow(clippy::single_match)]
    pub fn fill_intentions(&mut self, cmd: &Command) {
        match cmd {
            Command::UpdateTracks {
                peer_id,
                tracks_patches,
            } => {
                if let Some(peer) = self.peers.get(peer_id) {
                    let peer_ref = peer.borrow();
                    for track_patch in tracks_patches {
                        if let Some(track) =
                            peer_ref.tracks.get(&track_patch.id)
                        {
                            let mut track_ref = track.borrow_mut();
                            if let Some(is_muted) = track_patch.is_muted {
                                if track_ref.is_muted.get() != is_muted {
                                    track_ref.intent.is_muted = Some(is_muted);
                                }
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }

    /// Returns intention [`Command`]s which user requested while RPC
    /// reconnecting.
    pub fn get_intents(&self) -> Vec<Command> {
        let mut commands = Vec::new();
        for peer in self.peers.values() {
            let peer_ref = peer.borrow();
            commands.append(&mut peer_ref.get_intents());
        }

        commands
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

impl From<&ObservableRoomSnapshot> for RoomSnapshot {
    fn from(from: &ObservableRoomSnapshot) -> Self {
        Self {
            peers: from
                .peers
                .iter()
                .map(|(id, peer)| (*id, PeerSnapshot::from(&*peer.borrow())))
                .collect(),
        }
    }
}
