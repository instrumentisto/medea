//! Implementation of the `MediaTrack` with a `Recv` direction.

use medea_client_api_proto as proto;
use medea_client_api_proto::Mid;
use proto::{PeerId, TrackId};
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
};

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver {
    pub(super) track_id: TrackId,
    pub(super) sender_id: PeerId,
    pub(super) transceiver: Option<RtcRtpTransceiver>,
    pub(super) mid: Option<Mid>,
    pub(super) track: Option<MediaStreamTrack>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be injected
    /// when [`MediaStreamTrack`] arrives.
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`MediaStreamTrack`] data
    /// arrives.
    pub(super) fn new(
        track_id: TrackId,
        caps: &TrackConstraints,
        sender_id: PeerId,
        peer: &RtcPeerConnection,
        mid: Option<Mid>,
    ) -> Self {
        let kind = TransceiverKind::from(caps);
        let transceiver = match mid {
            None => {
                Some(peer.add_transceiver(kind, TransceiverDirection::Recvonly))
            }
            Some(_) => None,
        };
        Self {
            track_id,
            sender_id,
            transceiver,
            mid,
            track: None,
        }
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub(super) fn mid(&mut self) -> Option<Mid> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid().map(Into::into)
        }
        self.mid.clone()
    }
}

impl Drop for Receiver {
    /// Sets underlying [`RtcRtpTransceiver`]'s direction to the
    /// [`RtcRtpTransceiverDirection::Inactive`].
    fn drop(&mut self) {
        if let Some(transceiver) = &self.transceiver {
            if !transceiver.stopped() {
                transceiver.set_direction(RtcRtpTransceiverDirection::Inactive);
            }
        }
    }
}
