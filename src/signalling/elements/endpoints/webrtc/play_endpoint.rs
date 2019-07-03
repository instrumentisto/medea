use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use macro_attr::*;
use newtype_derive::{newtype_fmt, NewtypeDisplay, NewtypeFrom};

use crate::{
    api::control::endpoint::SrcUri, media::PeerId,
    signalling::control::member::Member,
};

use super::publish_endpoint::WebRtcPublishEndpoint;

pub use Id as WebRtcPlayId;

macro_attr! {
    /// ID of endpoint.
    #[derive(Clone, Debug, Eq, Hash, PartialEq, NewtypeFrom!, NewtypeDisplay!)]
    pub struct Id(pub String);
}

#[derive(Debug, Clone)]
struct WebRtcPlayEndpointInner {
    /// ID of this [`WebRtcPlayEndpoint`].
    id: Id,

    /// Source URI of [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src_uri: SrcUri,

    /// Publisher [`WebRtcPublishEndpoint`] from which this
    /// [`WebRtcPlayEndpoint`] receive data.
    src: Weak<WebRtcPublishEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPlayEndpoint`].
    owner: Weak<Member>,

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

    fn owner(&self) -> Rc<Member> {
        Weak::upgrade(&self.owner).unwrap()
    }

    fn weak_owner(&self) -> Weak<Member> {
        Weak::clone(&self.owner)
    }

    fn src(&self) -> Rc<WebRtcPublishEndpoint> {
        Weak::upgrade(&self.src).unwrap()
    }

    fn is_connected(&self) -> bool {
        self.peer_id.is_some()
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
        if let Some(receiver_publisher) = self.src.upgrade() {
            receiver_publisher.remove_empty_weaks_from_sinks();
        }
    }
}

/// Signalling representation of `WebRtcPlayEndpoint`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct WebRtcPlayEndpoint(RefCell<WebRtcPlayEndpointInner>);

impl WebRtcPlayEndpoint {
    /// Create new [`WebRtcPlayEndpoint`].
    pub fn new(
        id: Id,
        src_uri: SrcUri,
        publisher: Weak<WebRtcPublishEndpoint>,
        owner: Weak<Member>,
    ) -> Self {
        Self(RefCell::new(WebRtcPlayEndpointInner {
            id,
            src_uri,
            src: publisher,
            owner,
            peer_id: None,
        }))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    pub fn src_uri(&self) -> SrcUri {
        self.0.borrow().src_uri()
    }

    /// Returns owner [`Member`] of this [`WebRtcPlayEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn owner(&self) -> Rc<Member> {
        self.0.borrow().owner()
    }

    /// Returns `Weak` pointer to owner [`Member`] of this
    /// [`WebRtcPlayEndpoint`].
    pub fn weak_owner(&self) -> Weak<Member> {
        self.0.borrow().weak_owner()
    }

    /// Returns source's [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if pointer is empty.__
    pub fn src(&self) -> Rc<WebRtcPublishEndpoint> {
        self.0.borrow().src()
    }

    /// Check that peer connection established for this [`WebRtcPlayEndpoint`].
    pub fn is_connected(&self) -> bool {
        self.0.borrow().is_connected()
    }

    /// Save [`PeerId`] of this [`WebRtcPlayEndpoint`].
    pub fn connect(&self, peer_id: PeerId) {
        self.0.borrow_mut().set_peer_id(peer_id);
    }

    /// Return [`PeerId`] of [`Peer`] of this [`WebRtcPlayEndpoint`].
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
}
