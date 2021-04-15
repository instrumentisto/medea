//! Remote [MediaStreamTrack][1] representation.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack

use std::cell::{Cell, RefCell};

use medea_client_api_proto::{MediaType, TrackId as Id};

/// Representation of [MediaStreamTrack][1] object.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
#[derive(Debug, Eq, PartialEq)]
pub struct MediaTrack {
    id: Id,
    mid: RefCell<Option<String>>,
    media_type: MediaType,
    transceiver_enabled: Cell<bool>,
    send_media_state: MediaState,
    recv_media_state: MediaState,
}

impl MediaTrack {
    /// Creates new [`MediaTrack`] of the specified [`MediaType`].
    #[inline]
    #[must_use]
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self {
            id,
            mid: RefCell::new(None),
            media_type,
            transceiver_enabled: Cell::new(true),
            send_media_state: MediaState::default(),
            recv_media_state: MediaState::default(),
        }
    }

    /// Returns [`Id`] of this [`MediaTrack`].
    #[inline]
    #[must_use]
    pub fn id(&self) -> Id {
        self.id
    }

    /// Returns reference to the [`MediaType`] of this [`MediaTrack`].
    #[inline]
    #[must_use]
    pub fn media_type(&self) -> &MediaType {
        &self.media_type
    }

    /// Sets [`mid`] of this [`MediaTrack`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc#dom-rtptransceiver-mid
    #[inline]
    pub fn set_mid(&self, mid: String) {
        self.mid.borrow_mut().replace(mid);
    }

    /// Returns [`mid`] of this [`MediaTrack`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc#dom-rtptransceiver-mid
    #[inline]
    #[must_use]
    pub fn mid(&self) -> Option<String> {
        self.mid.borrow_mut().as_ref().cloned()
    }

    /// Enables/disables transceiver publishing state.
    #[inline]
    pub fn set_transceiver_enabled(&self, enabled: bool) {
        self.transceiver_enabled.set(enabled);
    }

    /// Returns transceiver publishing state of this [`MediaTrack`].
    #[inline]
    #[must_use]
    pub fn is_transceiver_enabled(&self) -> bool {
        self.transceiver_enabled.get()
    }

    /// Indicates whether this [`MediaTrack`] is enabled for send and recv side.
    #[inline]
    #[must_use]
    pub fn is_enabled_general(&self) -> bool {
        self.send_media_state.is_enabled() && self.recv_media_state.is_enabled()
    }

    /// Returns [`MediaState`] for the recv side.
    #[inline]
    #[must_use]
    pub fn recv_media_state(&self) -> &MediaState {
        &self.recv_media_state
    }

    /// Returns [`MediaState`] for the send side.
    #[inline]
    #[must_use]
    pub fn send_media_state(&self) -> &MediaState {
        &self.send_media_state
    }
}

/// Media publishing/receiving state.
#[derive(Debug, Eq, PartialEq)]
pub struct MediaState {
    /// Indicator whether [`MediaTrack`] is muted or unmuted.
    muted: Cell<bool>,

    /// Indicator whether [`MediaTrack`] is enabled or disabled.
    enabled: Cell<bool>,
}

impl Default for MediaState {
    #[inline]
    fn default() -> Self {
        Self {
            enabled: Cell::new(true),
            muted: Cell::new(false),
        }
    }
}

impl MediaState {
    /// Indicates whether this [`MediaState`] is enabled.
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    /// Indicates whether this [`MediaState`] is muted.
    #[inline]
    #[must_use]
    pub fn is_muted(&self) -> bool {
        self.muted.get()
    }

    /// Sets the current mute state to the provided one.
    #[inline]
    pub fn set_muted(&self, muted: bool) {
        self.muted.set(muted);
    }

    /// Sets the current media exchange state to the provided one.
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.set(enabled);
    }
}
