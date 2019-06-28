use super::publish_endpoint::Id as PublishEndpointId;
use crate::{
    api::control::MemberId,
    media::{IceUser, PeerId},
};
use std::rc::Rc;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(String);

#[derive(Debug)]
pub struct WebRtcPlayEndpoint {
    id: Id,
    src: PublishEndpointId,
    owner: MemberId,
    ice_user: Option<Rc<IceUser>>,
    peer_id: Option<PeerId>,
}

impl WebRtcPlayEndpoint {
    pub fn src(&self) -> &PublishEndpointId {
        &self.src
    }

    pub fn owner(&self) -> MemberId {
        self.owner.clone()
    }

    pub fn is_connected(&self) -> bool {
        self.peer_id.is_some()
    }

    pub fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id)
    }

    pub fn peer_id(&self) -> Option<PeerId> {
        self.peer_id.clone()
    }

    pub fn reset(&mut self) {
        self.peer_id = None;
    }

    pub fn take_ice_user(&mut self) -> Option<Rc<IceUser>> {
        self.ice_user.take()
    }

    pub fn set_ice_user(&mut self, user: Rc<IceUser>) {
        self.ice_user = Some(user);
    }
}
