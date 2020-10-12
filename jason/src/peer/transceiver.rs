use bitflags::bitflags;
use medea_client_api_proto::Direction as DirectionProto;
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

pub(super) struct Transceiver {
    transceiver: RtcRtpTransceiver,
    has_sender: bool,
    has_receiver: bool,
}

bitflags! {
    /// Representation of [RTCRtpTransceiverDirection][1].
    ///
    /// [1]:https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    pub struct TransceiverDirection: u32 {
        /// [`inactive` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y2zslyw2
        const INACTIVE = 0b0;
        /// [`sendonly` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y6y2ye97
        const SEND = 0b01;
        /// [`recvonly` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/y2nlxpzf
        const RECV = 0b10;
        /// [`sendrecv` direction][1] of transceiver.
        ///
        /// [1]: https://tinyurl.com/yywbvbzx
        const SENDRECV = 0b11;
    }
}

impl From<RtcRtpTransceiverDirection> for TransceiverDirection {
    fn from(direction: RtcRtpTransceiverDirection) -> Self {
        use RtcRtpTransceiverDirection as D;

        match direction {
            D::Sendonly => Self::SEND,
            D::Recvonly => Self::RECV,
            D::Inactive => Self::INACTIVE,
            D::Sendrecv => Self::SENDRECV,
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

        match direction {
            D::RECV => Self::Recvonly,
            D::SEND => Self::Sendonly,
            D::INACTIVE => Self::Inactive,
            D::SENDRECV => Self::Sendrecv,
            _ => unreachable!("TransceiverDirection bitflag is broken"),
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
