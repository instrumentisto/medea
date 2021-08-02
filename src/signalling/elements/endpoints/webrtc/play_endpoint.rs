//! [`WebRtcPlayEndpoint`] implementation.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api as proto;

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
    ///
    /// [`Peer`]: crate::media::peer::Peer
    peer_id: Option<PeerId>,

    /// [`PeerId`] of the [`Peer`] created to source a
    /// [`WebRtcPublishEndpoint`] from which this [`WebRtcPlayEndpoint`]
    /// receives data.
    partner_peer_id: Option<PeerId>,

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

    fn set_peer_id_and_partner_peer_id(
        &mut self,
        pid: PeerId,
        partner_pid: PeerId,
    ) {
        self.peer_id = Some(pid);
        self.partner_peer_id = Some(partner_pid);
    }

    fn peer_id(&self) -> Option<PeerId> {
        self.peer_id
    }

    fn reset(&mut self) {
        self.peer_id = None;
    }
}

impl Drop for WebRtcPlayEndpointInner {
    fn drop(&mut self) {
        if let Some(receiver_publisher) = self.src.safe_upgrade() {
            if let Some(partner_peer_id) = self.partner_peer_id {
                receiver_publisher.remove_peer_ids(&[partner_peer_id]);
            }
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
    #[inline]
    #[must_use]
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
            partner_peer_id: None,
            is_force_relayed,
        })))
    }

    /// Returns [`SrcUri`] of this [`WebRtcPlayEndpoint`].
    #[inline]
    #[must_use]
    pub fn src_uri(&self) -> SrcUri {
        self.0.borrow().src_uri()
    }

    /// Returns owner [`Member`] of this [`WebRtcPlayEndpoint`].
    ///
    /// __This function will panic if pointer to [`Member`] was dropped.__
    #[inline]
    #[must_use]
    pub fn owner(&self) -> Member {
        self.0.borrow().owner()
    }

    /// Returns weak pointer to owner [`Member`] of this
    /// [`WebRtcPlayEndpoint`].
    #[inline]
    #[must_use]
    pub fn weak_owner(&self) -> WeakMember {
        self.0.borrow().weak_owner()
    }

    /// Returns srcs's [`WebRtcPublishEndpoint`].
    ///
    /// __This function will panic if weak pointer was dropped.__
    #[inline]
    #[must_use]
    pub fn src(&self) -> WebRtcPublishEndpoint {
        self.0.borrow().src()
    }

    /// Saves [`PeerId`]s of this [`WebRtcPlayEndpoint`] and the source
    /// [`WebRtcPublishEndpoint`].
    #[inline]
    pub fn set_peer_id_and_partner_peer_id(
        &self,
        pid: PeerId,
        partner_pid: PeerId,
    ) {
        self.0
            .borrow_mut()
            .set_peer_id_and_partner_peer_id(pid, partner_pid);
    }

    /// Returns [`PeerId`] of this [`WebRtcPlayEndpoint`]'s [`Peer`].
    ///
    /// [`Peer`]: crate::media::peer::Peer
    #[inline]
    #[must_use]
    pub fn peer_id(&self) -> Option<PeerId> {
        self.0.borrow().peer_id()
    }

    /// Resets state of this [`WebRtcPlayEndpoint`].
    ///
    /// _Atm this only resets [`PeerId`]._
    #[inline]
    pub fn reset(&self) {
        self.0.borrow_mut().reset();
    }

    /// Returns [`Id`] of this [`WebRtcPlayEndpoint`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> Id {
        self.0.borrow().id.clone()
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPlayEndpoint`].
    #[inline]
    #[must_use]
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    #[allow(clippy::unused_self)]
    #[inline]
    #[must_use]
    pub fn has_traffic_callback(&self) -> bool {
        // TODO: Must depend on on_start/on_stop endpoint callbacks, when those
        //       will be added (#91).
        true
    }

    /// Downgrades [`WebRtcPlayEndpoint`] to [`WeakWebRtcPlayEndpoint`] weak
    /// pointer.
    #[inline]
    #[must_use]
    pub fn downgrade(&self) -> WeakWebRtcPlayEndpoint {
        WeakWebRtcPlayEndpoint(Rc::downgrade(&self.0))
    }

    /// Compares [`WebRtcPlayEndpoint`]'s inner pointers. If both pointers
    /// points to the same address, then returns `true`.
    #[cfg(test)]
    #[must_use]
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
    #[inline]
    #[must_use]
    pub fn upgrade(&self) -> WebRtcPlayEndpoint {
        WebRtcPlayEndpoint(self.0.upgrade().unwrap())
    }

    /// Upgrades to [`WebRtcPlayEndpoint`] safely.
    ///
    /// Returns [`None`] if weak pointer has been dropped.
    #[inline]
    pub fn safe_upgrade(&self) -> Option<WebRtcPlayEndpoint> {
        self.0.upgrade().map(WebRtcPlayEndpoint)
    }
}

impl From<WebRtcPlayEndpoint> for proto::member::Element {
    #[inline]
    fn from(endpoint: WebRtcPlayEndpoint) -> Self {
        Self {
            el: Some(proto::member::element::El::WebrtcPlay(endpoint.into())),
        }
    }
}

impl From<WebRtcPlayEndpoint> for proto::WebRtcPlayEndpoint {
    fn from(endpoint: WebRtcPlayEndpoint) -> Self {
        Self {
            on_start: String::new(),
            on_stop: String::new(),
            src: endpoint.src_uri().to_string(),
            id: endpoint.id().to_string(),
            force_relay: endpoint.is_force_relayed(),
        }
    }
}

impl From<WebRtcPlayEndpoint> for proto::Element {
    #[inline]
    fn from(endpoint: WebRtcPlayEndpoint) -> Self {
        Self {
            el: Some(proto::element::El::WebrtcPlay(endpoint.into())),
        }
    }
}
