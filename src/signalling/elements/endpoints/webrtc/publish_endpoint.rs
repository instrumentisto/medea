//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    collections::HashSet,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api::{
    Element as RootElementProto, Member_Element as ElementProto,
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

#[derive(Clone, Debug)]
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

/// Signalling representation of [`WebRtcPublishEndpoint`].
///
/// [`WebRtcPublishEndpoint`]:
/// crate::api::control::endpoints::WebRtcPublishEndpoint
#[derive(Debug, Clone)]
pub struct WebRtcPublishEndpoint(Rc<RefCell<WebRtcPublishEndpointInner>>);

impl WebRtcPublishEndpoint {
    /// Creates new [`WebRtcPublishEndpoint`].
    pub fn new(id: Id, p2p: P2pMode, owner: WeakMember) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            sinks: Vec::new(),
            owner,
            peer_ids: HashSet::new(),
        })))
    }

    /// Adds [`WebRtcPlayEndpoint`] (sink) to this [`WebRtcPublishEndpoint`].
    pub fn add_sink(&self, sink: WeakWebRtcPlayEndpoint) {
        self.0.borrow_mut().add_sinks(sink)
    }

    /// Returns all [`WebRtcPlayEndpoint`]s (sinks) of this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If meets empty pointer.
    pub fn sinks(&self) -> Vec<WebRtcPlayEndpoint> {
        self.0.borrow().sinks()
    }

    /// Returns owner [`Member`] of this [`WebRtcPublishEndpoint`].
    ///
    /// # Panics
    ///
    /// If pointer to [`Member`] has been dropped.
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Adds [`PeerId`] of this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().add_peer_id(peer_id)
    }

    /// Returns all [`PeerId`]s of this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
    }

    /// Resets state of this [`WebRtcPublishEndpoint`].
    ///
    /// _Atm this only resets `peer_ids`._
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Removes [`PeerId`] from this [`WebRtcPublishEndpoint`]'s `peer_ids`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn remove_peer_id(&self, peer_id: &PeerId) {
        self.0.borrow_mut().remove_peer_id(peer_id)
    }

    /// Removes all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn remove_peer_ids(&self, peer_ids: &[PeerId]) {
        self.0.borrow_mut().remove_peer_ids(peer_ids)
    }

    /// Returns [`Id`] of this [`WebRtcPublishEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Removes all dropped [`Weak`] pointers from sinks of this
    /// [`WebRtcPublishEndpoint`].
    pub fn remove_empty_weaks_from_sinks(&self) {
        self.0
            .borrow_mut()
            .sinks
            .retain(|e| e.safe_upgrade().is_some());
    }

    /// Peer-to-peer mode of this [`WebRtcPublishEndpoint`].
    pub fn p2p(&self) -> P2pMode {
        self.0.borrow().p2p.clone()
    }

    /// Downgrades [`WebRtcPublishEndpoint`] to weak pointer
    /// [`WeakWebRtcPublishEndpoint`].
    pub fn downgrade(&self) -> WeakWebRtcPublishEndpoint {
        WeakWebRtcPublishEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPublishEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_publish: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_publish.0)
    }
}

/// Weak pointer to [`WebRtcPublishEndpoint`].
#[derive(Clone, Debug)]
pub struct WeakWebRtcPublishEndpoint(Weak<RefCell<WebRtcPublishEndpointInner>>);

impl WeakWebRtcPublishEndpoint {
    /// Upgrades weak pointer to strong pointer.
    ///
    /// # Panics
    ///
    /// If weak pointer was dropped.
    pub fn upgrade(&self) -> WebRtcPublishEndpoint {
        WebRtcPublishEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns `None` if weak pointer was dropped.
    pub fn safe_upgrade(&self) -> Option<WebRtcPublishEndpoint> {
        self.0.upgrade().map(WebRtcPublishEndpoint)
    }
}

impl Into<ElementProto> for WebRtcPublishEndpoint {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut publish = WebRtcPublishEndpointProto::new();
        publish.set_p2p(self.p2p().into());
        publish.set_id(self.id().to_string());
        element.set_webrtc_pub(publish);

        element
    }
}

impl Into<RootElementProto> for WebRtcPublishEndpoint {
    fn into(self) -> RootElementProto {
        let mut element = RootElementProto::new();
        let mut member_element: ElementProto = self.into();
        let endpoint = member_element.take_webrtc_pub();
        element.set_webrtc_pub(endpoint);
        element
    }
}
