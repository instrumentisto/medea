use futures::sync::mpsc::UnboundedSender;
use std::collections::HashMap;
use std::rc::Rc;

use medea_client_api_proto::IceServer;

use crate::peer::peer_con::{Id, PeerConnection, PeerEvent};
use crate::utils::WasmErr;

/// [`PeerConnection`] factory and repository.
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<Id, Rc<PeerConnection>>,

    /// Sender that will be injected to all [`Peers`] created by this
    /// repository.
    peer_events_sender: UnboundedSender<PeerEvent>,
}

impl PeerRepository {
    /// Creates new [`PeerRepository`] saving provided sender to be injected in
    /// all peers that will be created by this repository.
    pub fn new(peer_events_sender: UnboundedSender<PeerEvent>) -> Self {
        Self {
            peers: HashMap::new(),
            peer_events_sender,
        }
    }

    /// Creates new [`PeerConnection`] with provided id injecting provided ice
    /// servers and stored [`PeerEvent`] sender.
    pub fn create(
        &mut self,
        id: Id,
        ice_servers: Vec<IceServer>,
    ) -> Result<&Rc<PeerConnection>, WasmErr> {
        let peer = Rc::new(PeerConnection::new(
            id,
            &self.peer_events_sender,
            ice_servers,
        )?);
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).unwrap())
    }

    pub fn get_peer(&self, id: Id) -> Option<&Rc<PeerConnection>> {
        self.peers.get(&id)
    }

    pub fn remove(&mut self, id: Id) {
        self.peers.remove(&id);
    }
}
