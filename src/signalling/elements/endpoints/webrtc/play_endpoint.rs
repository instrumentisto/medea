//! [`WebRtcPlayEndpoint`] implementation.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_grpc_proto::control::{
    Member_Element as ElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
};

use crate::{
    api::control::endpoints::webrtc_play_endpoint::{
        SrcUri, WebRtcPlayId as Id,
    },
    signalling::elements::{
        endpoints::webrtc::publish_endpoint::WeakWebRtcPublishEndpoint,
        member::WeakMember, Member,
    },
};

use super::publish_endpoint::WebRtcPublishEndpoint;

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// ID of this [`WebRtcPlayEndpoint`].
    id: Id,

    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src_uri: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: WeakWebRtcPublishEndpoint,

    /// Owner [`Member`] of this [`WebRtcPlayEndpoint`].
    owner: WeakMember,

    /// [`PeerId`] of [`Peer`] created for this [`WebRtcPlayEndpoint`].
    ///
    /// Currently this field used for detecting status of this
    /// [`WebRtcPlayEndpoint`] connection.
    ///
    /// In future this may be used for removing [`WebRtcPlayEndpoint`]
    /// and related peer.
    peer_id: Option<PeerId>,
}

impl WebRtcPlayEndpointInner {
    fn src_uri(&self) -> SrcUri {
        self.src_uri.clone()
    }

    fn owner(&self) -> Member {
        self.owner.upgrade()
    }

    fn weak_owner(&self) -> WeakMember {
        self.owner.clone()
    }

    fn src(&self) -> WebRtcPublishEndpoint {
        self.src.upgrade()
    }

    fn set_peer_id(&mut self, peer_id: PeerId) {
        self.peer_id = Some(peer_id)
    }

    fn peer_id(&self) -> Option<PeerId> {
        self.peer_id
    }

    fn reset(&mut self) {
        self.peer_id = None
    }
}

impl Drop for WebRtcPlayEndpointInner {
    fn drop(&mut self) {
        if let Some(receiver_publisher) = self.src.safe_upgrade() {
            receiver_publisher.remove_empty_weaks_from_sinks();
        }
    }
}

/// Signalling representation of `WebRtcPlayEndpoint`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint(Rc<RefCell<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    /// Create new [`WebRtcPlayEndpoint`].
    pub fn new(
        id: Id,
        src_uri: SrcUri,
        publisher: WeakWebRtcPublishEndpoint,
        owner: WeakMember,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPlayEndpointInner {
            id,
            src_uri,
            src: publisher,
            owner,
            peer_id: None,
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src_uri(&self) -> SrcUri {
        self.0.borrow().src_uri()
    }

    /// Returns owner [`Member`] of this [`WebRtcPlayEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Returns `Weak` pointer to owner [`Member`] of this
    /// [`WebRtcPlayEndpoint`].
    pub fn weak_owner(&self) -> WeakMember {
        self.0.borrow().weak_owner()
    }

    /// Returns source's [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn src(&self) -> WebRtcPublishEndpoint {
        self.0.borrow().src()
    }

    /// Save [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn set_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().set_peer_id(peer_id);
    }

    /// Return [`PeerId`] of [`Peer`] of this [`WebRtcPlayEndpoint`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.borrow().peer_id()
    }

    /// Reset state of this [`WebRtcPlayEndpoint`].
    ///
    /// Atm this only reset peer_id.
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Returns ID of this [`WebRtcPlayEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Downgrade [`WeakWebRtcPlayEndpoint`] to weak pointer
    /// [`WeakWebRtcPlayEndpoint`].
    pub fn downgrade(&self) -> WeakWebRtcPlayEndpoint {
        WeakWebRtcPlayEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares pointers. If both pointers point to the same address, then
    /// returns true.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_play: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_play.0)
    }
}

/// Weak pointer to [`WebRtcPlayEndpoint`].
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct WeakWebRtcPlayEndpoint(Weak<RefCell<WebRtcPlayEndpointInner>>);

impl WeakWebRtcPlayEndpoint {
    /// Upgrade weak pointer to strong pointer.
    ///
    /// This function will __panic__ if weak pointer is `None`.
    pub fn upgrade(&self) -> WebRtcPlayEndpoint {
        WebRtcPlayEndpoint(self.0.upgrade().unwrap())
    }

    /// Safe upgrade to [`WebRtcPlayEndpoint`].
    pub fn safe_upgrade(&self) -> Option<WebRtcPlayEndpoint> {
        self.0.upgrade().map(WebRtcPlayEndpoint)
    }
}

impl Into<ElementProto> for WebRtcPlayEndpoint {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut play = WebRtcPlayEndpointProto::new();
        play.set_src(self.src_uri().to_string());
        element.set_webrtc_play(play);

        element
    }
}
