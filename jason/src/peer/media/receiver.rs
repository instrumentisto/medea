//! Implementation of the `MediaTrack` with a `Recv` direction.

use std::cell::{Cell, RefCell};

use futures::channel::mpsc;
use medea_client_api_proto as proto;
use medea_client_api_proto::{MemberId, TrackPatch};
use proto::TrackId;
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, RecvConstraints, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        media::TransceiverSide,
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
    caps: TrackConstraints,
    sender_id: MemberId,
    transceiver: Option<RtcRtpTransceiver>,
    transceiver_direction: Cell<TransceiverDirection>,
    mid: RefCell<Option<String>>,
    track: Option<MediaStreamTrack>,
    enabled: bool,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be injected
    /// when [`MediaStreamTrack`] arrives.
    ///
    /// Created [`RtcRtpTransceiver`] direction is set to
    /// [`TransceiverDirection::Inactive`] if media receiving is disabled in
    /// provided [`RecvConstraints`].
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`MediaStreamTrack`] data
    /// arrives.
    pub(super) fn new(
        track_id: TrackId,
        caps: TrackConstraints,
        sender_id: MemberId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let kind = TransceiverKind::from(&caps);
        let enabled = match kind {
            TransceiverKind::Audio => recv_constraints.is_audio_enabled(),
            TransceiverKind::Video => recv_constraints.is_video_enabled(),
        };
        let transceiver_direction = if enabled {
            TransceiverDirection::Recvonly
        } else {
            TransceiverDirection::Inactive
        };
        let transceiver = match mid {
            None => Some(peer.add_transceiver(kind, transceiver_direction)),
            Some(_) => None,
        };
        Self {
            track_id,
            caps,
            sender_id,
            transceiver,
            transceiver_direction: Cell::new(transceiver_direction),
            mid: RefCell::new(mid),
            track: None,
            enabled,
            peer_events_sender,
        }
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

        if self.is_receiving() {
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::NewRemoteTrack {
                    sender_id: self.sender_id.clone(),
                    track_id: self.track_id,
                    track,
                },
            );
        }
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

    /// Returns `true` if current [`RtcRtpTransceiver`]s direction is
    /// [`TransceiverDirection::Recvonly`].
    pub fn is_receiving(&self) -> bool {
        match self.transceiver_direction.get() {
            TransceiverDirection::Sendonly | TransceiverDirection::Inactive => {
                false
            }
            TransceiverDirection::Recvonly => true,
        }
    }
}

impl TransceiverSide for Receiver {
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    fn mid(&self) -> Option<String> {
        if self.mid.borrow().is_none() && self.transceiver.is_some() {
            self.mid.replace(self.transceiver.as_ref().unwrap().mid());
        }
        self.mid.borrow().clone()
    }
}
