//! [`RtcRtpTransceiver`] wrapper.

use std::{cell::RefCell, rc::Rc};

use futures::future::LocalBoxFuture;
use medea_client_api_proto::Direction as DirectionProto;
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

use crate::{
    media::track::local,
    platform::{transceiver::TransceiverDirection, Error},
};

/// Wrapper around [`RtcRtpTransceiver`] which provides handy methods for
/// direction changes.
#[derive(Clone)]
pub struct Transceiver {
    send_track: RefCell<Option<Rc<local::Track>>>,
    transceiver: RtcRtpTransceiver,
}

impl Transceiver {
    /// Returns current [`TransceiverDirection`] of this [`Transceiver`].
    #[inline]
    #[must_use]
    fn current_direction(&self) -> TransceiverDirection {
        TransceiverDirection::from(self.transceiver.direction())
    }

    /// Disables provided [`TransceiverDirection`] of this [`Transceiver`].
    #[inline]
    pub fn sub_direction(&self, disabled_direction: TransceiverDirection) {
        self.transceiver.set_direction(
            (self.current_direction() - disabled_direction).into(),
        );
    }

    /// Enables provided [`TransceiverDirection`] of this [`Transceiver`].
    #[inline]
    pub fn add_direction(&self, enabled_direction: TransceiverDirection) {
        self.transceiver.set_direction(
            (self.current_direction() | enabled_direction).into(),
        );
    }

    /// Indicates whether the provided [`TransceiverDirection`] is enabled for
    /// this [`Transceiver`].
    #[inline]
    #[must_use]
    pub fn has_direction(&self, direction: TransceiverDirection) -> bool {
        self.current_direction().contains(direction)
    }

    /// Replaces [`TransceiverDirection::SEND`] [`local::Track`] of this
    /// [`Transceiver`].
    ///
    /// # Errors
    ///
    /// Errors with JS error if the underlying [`replaceTrack`][1] call fails.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn set_send_track(
        &self,
        new_track: Rc<local::Track>,
    ) -> Result<(), Error> {
        let sys_track: &web_sys::MediaStreamTrack =
            (*new_track).as_ref().as_ref();
        JsFuture::from(
            self.transceiver.sender().replace_track(Some(sys_track)),
        )
        .await?;
        self.send_track.replace(Some(new_track));
        Ok(())
    }

    /// Sets a [`TransceiverDirection::SEND`] [`local::Track`] of this
    /// [`Transceiver`] to [`None`].
    ///
    /// # Panics
    ///
    /// If [`local::Track`] replacement with [`None`] fails on JS side, but
    /// basing on [WebAPI docs] it should never happen.
    ///
    /// [WebAPI docs]: https://tinyurl.com/7pnszaa8
    pub fn drop_send_track(&self) -> LocalBoxFuture<'static, ()> {
        self.send_track.replace(None);
        let fut = self.transceiver.sender().replace_track(None);
        Box::pin(async move {
            // Replacing track to None should never fail.
            JsFuture::from(fut).await.unwrap();
        })
    }

    /// Returns [`mid`] of this [`Transceiver`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    #[inline]
    #[must_use]
    pub fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }

    /// Returns [`local::Track`] that is being send to remote, if any.
    #[inline]
    #[must_use]
    pub fn send_track(&self) -> Option<Rc<local::Track>> {
        self.send_track.borrow().clone()
    }

    /// Indicates whether this [`Transceiver`] has [`local::Track`].
    #[inline]
    #[must_use]
    pub fn has_send_track(&self) -> bool {
        self.send_track.borrow().is_some()
    }

    /// Sets the underlying [`local::Track`]'s `enabled` field to the provided
    /// value, if any.
    #[inline]
    pub fn set_send_track_enabled(&self, enabled: bool) {
        if let Some(track) = self.send_track.borrow().as_ref() {
            track.set_enabled(enabled);
        }
    }

    /// Indicates whether the underlying [`RtcRtpTransceiver`] is stopped.
    #[inline]
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.transceiver.stopped()
    }
}

impl From<RtcRtpTransceiver> for Transceiver {
    #[inline]
    fn from(transceiver: RtcRtpTransceiver) -> Self {
        Transceiver {
            send_track: RefCell::new(None),
            transceiver,
        }
    }
}

impl From<RtcRtpTransceiverDirection> for TransceiverDirection {
    fn from(direction: RtcRtpTransceiverDirection) -> Self {
        use RtcRtpTransceiverDirection as D;

        match direction {
            D::Sendonly => Self::SEND,
            D::Recvonly => Self::RECV,
            D::Inactive => Self::INACTIVE,
            D::Sendrecv => Self::SEND | Self::RECV,
            D::__Nonexhaustive => {
                unreachable!("unexpected transceiver direction")
            }
        }
    }
}

impl From<TransceiverDirection> for RtcRtpTransceiverDirection {
    #[inline]
    fn from(direction: TransceiverDirection) -> Self {
        use TransceiverDirection as D;

        if direction.is_all() {
            Self::Sendrecv
        } else if direction.contains(D::RECV) {
            Self::Recvonly
        } else if direction.contains(D::SEND) {
            Self::Sendonly
        } else {
            Self::Inactive
        }
    }
}

impl From<&DirectionProto> for TransceiverDirection {
    #[inline]
    fn from(proto: &DirectionProto) -> Self {
        match proto {
            DirectionProto::Recv { .. } => Self::RECV,
            DirectionProto::Send { .. } => Self::SEND,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RtcRtpTransceiverDirection, TransceiverDirection};

    #[test]
    fn enable_works_correctly() {
        use TransceiverDirection as D;

        for (init, enable_dir, result) in [
            (D::INACTIVE, D::SEND, D::SEND),
            (D::INACTIVE, D::RECV, D::RECV),
            (D::SEND, D::RECV, D::all()),
            (D::RECV, D::SEND, D::all()),
        ] {
            assert_eq!(init | enable_dir, result);
        }
    }

    #[test]
    fn disable_works_correctly() {
        use TransceiverDirection as D;

        for (init, disable_dir, result) in [
            (D::SEND, D::SEND, D::INACTIVE),
            (D::RECV, D::RECV, D::INACTIVE),
            (D::all(), D::SEND, D::RECV),
            (D::all(), D::RECV, D::SEND),
        ] {
            assert_eq!(init - disable_dir, result);
        }
    }

    #[test]
    fn from_trnscvr_direction_to_sys() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (trnscvr_dir, sys_dir) in [
            (D::SEND, S::Sendonly),
            (D::RECV, S::Recvonly),
            (D::all(), S::Sendrecv),
            (D::INACTIVE, S::Inactive),
        ] {
            assert_eq!(S::from(trnscvr_dir), sys_dir);
        }
    }

    #[test]
    fn from_sys_direction_to_trnscvr() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (sys_dir, trnscvr_dir) in [
            (S::Sendonly, D::SEND),
            (S::Recvonly, D::RECV),
            (S::Sendrecv, D::all()),
            (S::Inactive, D::INACTIVE),
        ] {
            assert_eq!(D::from(sys_dir), trnscvr_dir);
        }
    }
}
