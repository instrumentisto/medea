use std::{collections::HashMap, rc::Rc};

use futures::sync::mpsc::UnboundedSender;
use medea_client_api_proto::IceServer;

use crate::{media::MediaManager, utils::WasmErr};

use super::{PeerConnection, PeerEvent, PeerId};

/// [`PeerConnection`] factory and repository.
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    /// Peer id to [`PeerConnection`],
    peers: HashMap<PeerId, Rc<PeerConnection>>,

    /// Sender that will be injected into all [`PeerConnection`]s
    /// created by this repository.
    peer_events_sender: UnboundedSender<PeerEvent>,

    /// [`MediaManager`] that will be injected into all [`PeerConnection`]s
    /// created by this repository
    media_manager: Rc<MediaManager>,
}

impl PeerRepository {
    /// Creates new [`PeerRepository`].
    #[inline]
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

    /// Creates new [`PeerConnection`] with provided ID and injecting provided
    /// [`IceServer`]s, stored [`PeerEvent`] sender and [`MediaManager`].
    pub fn create<I: IntoIterator<Item = IceServer>>(
        &mut self,
        id: PeerId,
        ice_servers: I,
    ) -> Result<&Rc<PeerConnection>, WasmErr> {
        web_sys::console::log_1(&format!("Created Peer [id = {}].", id).into());
        let peer = Rc::new(PeerConnection::new(
            id,
            self.peer_events_sender.clone(),
            ice_servers,
            Rc::clone(&self.media_manager),
        )?);
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).unwrap())
    }

    pub fn peers(&self) -> HashMap<PeerId, Rc<PeerConnection>> {
        self.peers.clone()
    }

    /// Returns [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn get(&self, id: PeerId) -> Option<&Rc<PeerConnection>> {
        self.peers.get(&id)
    }

    /// Removes [`PeerConnection`] stored in repository by its ID.
    #[inline]
    pub fn remove(&mut self, id: PeerId) {
        self.peers.remove(&id);
    }
}
