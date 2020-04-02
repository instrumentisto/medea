use std::{cell::RefCell, rc::Rc};

use futures::Stream;
use medea_client_api_proto::{snapshots::room::RoomSnapshotAccessor, PeerId};
use medea_reactive::collections::ObservableHashMap;

use super::ObservablePeerSnapshot;
use medea_client_api_proto::snapshots::{
    peer::PeerSnapshotAccessor, room::RoomSnapshot,
};

#[derive(Debug)]
pub struct ObservableRoomSnapshot {
    pub(super) peers:
        ObservableHashMap<PeerId, Rc<RefCell<ObservablePeerSnapshot>>>,
}

impl ObservableRoomSnapshot {
    pub fn new() -> Self {
        Self {
            peers: ObservableHashMap::new(),
        }
    }
}

impl ObservableRoomSnapshot {
    pub fn on_peer_created(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<ObservablePeerSnapshot>>)> {
        self.peers.on_insert()
    }

    pub fn on_peer_removed(
        &self,
    ) -> impl Stream<Item = (PeerId, Rc<RefCell<ObservablePeerSnapshot>>)> {
        self.peers.on_remove()
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

    fn update_snapshot(&mut self, snapshot: RoomSnapshot) {
        for (peer_id, peer_snapshot) in snapshot.peers {
            if let Some(peer) = self.peers.get_mut(&peer_id) {
                peer.borrow_mut().update_snapshot(peer_snapshot)
            } else {
                todo!("Reset state");
            }
        }
    }
}
