//! Implementation of the wrapper around [`RtcRtpTranseiver`].

use bitflags::bitflags;
use derive_more::From;
use medea_client_api_proto::Direction as DirectionProto;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack as SysMediaStreamTrack, RtcRtpTransceiver,
    RtcRtpTransceiverDirection,
};

/// Wrapper around [`RtcRtpTransceiver`] which provides handy methods for
/// direction changes.
#[derive(Clone, From)]
pub struct Transceiver(RtcRtpTransceiver);

impl Transceiver {
    /// Returns current [`TransceiverDirection`] of this [`Transceiver`].
    fn current_direction(&self) -> TransceiverDirection {
        TransceiverDirection::from(self.0.direction())
    }

    /// Disables provided [`TransceiverDirection`] of this [`Transceiver`].
    pub fn disable(&self, disabled_direction: TransceiverDirection) {
        self.0.set_direction(
            self.current_direction().disable(disabled_direction).into(),
        );
    }

    /// Enables provided [`TransceiverDirection`] of this [`Transceiver`].
    pub fn enable(&self, enabled_direction: TransceiverDirection) {
        self.0.set_direction(
            self.current_direction().enable(enabled_direction).into(),
        );
    }

    /// Returns `true` if provided [`TransceiverDirection`] if enabled for this
    /// [`Transceiver`].
    pub fn is_enabled(&self, direction: TransceiverDirection) -> bool {
        self.current_direction().contains(direction)
    }

    /// Replaces [`TransceiverDirection::SEND`] [`SysMediaStreamTrack`] of this
    /// [`Transceiver`].
    pub async fn replace_sender_track(
        &self,
        new_track: Option<&SysMediaStreamTrack>,
    ) -> Result<JsValue, JsValue> {
        JsFuture::from(self.0.sender().replace_track(new_track)).await
    }

    /// Returns `mid` of this [`Transceiver`].
    pub fn mid(&self) -> Option<String> {
        self.0.mid()
    }
}

bitflags! {
    /// Representation of [RTCRtpTransceiverDirection][1].
    ///
    /// [`sendrecv` direction][2] can be represented by
    /// [`TransceiverDirection::all`] bitflag.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    /// [2]: https://tinyurl.com/yywbvbzx
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

impl TransceiverDirection {
    /// Enables provided [`TransceiverDirection`] in this
    /// [`TransceiverDirection`].
    fn enable(self, enabled_direction: Self) -> Self {
        self | enabled_direction
    }

    /// Disables provided [`TransceiverDirection`] in this
    /// [`TransceiverDirection`].
    fn disable(self, disabled_direction: Self) -> Self {
        self - disabled_direction
    }
}

impl From<RtcRtpTransceiverDirection> for TransceiverDirection {
    #[allow(clippy::match_wildcard_for_single_variants)]
    fn from(direction: RtcRtpTransceiverDirection) -> Self {
        use RtcRtpTransceiverDirection as D;

        match direction {
            D::Sendonly => Self::SEND,
            D::Recvonly => Self::RECV,
            D::Inactive => Self::INACTIVE,
            D::Sendrecv => Self::SEND | Self::RECV,
            _ => unreachable!("unexpected transceiver direction"),
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
    use super::*;

    #[test]
    fn enable_works_correctly() {
        use TransceiverDirection as D;

        for (init, enable_dir, result) in &[
            (D::INACTIVE, D::SEND, D::SEND),
            (D::INACTIVE, D::RECV, D::RECV),
            (D::SEND, D::RECV, D::all()),
            (D::RECV, D::SEND, D::all()),
        ] {
            assert_eq!(init.enable(*enable_dir), *result);
        }
    }

    #[test]
    fn disable_works_corretly() {
        use TransceiverDirection as D;

        for (init, disable_dir, result) in &[
            (D::SEND, D::SEND, D::INACTIVE),
            (D::RECV, D::RECV, D::INACTIVE),
            (D::all(), D::SEND, D::RECV),
            (D::all(), D::RECV, D::SEND),
        ] {
            assert_eq!(init.disable(*disable_dir), *result);
        }
    }

    #[test]
    fn from_trnsvr_direction_to_sys() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (trnsv_dir, sys_dir) in &[
            (D::SEND, S::Sendonly),
            (D::RECV, S::Recvonly),
            (D::all(), S::Sendrecv),
            (D::INACTIVE, S::Inactive),
        ] {
            assert_eq!(S::from(*trnsv_dir), *sys_dir);
        }
    }

    #[test]
    fn from_sys_direction_to_trnsvr() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (sys_dir, trnsv_dir) in &[
            (S::Sendonly, D::SEND),
            (S::Recvonly, D::RECV),
            (S::Sendrecv, D::all()),
            (S::Inactive, D::INACTIVE),
        ] {
            assert_eq!(D::from(*sys_dir), *trnsv_dir);
        }
    }
}
