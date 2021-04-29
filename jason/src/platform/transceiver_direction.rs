use bitflags::bitflags;
use medea_client_api_proto::Direction as DirectionProto;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_sys::RtcRtpTransceiverDirection;

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

impl Into<i32> for TransceiverDirection {
    fn into(self) -> i32 {
        use TransceiverDirection as D;

        if self.is_all() {
            0
        } else if self.contains(D::RECV) {
            2
        } else if self.contains(D::SEND) {
            1
        } else {
            3
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
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

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
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

impl From<i32> for TransceiverDirection {
    fn from(i: i32) -> Self {
        match i {
            0 => TransceiverDirection::INACTIVE,
            1 => TransceiverDirection::SEND,
            2 => TransceiverDirection::RECV,
            _ => unreachable!("Unknown TransceiverDirection enum variant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RtcRtpTransceiverDirection, TransceiverDirection};

    #[test]
    fn enable_works_correctly() {
        use TransceiverDirection as D;

        for (init, enable_dir, result) in &[
            (D::INACTIVE, D::SEND, D::SEND),
            (D::INACTIVE, D::RECV, D::RECV),
            (D::SEND, D::RECV, D::all()),
            (D::RECV, D::SEND, D::all()),
        ] {
            assert_eq!(*init | *enable_dir, *result);
        }
    }

    #[test]
    fn disable_works_correctly() {
        use TransceiverDirection as D;

        for (init, disable_dir, result) in &[
            (D::SEND, D::SEND, D::INACTIVE),
            (D::RECV, D::RECV, D::INACTIVE),
            (D::all(), D::SEND, D::RECV),
            (D::all(), D::RECV, D::SEND),
        ] {
            assert_eq!(*init - *disable_dir, *result);
        }
    }

    #[test]
    fn from_trnscvr_direction_to_sys() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (trnscvr_dir, sys_dir) in &[
            (D::SEND, S::Sendonly),
            (D::RECV, S::Recvonly),
            (D::all(), S::Sendrecv),
            (D::INACTIVE, S::Inactive),
        ] {
            assert_eq!(S::from(*trnscvr_dir), *sys_dir);
        }
    }

    #[test]
    fn from_sys_direction_to_trnscvr() {
        use RtcRtpTransceiverDirection as S;
        use TransceiverDirection as D;

        for (sys_dir, trnscvr_dir) in &[
            (S::Sendonly, D::SEND),
            (S::Recvonly, D::RECV),
            (S::Sendrecv, D::all()),
            (S::Inactive, D::INACTIVE),
        ] {
            assert_eq!(D::from(*sys_dir), *trnscvr_dir);
        }
    }
}
