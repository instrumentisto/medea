use crate::media::MediaStreamTrack;
use bitflags::bitflags;
use medea_client_api_proto::Direction as DirectionProto;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack as SysMediaStreamTrack, RtcRtpTransceiver,
    RtcRtpTransceiverDirection,
};

#[derive(Clone)]
pub(super) struct Transceiver(Rc<Inner>);

struct Inner {
    transceiver: RtcRtpTransceiver,
    has_sender: Cell<bool>,
    has_receiver: Cell<bool>,
}

impl Transceiver {
    pub fn new(transceiver: RtcRtpTransceiver) -> Self {
        Self(Rc::new(Inner {
            transceiver,
            has_sender: Cell::new(false),
            has_receiver: Cell::new(false),
        }))
    }

    fn current_direction(&self) -> TransceiverDirection {
        TransceiverDirection::from(self.0.transceiver.direction())
    }

    pub fn has_sender(&self) -> bool {
        self.0.has_sender.get()
    }

    pub fn has_receiver(&self) -> bool {
        self.0.has_receiver.get()
    }

    pub fn disable(&self, disabled_direction: TransceiverDirection) {
        self.0.transceiver.set_direction(
            (self.current_direction() - disabled_direction).into(),
        );
    }

    pub fn enable(&self, enabled_direction: TransceiverDirection) {
        self.0.transceiver.set_direction(
            (self.current_direction() | enabled_direction).into(),
        );
    }

    pub fn is_enabled(&self, direction: TransceiverDirection) -> bool {
        self.current_direction().contains(direction)
    }

    pub async fn replace_sender_track(
        &self,
        new_track: Option<&SysMediaStreamTrack>,
    ) -> Result<JsValue, JsValue> {
        JsFuture::from(self.0.transceiver.sender().replace_track(new_track))
            .await
    }

    pub fn mid(&self) -> Option<String> {
        self.0.transceiver.mid()
    }
}

bitflags! {
    /// Representation of [RTCRtpTransceiverDirection][1].
    ///
    /// [1]:https://w3.org/TR/webrtc/#dom-rtcrtptransceiverdirection
    pub struct TransceiverDirection: u32 {
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
        const SENDRECV = Self::SEND.bits | Self::RECV.bits;
    }
}

impl From<RtcRtpTransceiverDirection> for TransceiverDirection {
    fn from(direction: RtcRtpTransceiverDirection) -> Self {
        use RtcRtpTransceiverDirection as D;

        match direction {
            D::Sendonly => Self::SEND,
            D::Recvonly => Self::RECV,
            D::Inactive => Self::empty(),
            D::Sendrecv => Self::SENDRECV,
            _ => unreachable!("unexpected transceiver direction"),
        }
    }
}

impl From<TransceiverDirection> for RtcRtpTransceiverDirection {
    #[inline]
    fn from(direction: TransceiverDirection) -> Self {
        use TransceiverDirection as D;

        if direction.contains(D::RECV) {
            Self::Recvonly
        } else if direction.contains(D::SEND) {
            Self::Sendonly
        } else if direction.contains(D::SENDRECV) {
            Self::Sendrecv
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
