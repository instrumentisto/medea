//! Implementation of the `MediaTrack` with a `Recv` direction.

use std::{cell::RefCell, rc::Rc};

use futures::{channel::mpsc, future::LocalBoxFuture, StreamExt as _};
use medea_client_api_proto as proto;
use medea_client_api_proto::{MemberId, TrackPatch};
use proto::TrackId;
use wasm_bindgen_futures::spawn_local;
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, RecvConstraints, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        media::{mute_state::MuteStateController, Result},
        MuteState, MuteableTrack, PeerEvent, Track,
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
    notified_track: bool,
    mute_state_controller: Rc<MuteStateController>,
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
        recv_constraints: &RecvConstraints,
    ) -> Rc<Self> {
        let kind = TransceiverKind::from(caps);
        let muted = match kind {
            TransceiverKind::Audio => recv_constraints.is_audio_disabled(),
            TransceiverKind::Video => recv_constraints.is_video_disabled(),
        };
        let transceiver_direction = if muted {
            TransceiverDirection::Inactive
        } else {
            TransceiverDirection::Recvonly
        };
        let transceiver = match mid {
            None => Some(peer.add_transceiver(kind, transceiver_direction)),
            Some(_) => None,
        };
        let mute_state_controller = MuteStateController::new(muted.into());
        let mut on_finalized_mute_state = mute_state_controller.on_finalized();
        let this: Rc<Self> = Rc::new(Self(RefCell::new(InnerReceiver {
            track_id,
            sender_id,
            transceiver,
            transceiver_direction,
            kind,
            mid,
            mute_state_controller,
            notified_track: false,
            track: None,
            peer_events_sender,
        })));

        spawn_local({
            let weak_this = Rc::downgrade(&this);

            async move {
                while let Some(finalize_mute_state) =
                    on_finalized_mute_state.next().await
                {
                    if let Some(this) = weak_this.upgrade() {
                        let mut inner = this.0.borrow_mut();
                        match finalize_mute_state {
                            StableMuteState::Muted => {
                                if let Some(track) = &inner.track {
                                    track.set_enabled(false);
                                }
                                if let Some(transceiver) = &inner.transceiver {
                                    transceiver.set_direction(
                                        inner.transceiver_direction.into(),
                                    );
                                }
                                if inner.transceiver.is_some() {
                                    inner.transceiver_direction =
                                        TransceiverDirection::Inactive;
                                }
                            }
                            StableMuteState::NotMuted => {
                                if let Some(track) = &inner.track {
                                    track.set_enabled(true);
                                }
                                if let Some(transceiver) = &inner.transceiver {
                                    transceiver.set_direction(
                                        inner.transceiver_direction.into(),
                                    );
                                }
                                if inner.transceiver.is_some() {
                                    inner.transceiver_direction =
                                        TransceiverDirection::Recvonly;
                                }
                            }
                        }
                    }
                }
            }
        });

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
        track: MediaStreamTrack,
    ) {
        let mut inner = self.0.borrow_mut();

        transceiver.set_direction(inner.transceiver_direction.into());
        track.set_enabled(inner.mute_state_controller.is_not_muted());

        inner.transceiver.replace(transceiver);
        inner.track.replace(track);
        inner.maybe_notify_track();
    }

    /// Updates [`Receiver`] with a provided [`TrackPatch`].
    pub fn update(&self, track_patch: &TrackPatch) {
        if let Some(is_muted) = track_patch.is_muted {
            self.0.borrow().mute_state_controller.update(is_muted);
        }
    }

    /// Checks underlying transceiver direction returning `true` if its
    /// [`TransceiverDirection::Recvonly`].
    // TODO: `sendrecv` is also true.
    pub fn is_receiving(&self) -> bool {
        self.0.borrow().is_receiving()
    }

    /// Returns [`TransceiverKind`] of this [`Receiver`].
    #[inline]
    pub fn kind(&self) -> TransceiverKind {
        self.0.borrow().kind
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
    fn is_receiving(&self) -> bool {
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

    fn maybe_notify_track(&mut self) {
        if self.notified_track {
            return;
        }
        if !self.is_receiving() {
            return;
        }
        if self.mute_state_controller.is_muted() {
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
    /// Returns [`TrackId`] of this [`Sender`].
    fn track_id(&self) -> TrackId {
        self.0.borrow().track_id
    }
}

impl HasMuteStateController for Receiver {
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.0.borrow().mute_state_controller.clone()
    }
}

impl MuteableTrack for Receiver {}
