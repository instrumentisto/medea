//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto as proto;
use proto::{PeerId, TrackId};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        media::TransceiverSide,
        PeerEvent,
    },
};

use super::{
    mute_state::{MuteState, MuteStateController, StableMuteState},
    MediaConnectionsError, Muteable, Result,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub peer_id: PeerId,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub peer: &'a RtcPeerConnection,
    pub peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    pub mid: Option<String>,
    pub mute_state: StableMuteState,
    pub is_required: bool,
}

impl<'a> SenderBuilder<'a> {
    /// Builds new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// retrieves existing [`RtcRtpTransceiver`] via provided `mid` from a
    /// provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`] lookup
    /// fails.
    pub fn build(self) -> Result<Rc<Sender>> {
        let kind = TransceiverKind::from(&self.caps);
        let transceiver = match self.mid {
            None => self
                .peer
                .add_transceiver(kind, TransceiverDirection::Inactive),
            Some(mid) => self
                .peer
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };

        let mute_state_observer = MuteStateController::new(self.mute_state);
        let mut mute_state_rx = mute_state_observer.on_stabilize();
        let this = Rc::new(Sender {
            peer_id: self.peer_id,
            track_id: self.track_id,
            caps: self.caps,
            track: RefCell::new(None),
            transceiver,
            mute_state: mute_state_observer,
            is_required: self.is_required,
            transceiver_direction: Cell::new(TransceiverDirection::Inactive),
            peer_events_sender: self.peer_events_sender,
        });
        spawn_local({
            let weak_this = Rc::downgrade(&this);
            async move {
                while let Some(mute_state) = mute_state_rx.next().await {
                    if let Some(this) = weak_this.upgrade() {
                        match mute_state {
                            StableMuteState::Unmuted => {
                                this.set_transceiver_direction(
                                    TransceiverDirection::Sendonly,
                                );
                                this.request_track();
                            }
                            StableMuteState::Muted => {
                                this.set_transceiver_direction(
                                    TransceiverDirection::Inactive,
                                );
                                this.disable().await;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        Ok(this)
    }
}

/// Representation of a local [`MediaStreamTrack`] that is being sent to some
/// remote peer.
pub struct Sender {
    peer_id: PeerId,
    track_id: TrackId,
    caps: TrackConstraints,
    track: RefCell<Option<MediaStreamTrack>>,
    transceiver: RtcRtpTransceiver,
    transceiver_direction: Cell<TransceiverDirection>,
    mute_state: Rc<MuteStateController>,
    is_required: bool,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
}

impl Sender {
    /// Returns [`TrackConstraints`] of this [`Sender`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns `true` if this [`Sender`] is publishing media traffic.
    pub fn is_publishing(&self) -> bool {
        match self.transceiver_direction.get() {
            TransceiverDirection::Recvonly | TransceiverDirection::Inactive => {
                false
            }
            TransceiverDirection::Sendonly => true,
        }
    }

    /// Inserts provided [`MediaStreamTrack`] into provided [`Sender`]s
    /// transceiver and enables transceivers sender by changing its
    /// direction to `sendonly`.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub(super) async fn insert_track_and_enable(
        self: Rc<Self>,
        new_track: MediaStreamTrack,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.track.borrow().as_ref() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        // no-op if transceiver is not Unmuted
        if let MuteState::Stable(StableMuteState::Unmuted) = self.mute_state() {
            JsFuture::from(
                self.transceiver
                    .sender()
                    .replace_track(Some(new_track.as_ref())),
            )
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertLocalTrack)
            .map_err(tracerr::wrap!())?;

            self.track.borrow_mut().replace(new_track);

            self.set_transceiver_direction(TransceiverDirection::Sendonly);
        }

        Ok(())
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`proto::TrackPatch`].
    pub fn update(&self, track: &proto::TrackPatch) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted {
            self.mute_state.update(is_muted);
        }
    }

    /// Sets provided [`TransceiverDirection`] of this [`Sender`]'s
    /// [`RtcRtpTransceiver`].
    fn set_transceiver_direction(&self, direction: TransceiverDirection) {
        self.transceiver.set_direction(direction.into());
        self.transceiver_direction.set(direction);
    }

    /// Disables this [`Sender`].
    async fn disable(&self) {
        self.track.borrow_mut().take();
        // cannot fail
        let _ = JsFuture::from(self.transceiver.sender().replace_track(None))
            .await
            .unwrap();
    }

    /// Sends [`PeerEvent::NewLocalStreamRequired`] to the
    /// [`Sender::peer_events_sender`].
    fn request_track(&self) {
        let _ = self.peer_events_sender.unbounded_send(
            PeerEvent::NewLocalStreamRequired {
                peer_id: self.peer_id,
            },
        );
    }
}

impl TransceiverSide for Sender {
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }

    fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }
}

impl Muteable for Sender {
    /// Returns reference to the [`MuteStateController`] of this [`Sender`].
    #[inline]
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.mute_state.clone()
    }

    /// Sets current [`MuteState`] to [`MuteState::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::SenderIsRequired`] is returned if [`Sender`] is
    /// required for the call and can't be muted.
    fn mute_state_transition_to(
        &self,
        desired_state: StableMuteState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            self.mute_state.transition_to(desired_state);
            Ok(())
        }
    }
}
