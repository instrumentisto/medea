use std::convert::From;

use medea_client_api_proto as proto;
use proto::{PeerId, TrackId};
use web_sys::RtcRtpTransceiver;

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
    pub(super) mid: Option<String>,
    pub(super) track: Option<MediaStreamTrack>,
    pub(super) is_required: bool,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be
    /// injected when [`MediaStreamTrack`] arrives.
    ///
    /// `track` field in the created [`Receiver`] will be `None`,
    /// since [`Receiver`] must be created before the actual
    /// [`MediaStreamTrack`] data arrives.
    #[inline]
    pub(super) fn new(
        track_id: TrackId,
        caps: &TrackConstraints,
        sender_id: PeerId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Self {
        let is_required = caps.is_required();
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
            is_required,
        }
    }

    /// Returns `true` if this [`Sender`] is important and without it call
    /// session can't be started.
    pub fn is_required(&self) -> bool {
        self.is_required
    }

    /// Returns associated [`MediaStreamTrack`] with this [`Receiver`], if any.
    #[inline]
    pub(crate) fn track(&self) -> Option<MediaStreamTrack> {
        self.track.as_ref().cloned()
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub(crate) fn mid(&mut self) -> Option<&str> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid()
        }
        self.mid.as_deref()
    }
}
