use super::play_endpoint::Id as PlayerEndpointId;
use crate::{
    api::control::MemberId,
    media::{IceUser, PeerId},
};
use hashbrown::HashSet;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(String);

#[derive(Debug)]
pub struct WebRtcPublishEndpoint {
    id: Id,
    sinks: Vec<PlayerEndpointId>,
    owner: MemberId,
    ice_user: Option<Rc<IceUser>>,
    peer_ids: HashSet<PeerId>,
}

impl WebRtcPublishEndpoint {
    pub fn add_peer_id(&mut self, peer_id: PeerId) {
        self.peer_ids.insert(peer_id);
    }

    pub fn add_sink(&mut self, id: PlayerEndpointId) {
        self.sinks.push(id);
    }

    pub fn sinks(&self) -> Vec<&PlayerEndpointId> {
        self.sinks.iter().collect()
    }

    pub fn owner(&self) -> &MemberId {
        &self.owner
    }

    pub fn peer_ids(&self) -> &HashSet<PeerId> {
        &self.peer_ids
    }

    pub fn reset(&mut self) {
        self.peer_ids = HashSet::new();
    }

    pub fn remove_peer_id(&mut self, peer_id: &PeerId) -> bool {
        self.peer_ids.remove(peer_id)
    }

    pub fn remove_peer_ids(&mut self, peer_ids: &[PeerId]) {
        for peer_id in peer_ids {
            self.remove_peer_id(peer_id);
        }
    }
}
