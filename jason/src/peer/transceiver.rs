use bitflags::bitflags;
use derive_more::From;
use medea_client_api_proto::Direction as DirectionProto;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack as SysMediaStreamTrack, RtcRtpTransceiver,
    RtcRtpTransceiverDirection,
};

#[derive(Clone, From)]
pub(super) struct Transceiver(RtcRtpTransceiver);

impl Transceiver {
    fn current_direction(&self) -> TransceiverDirection {
        TransceiverDirection::from(self.0.direction())
    }

    pub fn disable(&self, disabled_direction: TransceiverDirection) {
        self.0.set_direction(
            self.current_direction().disable(disabled_direction).into(),
        );
    }

    pub fn enable(&self, enabled_direction: TransceiverDirection) {
        self.0.set_direction(
            self.current_direction().enable(enabled_direction).into(),
        );
    }

    pub fn is_enabled(&self, direction: TransceiverDirection) -> bool {
        self.current_direction().contains(direction)
    }

    pub async fn replace_sender_track(
        &self,
        new_track: Option<&SysMediaStreamTrack>,
    ) -> Result<JsValue, JsValue> {
        JsFuture::from(self.0.sender().replace_track(new_track)).await
    }

    pub fn mid(&self) -> Option<String> {
        self.0.mid()
    }
}

bitflags! {
    /// Representation of [RTCRtpTransceiverDirection][1].
    ///
    /// [1]:https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    pub struct TransceiverDirection: u8 {
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
    fn disable(self, disabled_direction: Self) -> Self {
        self - disabled_direction
    }

    fn enable(self, enabled_direction: Self) -> Self {
        self | enabled_direction
    }
}

impl From<RtcRtpTransceiverDirection> for TransceiverDirection {
    fn from(direction: RtcRtpTransceiverDirection) -> Self {
        use RtcRtpTransceiverDirection as D;

        match direction {
            D::Sendonly => Self::SEND,
            D::Recvonly => Self::RECV,
            D::Inactive => Self::empty(),
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
        for (init, enable_dir, result) in &[
            (
                TransceiverDirection::empty(),
                TransceiverDirection::SEND,
                TransceiverDirection::SEND,
            ),
            (
                TransceiverDirection::empty(),
                TransceiverDirection::RECV,
                TransceiverDirection::RECV,
            ),
            (
                TransceiverDirection::SEND,
                TransceiverDirection::RECV,
                TransceiverDirection::all(),
            ),
            (
                TransceiverDirection::RECV,
                TransceiverDirection::SEND,
                TransceiverDirection::all(),
            ),
        ] {
            assert_eq!(init.enable(*enable_dir), *result);
        }
    }

    #[test]
    fn disable_works_corretly() {
        for (init, disable_dir, result) in &[
            (
                TransceiverDirection::SEND,
                TransceiverDirection::SEND,
                TransceiverDirection::empty(),
            ),
            (
                TransceiverDirection::RECV,
                TransceiverDirection::RECV,
                TransceiverDirection::empty(),
            ),
            (
                TransceiverDirection::all(),
                TransceiverDirection::SEND,
                TransceiverDirection::RECV,
            ),
            (
                TransceiverDirection::all(),
                TransceiverDirection::RECV,
                TransceiverDirection::SEND,
            ),
        ] {
            assert_eq!(init.disable(*disable_dir), *result);
        }
    }

    #[test]
    fn from_trnsvr_direction_to_sys() {
        for (trnsv_dir, sys_dir) in &[
            (
                TransceiverDirection::SEND,
                RtcRtpTransceiverDirection::Sendonly,
            ),
            (
                TransceiverDirection::RECV,
                RtcRtpTransceiverDirection::Recvonly,
            ),
            (
                TransceiverDirection::all(),
                RtcRtpTransceiverDirection::Sendrecv,
            ),
            (
                TransceiverDirection::empty(),
                RtcRtpTransceiverDirection::Inactive,
            ),
        ] {
            assert_eq!(RtcRtpTransceiverDirection::from(*trnsv_dir), *sys_dir);
        }
    }

    #[test]
    fn from_sys_direction_to_trnsvr() {
        for (sys_dir, trnsv_dir) in &[
            (
                RtcRtpTransceiverDirection::Sendonly,
                TransceiverDirection::SEND,
            ),
            (
                RtcRtpTransceiverDirection::Recvonly,
                TransceiverDirection::RECV,
            ),
            (
                RtcRtpTransceiverDirection::Sendrecv,
                TransceiverDirection::all(),
            ),
            (
                RtcRtpTransceiverDirection::Inactive,
                TransceiverDirection::empty(),
            ),
        ] {
            assert_eq!(TransceiverDirection::from(*sys_dir), *trnsv_dir);
        }
    }
}
