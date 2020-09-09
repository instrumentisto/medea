//! Implementation of the `MediaTrack` with a `Recv` direction.

use std::{cell::RefCell, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto as proto;
use medea_client_api_proto::{MemberId, ServerTrackPatch};
use proto::TrackId;
use web_sys::{MediaStreamTrack as RtcMediaStreamTrack, RtcRtpTransceiver};

use crate::{
    media::{MediaStreamTrack, RecvConstraints, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        media::mute_state::MuteStateController,
        MuteableTrack, PeerEvent, Track,
    },
};

use super::{mute_state::StableMuteState, HasMuteStateController};

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver(RefCell<InnerReceiver>);

struct InnerReceiver {
    track_id: TrackId,
    sender_id: MemberId,
    transceiver: Option<RtcRtpTransceiver>,
    transceiver_direction: TransceiverDirection,
    kind: TransceiverKind,
    mid: Option<String>,
    track: Option<MediaStreamTrack>,
    general_mute_state: StableMuteState,
    notified_track: bool,
    mute_state_controller: Rc<MuteStateController>,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
}

impl InnerReceiver {
    /// Sets this [`Receiver`]'s [`TransceiverDirection`] to the provided one.
    ///
    /// Tries to update underlying [`RtcRtpTransceiver`] if it present.
    fn set_direction(&mut self, direction: TransceiverDirection) {
        self.transceiver_direction = direction;
        if let Some(transceiver) = &self.transceiver {
            transceiver.set_direction(direction.into());
        }
    }

    /// Updates [`TransceiverDirection`] and underlying [`MediaStreamTrack`]
    /// based on the provided [`StableMuteState`].
    ///
    /// Updates [`InnerReceiver::general_mute_state`].
    ///
    /// If old general mute state same as provided - nothing will be done.
    fn update_general_mute_state(&mut self, mute_state: StableMuteState) {
        if self.general_mute_state != mute_state {
            self.general_mute_state = mute_state;
            match mute_state {
                StableMuteState::Muted => {
                    if let Some(track) = &self.track {
                        track.set_enabled(false);
                    }
                    self.set_direction(TransceiverDirection::Inactive);
                }
                StableMuteState::NotMuted => {
                    if let Some(track) = &self.track {
                        track.set_enabled(true);
                    }
                    self.set_direction(TransceiverDirection::Recvonly);
                }
            }
            self.maybe_notify_track();
        }
    }

    /// Checks whether general mute state of the [`Receiver`] is in
    /// [`MuteState::NotMuted`].
    fn is_not_muted(&self) -> bool {
        self.general_mute_state == StableMuteState::NotMuted
    }

    /// Checks whether general mute state of the [`Receiver`] is in
    /// [`MuteState::Muted`].
    fn is_muted(&self) -> bool {
        self.general_mute_state == StableMuteState::Muted
    }
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
    pub fn new(
        track_id: TrackId,
        caps: &TrackConstraints,
        sender_id: MemberId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        recv_constraints: &RecvConstraints,
    ) -> Rc<Self> {
        let kind = TransceiverKind::from(caps);
        let is_enabled = match kind {
            TransceiverKind::Audio => recv_constraints.is_audio_enabled(),
            TransceiverKind::Video => recv_constraints.is_video_enabled(),
        };
        let transceiver_direction = if is_enabled {
            TransceiverDirection::Recvonly
        } else {
            TransceiverDirection::Inactive
        };
        let transceiver = match mid {
            None => Some(peer.add_transceiver(kind, transceiver_direction)),
            Some(_) => None,
        };
        let mute_state_controller =
            MuteStateController::new(StableMuteState::from(!is_enabled));
        let this: Rc<Self> = Rc::new(Self(RefCell::new(InnerReceiver {
            track_id,
            sender_id,
            transceiver,
            transceiver_direction,
            kind,
            mid,
            mute_state_controller,
            general_mute_state: StableMuteState::from(!is_enabled),
            notified_track: false,
            track: None,
            peer_events_sender,
        })));

        this
    }

    /// Adds provided [`MediaStreamTrack`] and [`RtcRtpTransceiver`] to this
    /// [`Receiver`].
    ///
    /// Sets [`MediaStreamTrack::enabled`] same as [`Receiver::enabled`] of this
    /// [`Receiver`].
    pub fn set_remote_track(
        &self,
        transceiver: RtcRtpTransceiver,
        new_track: RtcMediaStreamTrack,
    ) {
        let mut inner = self.0.borrow_mut();

        if let Some(old_track) = &inner.track {
            if old_track.id() == new_track.id() {
                return;
            }
        }

        let new_track = MediaStreamTrack::from(new_track);

        transceiver.set_direction(inner.transceiver_direction.into());
        new_track.set_enabled(inner.is_not_muted());

        inner.transceiver.replace(transceiver);
        inner.track.replace(new_track);
        inner.maybe_notify_track();
    }

    /// Checks whether general mute state of the [`Receiver`] is in
    /// [`MuteState::Muted`].
    #[cfg(feature = "mockable")]
    pub fn is_general_muted(&self) -> bool {
        self.0.borrow().is_muted()
    }

    /// Updates [`Receiver`] with a provided [`TrackPatch`].
    pub fn update(&self, track_patch: &ServerTrackPatch) {
        let mut inner = self.0.borrow_mut();
        if inner.track_id != track_patch.id {
            return;
        }
        if let Some(is_muted_general) = track_patch.is_muted_general {
            inner.update_general_mute_state(is_muted_general.into());
        }
        if let Some(is_muted) = track_patch.is_muted_individual {
            inner.mute_state_controller.update_individual(is_muted);
        }
    }

    /// Checks underlying transceiver direction returning `true` if its
    /// [`TransceiverDirection::Recvonly`].
    // TODO: `sendrecv` is also true.
    pub fn is_receiving(&self) -> bool {
        self.0.borrow().is_receiving()
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub fn mid(&self) -> Option<String> {
        let mut inner = self.0.borrow_mut();

        if inner.mid.is_none() && inner.transceiver.is_some() {
            if let Some(transceiver) = &inner.transceiver {
                inner.mid = transceiver.mid();
            }
        }
        inner.mid.clone()
    }
}

impl InnerReceiver {
    /// Returns `true` if this [`Receiver`] is receives media data.
    fn is_receiving(&self) -> bool {
        if self.is_muted() {
            return false;
        }
        if self.transceiver.is_none() {
            return false;
        }
        match self.transceiver_direction {
            TransceiverDirection::Sendonly | TransceiverDirection::Inactive => {
                false
            }
            TransceiverDirection::Recvonly => true,
        }
    }

    /// Sends [`PeerEvent::NewRemoteTrack`] to the
    /// [`InnerReceiver::peer_events_sender`] if it's needed.
    fn maybe_notify_track(&mut self) {
        if self.notified_track {
            return;
        }
        if !self.is_receiving() {
            return;
        }
        if self.is_muted() {
            return;
        }
        if let Some(track) = &self.track {
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::NewRemoteTrack {
                    sender_id: self.sender_id.clone(),
                    track_id: self.track_id,
                    track: track.clone(),
                },
            );
            self.notified_track = true;
        }
    }
}

impl Track for Receiver {
    /// Returns [`TrackId`] of this [`Receiver`].
    #[inline]
    fn track_id(&self) -> TrackId {
        self.0.borrow().track_id
    }

    /// Returns [`TransceiverKind`] of this [`Receiver`].
    #[inline]
    fn kind(&self) -> TransceiverKind {
        self.0.borrow().kind
    }
}

impl HasMuteStateController for Receiver {
    /// Returns reference to the [`MuteStateController`] from this [`Receiver`].
    #[inline]
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.0.borrow().mute_state_controller.clone()
    }
}

impl MuteableTrack for Receiver {}
