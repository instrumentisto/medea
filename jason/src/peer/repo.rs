use futures::sync::mpsc::UnboundedSender;
use std::{collections::HashMap, rc::Rc};

use medea_client_api_proto::IceServer;

use crate::{
    media::MediaManager,
    peer::{PeerConnection, PeerEvent, PeerId},
    utils::WasmErr,
};

/// [`PeerConnection`] factory and repository.
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<PeerId, Rc<PeerConnection>>,

    /// Sender that will be injected to all [`Peers`] created by this
    /// repository.
    peer_events_sender: UnboundedSender<PeerEvent>,

    media_manager: Rc<MediaManager>,
}

impl PeerRepository {
    /// Creates new [`PeerRepository`] saving provided sender to be injected in
    /// all peers that will be created by this repository.
    pub fn new(
        peer_events_sender: UnboundedSender<PeerEvent>,
        media_manager: Rc<MediaManager>,
    ) -> Self {
        Self {
            peers: HashMap::new(),
            peer_events_sender,
            media_manager,
        }
    }

    /// Creates new [`PeerConnection`] with provided id injecting provided ice
    /// servers and stored [`PeerEvent`] sender.
    pub fn create(
        &mut self,
        id: PeerId,
        ice_servers: Vec<IceServer>,
    ) -> Result<&Rc<PeerConnection>, WasmErr> {
        let peer = Rc::new(PeerConnection::new(
            id,
            self.peer_events_sender.clone(),
            ice_servers,
            Rc::clone(&self.media_manager),
        )?);
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).unwrap())
    }

    pub fn get_peer(&self, id: PeerId) -> Option<&Rc<PeerConnection>> {
        self.peers.get(&id)
    }

    pub fn remove(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }
}
