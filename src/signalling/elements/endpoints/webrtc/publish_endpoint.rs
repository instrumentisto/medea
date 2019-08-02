//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    collections::HashSet,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_grpc_proto::control::{
    Member_Element as ElementProto,
    WebRtcPublishEndpoint as WebRtcPublishEndpointProto,
};

use crate::{
    api::control::endpoints::webrtc_publish_endpoint::{
        P2pMode, WebRtcPublishId as Id,
    },
    signalling::elements::{
        endpoints::webrtc::play_endpoint::WeakWebRtcPlayEndpoint,
        member::WeakMember, Member,
    },
};

use super::play_endpoint::WebRtcPlayEndpoint;

#[derive(Debug, Clone)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<WeakWebRtcPlayEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: WeakMember,

    /// [`PeerId`] of all [`Peer`]s created for this [`WebRtcPublishEndpoint`].
    ///
    /// Currently this field used for nothing but in future this may be used
    /// while removing [`WebRtcPublishEndpoint`] for removing all [`Peer`]s of
    /// this [`WebRtcPublishEndpoint`].
    peer_ids: HashSet<PeerId>,
}

impl Drop for WebRtcPublishEndpointInner {
    fn drop(&mut self) {
        for receiver in self
            .sinks
            .iter()
            .filter_map(WeakWebRtcPlayEndpoint::safe_upgrade)
        {
            if let Some(receiver_owner) = receiver.weak_owner().safe_upgrade() {
                receiver_owner.remove_sink(&receiver.id())
            }
        }
    }
}

impl WebRtcPublishEndpointInner {
    fn add_sinks(&mut self, sink: WeakWebRtcPlayEndpoint) {
        self.sinks.push(sink);
    }

    fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.sinks
            .iter()
            .map(WeakWebRtcPlayEndpoint::upgrade)
            .collect()
    }

    fn owner(&self) -> Member {
        self.owner.upgrade()
    }

    fn add_peer_id(&mut self, peer_id: PeerId) {
        self.peer_ids.insert(peer_id);
    }

    fn peer_ids(&self) -> HashSet<PeerId> {
        self.peer_ids.clone()
    }

    fn reset(&mut self) {
        self.peer_ids = HashSet::new()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peer_ids.remove(peer_id);
    }

    fn remove_peer_ids(&mut self, peer_ids: &[PeerId]) {
        for peer_id in peer_ids {
            self.remove_peer_id(peer_id)
        }
    }
}

/// Signalling representation of `WebRtcPublishEndpoint`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint(Rc<RefCell<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    /// Create new [`WebRtcPublishEndpoint`].
    pub fn new(id: Id, p2p: P2pMode, owner: WeakMember) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            sinks: Vec::new(),
            owner,
            peer_ids: HashSet::new(),
        })))
    }

    /// Add sink for this [`WebRtcPublishEndpoint`].
    pub fn add_sink(&self, sink: WeakWebRtcPlayEndpoint) {
        self.0.borrow_mut().add_sinks(sink)
    }

    /// Returns all sinks of this [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if meet empty pointer.__
    pub fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.borrow().sinks()
    }

    /// Returns owner [`Member`] of this [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Add [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Reset state of this [`WebRtcPublishEndpoint`].
    ///
    /// Atm this only reset peer_ids.
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Remove [`PeerId`] from peer_ids.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.borrow_mut().remove_peer_id(peer_id)
    }

    /// Remove all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.borrow_mut().remove_peer_ids(peer_ids)
    }

    /// Returns ID of this [`WebRtcPublishEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Remove all empty Weak pointers from sinks of this
    /// [`WebRtcPublishEndpoint`].
    pub fn remove_empty_weaks_from_sinks(&self) {
        self.0
            .borrow_mut()
            .sinks
            .retain(|e| e.safe_upgrade().is_some());
    }

    pub fn p2p(&self) -> P2pMode {
        self.0.borrow().p2p.clone()
    }

    /// Downgrade [`WeakWebRtcPublishEndpoint`] to weak pointer
    /// [`WeakWebRtcPublishEndpoint`].
    pub fn downgrade(&self) -> WeakWebRtcPublishEndpoint {
        WeakWebRtcPublishEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares pointers. If both pointers point to the same address, then
    /// returns true.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_publish: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_publish.0)
    }
}

/// Weak pointer to [`WebRtcPublishEndpoint`].
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct WeakWebRtcPublishEndpoint(Weak<RefCell<WebRtcPublishEndpointInner>>);

impl WeakWebRtcPublishEndpoint {
    /// Upgrade weak pointer to strong pointer.
    ///
    /// This function will __panic__ if weak pointer is `None`.
    pub fn upgrade(&self) -> WebRtcPublishEndpoint {
        WebRtcPublishEndpoint(self.0.upgrade().unwrap())
    }

    /// Safe upgrade to [`WebRtcPlayEndpoint`].
    pub fn safe_upgrade(&self) -> Option<WebRtcPublishEndpoint> {
        self.0.upgrade().map(WebRtcPublishEndpoint)
    }
}

impl Into<ElementProto> for WebRtcPublishEndpoint {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut publish = WebRtcPublishEndpointProto::new();
        publish.set_p2p(self.p2p().into());
        element.set_webrtc_pub(publish);

        element
    }
}
