//! [`WebRtcPlayEndpoint`] implementation.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::medea::{
    member::Element as ElementProto, Element as RootElementProto,
    WebRtcPlayEndpoint as WebRtcPlayEndpointProto,
};

use crate::{
    api::control::{
        endpoints::webrtc_play_endpoint::WebRtcPlayId as Id, refs::SrcUri,
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

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPlayEndpoint`].
    is_force_relayed: bool,
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

/// Signalling representation of Control API's [`WebRtcPlayEndpoint`].
///
/// [`WebRtcPlayEndpoint`]: crate::api::control::endpoints::WebRtcPlayEndpoint
#[derive(Debug, Clone)]
pub struct WebRtcPlayEndpoint(Rc<RefCell<WebRtcPlayEndpointInner>>);

impl WebRtcPlayEndpoint {
    /// Creates new [`WebRtcPlayEndpoint`].
    pub fn new(
        id: Id,
        src_uri: SrcUri,
        publisher: WeakWebRtcPublishEndpoint,
        owner: WeakMember,
        is_force_relayed: bool,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPlayEndpointInner {
            id,
            src_uri,
            src: publisher,
            owner,
            peer_id: None,
            is_force_relayed,
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src_uri(&self) -> SrcUri {
        self.0.borrow().src_uri()
    }

    /// Returns owner [`Member`] of this [`WebRtcPlayEndpoint`].
    ///
    /// __This function will panic if pointer to [`Member`] was dropped.__
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Returns weak pointer to owner [`Member`] of this
    /// [`WebRtcPlayEndpoint`].
    pub fn weak_owner(&self) -> WeakMember {
        self.0.borrow().weak_owner()
    }

    /// Returns srcs's [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if weak pointer was dropped.__
    pub fn src(&self) -> WebRtcPublishEndpoint {
        self.0.borrow().src()
    }

    /// Saves [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn set_peer_id(&self, peer_id: PeerId) {
        self.0.borrow_mut().set_peer_id(peer_id);
    }

    /// Returns [`PeerId`] of this [`WebRtcPlayEndpoint`]'s [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.borrow().peer_id()
    }

    /// Resets state of this [`WebRtcPlayEndpoint`].
    ///
    /// _Atm this only resets [`PeerId`]._
    pub fn reset(&self) {
        self.0.borrow_mut().reset()
    }

    /// Returns [`Id`] of this [`WebRtcPlayEndpoint`].
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPlayEndpoint`].
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Downgrades [`WebRtcPlayEndpoint`] to [`WeakWebRtcPlayEndpoint`] weak
    /// pointer.
    pub fn downgrade(&self) -> WeakWebRtcPlayEndpoint {
        WeakWebRtcPlayEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPlayEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    pub fn ptr_eq(&self, another_play: &Self) -> bool {
        Rc::ptr_eq(&self.0, &another_play.0)
    }
}

/// Weak pointer to [`WebRtcPlayEndpoint`].
#[derive(Debug, Clone)]
pub struct WeakWebRtcPlayEndpoint(Weak<RefCell<WebRtcPlayEndpointInner>>);

impl WeakWebRtcPlayEndpoint {
    /// Upgrades weak pointer to strong pointer.
    ///
    /// # Panics
    ///
    /// If weak pointer has been dropped.
    pub fn upgrade(&self) -> WebRtcPlayEndpoint {
        WebRtcPlayEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns `None` if weak pointer has been dropped.
    pub fn safe_upgrade(&self) -> Option<WebRtcPlayEndpoint> {
        self.0.upgrade().map(WebRtcPlayEndpoint)
    }
}

impl Into<ElementProto> for WebRtcPlayEndpoint {
    fn into(self) -> ElementProto {
        let mut element = ElementProto::new();
        let mut play = WebRtcPlayEndpointProto::new();
        play.set_src(self.src_uri().to_string());
        play.set_id(self.id().to_string());
        play.set_force_relay(self.is_force_relayed());
        element.set_webrtc_play(play);

        element
    }
}

impl Into<RootElementProto> for WebRtcPlayEndpoint {
    fn into(self) -> RootElementProto {
        let mut element = RootElementProto::new();
        let mut member_element: ElementProto = self.into();
        let endpoint = member_element.take_webrtc_play();
        element.set_webrtc_play(endpoint);

        element
    }
}
