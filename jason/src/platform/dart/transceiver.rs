//! [RTCRtpTransceiver] wrapper.
//!
//! [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver

use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::{
    media::track::local,
    platform::{Error, TransceiverDirection},
};

/// Wrapper around [RTCRtpTransceiver] which provides handy methods for
/// direction changes.
///
/// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
#[derive(Clone)]
pub struct Transceiver;

impl Transceiver {
    /// Disables provided [`TransceiverDirection`] of this [`Transceiver`].
    #[inline]
    pub fn sub_direction(&self, disabled_direction: TransceiverDirection) {
        unimplemented!()
    }

    /// Enables provided [`TransceiverDirection`] of this [`Transceiver`].
    #[inline]
    pub fn add_direction(&self, enabled_direction: TransceiverDirection) {
        unimplemented!()
    }

    /// Indicates whether the provided [`TransceiverDirection`] is enabled for
    /// this [`Transceiver`].
    #[inline]
    #[must_use]
    pub fn has_direction(&self, direction: TransceiverDirection) -> bool {
        unimplemented!()
    }

    /// Replaces [`TransceiverDirection::SEND`] [`local::Track`] of this
    /// [`Transceiver`].
    ///
    /// # Errors
    ///
    /// Errors with [`Error`] if the underlying [`replaceTrack`][1] call fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn set_send_track(
        &self,
        new_track: Rc<local::Track>,
    ) -> Result<(), Error> {
        unimplemented!()
    }

    /// Sets a [`TransceiverDirection::SEND`] [`local::Track`] of this
    /// [`Transceiver`] to [`None`].
    #[must_use]
    pub fn drop_send_track(&self) -> LocalBoxFuture<'static, ()> {
        unimplemented!()
    }

    /// Returns [`mid`] of this [`Transceiver`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    #[inline]
    #[must_use]
    pub fn mid(&self) -> Option<String> {
        unimplemented!()
    }

    /// Returns [`local::Track`] that is being send to remote, if any.
    #[inline]
    #[must_use]
    pub fn send_track(&self) -> Option<Rc<local::Track>> {
        unimplemented!()
    }

    /// Indicates whether this [`Transceiver`] has [`local::Track`].
    #[inline]
    #[must_use]
    pub fn has_send_track(&self) -> bool {
        unimplemented!()
    }

    /// Sets the underlying [`local::Track`]'s `enabled` field to the provided
    /// value, if any.
    #[inline]
    pub fn set_send_track_enabled(&self, enabled: bool) {
        unimplemented!()
    }

    /// Indicates whether the underlying [RTCRtpTransceiver] is stopped.
    ///
    /// [RTCRtpTransceiver]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiver
    #[inline]
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        unimplemented!()
    }
}
