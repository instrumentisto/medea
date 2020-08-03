//! Implementation of the `MediaTrack` with a `Recv` direction.

use futures::channel::mpsc;
use medea_client_api_proto as proto;
use medea_client_api_proto::{MemberId, TrackPatch};
use proto::TrackId;
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        PeerEvent,
    },
};

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    sender_id: MemberId,
    transceiver: Option<RtcRtpTransceiver>,
    kind: TransceiverKind,
    mid: Option<String>,
    track: Option<MediaStreamTrack>,
    enabled: bool,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
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
        sender_id: MemberId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
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
            kind,
            mid,
            track: None,
            enabled: true,
            peer_events_sender,
        }
    }

    pub fn track_id(&self) -> TrackId {
        self.track_id
    }

    /// Adds provided [`MediaStreamTrack`] and [`RtcRtpTransceiver`] to this
    /// [`Receiver`].
    ///
    /// Sets [`MediaStreamTrack::enabled`] same as [`Receiver::enabled`] of this
    /// [`Receiver`].
    pub fn set_remote_track(
        &mut self,
        transceiver: RtcRtpTransceiver,
        track: MediaStreamTrack,
    ) {
        self.transceiver.replace(transceiver);
        self.track.replace(track.clone());
        track.set_enabled(self.enabled);

        let _ =
            self.peer_events_sender
                .unbounded_send(PeerEvent::NewRemoteTrack {
                    sender_id: self.sender_id.clone(),
                    track_id: self.track_id,
                    track,
                });
    }

    /// Updates [`Receiver`] with a provided [`TrackPatch`].
    pub fn update(&mut self, track_patch: &TrackPatch) {
        if let Some(is_muted) = track_patch.is_muted {
            self.enabled = !is_muted;
            if let Some(track) = &self.track {
                track.set_enabled(!is_muted);
            }
        }
    }

    #[inline]
    pub fn kind(&self) -> TransceiverKind {
        self.kind
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
