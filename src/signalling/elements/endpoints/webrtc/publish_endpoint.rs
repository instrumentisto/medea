//! [`WebRtcPublishEndpoint`] implementation.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::{Rc, Weak},
};

use medea_client_api_proto::PeerId;
use medea_control_api_proto::grpc::api as proto;

use crate::{
    api::control::{
        callback::url::CallbackUrl,
        endpoints::webrtc_publish_endpoint::{P2pMode, WebRtcPublishId as Id},
        refs::{Fid, ToEndpoint},
    },
    signalling::elements::{
        endpoints::webrtc::play_endpoint::WeakWebRtcPlayEndpoint,
        member::WeakMember, Member,
    },
};

use super::play_endpoint::WebRtcPlayEndpoint;
use crate::{
    api::control::callback::{
        CallbackRequest, EndpointDirection, EndpointKind, OnStartEvent,
        OnStopEvent, OnStopReason,
    },
    signalling::elements::endpoints::webrtc::TracksState,
};
use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
struct WebRtcPublishEndpointInner {
    /// ID of this [`WebRtcPublishEndpoint`].
    id: Id,

    /// P2P connection mode for this [`WebRtcPublishEndpoint`].
    p2p: P2pMode,

    /// Indicator whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    is_force_relayed: bool,

    /// All sinks of this [`WebRtcPublishEndpoint`].
    sinks: Vec<WeakWebRtcPlayEndpoint>,

    /// Owner [`Member`] of this [`WebRtcPublishEndpoint`].
    owner: WeakMember,

    /// Publishing statuses of the [`Peer`]s related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// `true` value indicated that [`Peer`] is publishes some media traffic.
    ///
    /// Also this field acts as store of all [`PeerId`]s related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// [`Peer`]: crate::media::peer:Peer
    peers_publishing_statuses: HashMap<PeerId, bool>,

    /// URL to which `OnStart` Control API callback will be sent.
    on_start: Option<CallbackUrl>,

    /// URL to which `OnStop` Control API callback will be sent.
    on_stop: Option<CallbackUrl>,

    state: TracksState,

    awaits_start_state: Option<TracksState>,

    awaits_stop_state: Option<TracksState>,

    on_stop_reason: Option<OnStopReason>,
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

    fn peer_ids(&self) -> HashSet<PeerId> {
        self.peers_publishing_statuses.keys().cloned().collect()
    }

    #[allow(clippy::trivially_copy_pass_by_ref)]
    fn remove_peer_id(&mut self, peer_id: &PeerId) {
        self.peers_publishing_statuses.remove(peer_id);
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
    pub const DIRECTION: EndpointDirection = EndpointDirection::Publish;

    /// Creates new [`WebRtcPublishEndpoint`].
    pub fn new(
        id: Id,
        p2p: P2pMode,
        owner: WeakMember,
        is_force_relayed: bool,
        on_start: Option<CallbackUrl>,
        on_stop: Option<CallbackUrl>,
    ) -> Self {
        Self(Rc::new(RefCell::new(WebRtcPublishEndpointInner {
            id,
            p2p,
            is_force_relayed,
            sinks: Vec::new(),
            owner,
            peers_publishing_statuses: HashMap::new(),
            on_start,
            on_stop,
            state: TracksState::new(),
            awaits_start_state: Some(TracksState::with_kind(
                EndpointKind::Both,
            )),
            awaits_stop_state: None,
            on_stop_reason: None,
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

    /// Adds [`PeerId`] related to this [`WebRtcPublishEndpoint`].
    pub fn add_peer_id(&self, peer_id: PeerId) {
        self.0
            .borrow_mut()
            .peers_publishing_statuses
            .insert(peer_id, false);
    }

    /// Returns all [`PeerId`]s related to this [`WebRtcPublishEndpoint`].
    pub fn peer_ids(&self) -> HashSet<PeerId> {
        self.0.borrow().peer_ids()
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
        self.0.borrow().p2p
    }

    /// Indicates whether only `relay` ICE candidates are allowed for this
    /// [`WebRtcPublishEndpoint`].
    pub fn is_force_relayed(&self) -> bool {
        self.0.borrow().is_force_relayed
    }

    /// Sets publishing status of the provided [`PeerId`] related to this
    /// [`WebRtcPublishEndpoint`].
    ///
    /// If provided [`PeerId`] is not related to this [`WebRtcPublishEndpoint`]
    /// then nothing will be done.
    pub fn set_peer_status(&self, peer_id: PeerId, is_publishing: bool) {
        if let Some(peer_status) = self
            .0
            .borrow_mut()
            .peers_publishing_statuses
            .get_mut(&peer_id)
        {
            *peer_status = is_publishing;
        }
    }

    /// Returns `true` if at least one [`PeerConnection`] related to this
    /// [`WebRtcPublishEndpoint`] is publishing.
    pub fn is_endpoint_publishing(&self) -> bool {
        self.0
            .borrow()
            .peers_publishing_statuses
            .values()
            .any(|status| *status)
    }

    /// Returns count of [`Peer`]s which are publishes.
    pub fn publishing_peers_count(&self) -> usize {
        self.0
            .borrow()
            .peers_publishing_statuses
            .values()
            .filter(|s| **s)
            .count()
    }

    /// Returns [`CallbackUrl`] to which Medea should send `OnStart` callback.
    ///
    /// Sets [`WebRtcPlayEndpoint::state`] to the [`EndpointState::Started`].
    #[allow(clippy::if_not_else)]
    pub fn get_on_start(
        &self,
        at: DateTime<Utc>,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        let mut inner = self.0.borrow_mut();
        if let Some(awaits_on_start) = inner.awaits_start_state {
            if inner.state == awaits_on_start {
                return None;
            }
            inner.state = awaits_on_start;
            inner.awaits_start_state = None;
            let fid =
                inner.owner().get_fid_to_endpoint(inner.id.clone().into());

            if let Some(on_start) = inner.on_start.clone() {
                return Some((
                    on_start,
                    CallbackRequest::new(
                        fid,
                        OnStartEvent {
                            direction: Self::DIRECTION,
                            kind: if awaits_on_start
                                .is_started(EndpointKind::Both)
                            {
                                EndpointKind::Both
                            } else if awaits_on_start
                                .is_started(EndpointKind::Audio)
                            {
                                EndpointKind::Audio
                            } else if awaits_on_start
                                .is_started(EndpointKind::Video)
                            {
                                EndpointKind::Video
                            } else {
                                return None;
                            },
                        },
                        at,
                    ),
                ));
            }
        }

        None
    }

    pub fn awaits_starting(&self, kind: EndpointKind) {
        let mut inner = self.0.borrow_mut();
        if let Some(awaits_start_state) = inner.awaits_start_state.as_mut() {
            awaits_start_state.started(kind);
        } else {
            let state = TracksState::with_kind(kind);
            inner.awaits_start_state = Some(state);
        }
    }

    pub fn awaits_stopping(&self, kind: EndpointKind) {
        let mut inner = self.0.borrow_mut();
        if let Some(await_stop_state) = inner.awaits_stop_state.as_mut() {
            await_stop_state.stopped(kind);
        } else {
            let state = TracksState::with_kind(kind);
            inner.awaits_stop_state = Some(state);
        }
    }

    pub fn set_on_stop_reason(&self, reason: OnStopReason) {
        let mut inner = self.0.borrow_mut();
        inner.on_stop_reason = Some(reason);
    }

    /// Returns `true` if `on_start` or `on_stop` callback is set.
    pub fn any_traffic_callback_is_some(&self) -> bool {
        let inner = self.0.borrow();
        inner.on_stop.is_some() || inner.on_start.is_some()
    }

    /// Returns [`CallbackUrl`] and [`Fid`] for the `on_stop` Control API
    /// callback of this [`WebRtcPublishEndpoint`].
    ///
    /// Also this function changes peer status of [`WebRtcPublishEndpoint`].
    ///
    /// Sets [`WebRtcPublishEndpoint::state`] to the [`EndpointState::Stopped`].
    ///
    /// If [`WebRtcPublishEndpoint::state`] currently is
    /// [`EndpointState::Stopped`] `None` will be returned.
    pub fn get_on_stop(
        &self,
        peer_id: PeerId,
        at: DateTime<Utc>,
        kind: EndpointKind,
    ) -> Option<(CallbackUrl, CallbackRequest)> {
        self.set_peer_status(peer_id, false);
        let mut inner = self.0.borrow_mut();

        if !inner.state.is_stopped(kind) {
            inner.state.stopped(kind);

            let fid =
                inner.owner().get_fid_to_endpoint(inner.id.clone().into());
            if let Some(url) = inner.on_stop.clone() {
                return Some((
                    url,
                    CallbackRequest::new(
                        fid,
                        OnStopEvent {
                            reason: inner
                                .on_stop_reason
                                .take()
                                .unwrap_or(OnStopReason::TrafficNotFlowing),
                            kind,
                            direction: Self::DIRECTION,
                        },
                        at,
                    ),
                ));
            }
        }

        None
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

impl Into<proto::WebRtcPublishEndpoint> for WebRtcPublishEndpoint {
    fn into(self) -> proto::WebRtcPublishEndpoint {
        let p2p: proto::web_rtc_publish_endpoint::P2p = self.p2p().into();
        proto::WebRtcPublishEndpoint {
            p2p: p2p as i32,
            id: self.id().to_string(),
            force_relay: self.is_force_relayed(),
            on_stop: self.0.borrow().on_stop.as_ref().map(ToString::to_string),
            on_start: self
                .0
                .borrow()
                .on_start
                .as_ref()
                .map(ToString::to_string),
        }
    }
}

impl Into<proto::member::Element> for WebRtcPublishEndpoint {
    fn into(self) -> proto::member::Element {
        proto::member::Element {
            el: Some(proto::member::element::El::WebrtcPub(self.into())),
        }
    }
}

impl Into<proto::Element> for WebRtcPublishEndpoint {
    fn into(self) -> proto::Element {
        proto::Element {
            el: Some(proto::element::El::WebrtcPub(self.into())),
        }
    }
}
