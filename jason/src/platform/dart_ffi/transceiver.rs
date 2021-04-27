//! [`RtcRtpTransceiver`] wrapper.

use std::rc::Rc;

use bitflags::bitflags;
use futures::future::LocalBoxFuture;

use crate::{media::track::local, platform::Error};

/// Wrapper around [`RtcRtpTransceiver`] which provides handy methods for
/// direction changes.
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

    /// Indicates whether the underlying [`RtcRtpTransceiver`] is stopped.
    #[inline]
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        unimplemented!()
    }
}

// TODO: Probably should be shared between wasm and dart_ffi.
bitflags! {
    /// Representation of [RTCRtpTransceiverDirection][1].
    ///
    /// [`sendrecv` direction][2] can be represented by
    /// [`TransceiverDirection::all`] bitflag.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection-sendrecv
    pub struct TransceiverDirection: u8 {
        /// [`inactive` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y2zslyw2
        const INACTIVE = 0b00;

        /// [`sendonly` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y6y2ye97
        const SEND = 0b01;

        /// [`recvonly` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y2nlxpzf
        const RECV = 0b10;
    }
}
