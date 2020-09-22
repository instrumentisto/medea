//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::{channel::mpsc, StreamExt};
use medea_client_api_proto::{PeerId, TrackId, TrackPatchEvent};
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
    mute_state::{MuteStateController, StableMuteState},
    MediaConnectionsError, Muteable, Result,
};
use crate::peer::conn::RTCPeerConnectionError::PeerConnectionEventBindFailed;

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
            general_mute_state: Cell::new(self.mute_state),
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
                                this.maybe_request_track();
                            }
                            StableMuteState::Muted => {
                                this.remove_track().await;
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
    general_mute_state: Cell<StableMuteState>,
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

    /// Updates [`Sender`]s general mute state based on the provided
    /// [`StableMuteState`].
    ///
    /// Sets [`Sender`]s underlying transceiver direction to
    /// [`TransceiverDirection::Inactive`] if provided mute state is
    /// [`StableMuteState::Muted`].
    ///
    /// Emits [`PeerEvent::NewLocalStreamRequired`] if new state is
    /// [`StableMuteState::Unmuted`] and [`Sender`] does not have a track to
    /// send.
    fn update_general_mute_state(&self, mute_state: StableMuteState) {
        if self.general_mute_state.get() != mute_state {
            self.general_mute_state.set(mute_state);
            match mute_state {
                StableMuteState::Unmuted => {
                    self.maybe_request_track();
                }
                StableMuteState::Muted => {
                    self.set_transceiver_direction(
                        TransceiverDirection::Inactive,
                    );
                }
            }
        }
    }

    /// Inserts provided [`MediaStreamTrack`] into provided [`Sender`]s
    /// transceiver. No-op if provided track already being used by this
    /// [`Sender`].
    pub(super) async fn insert_track(
        self: Rc<Self>,
        new_track: MediaStreamTrack,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.track.borrow().as_ref() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

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

        Ok(())
    }

    pub(super) fn free_track(&self) {
        self.track.borrow_mut().take();
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`TrackPatchEvent`].
    pub fn update(&self, track: &TrackPatchEvent) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted_individual {
            self.mute_state.update(is_muted);
        }
        if let Some(is_muted_general) = track.is_muted_general {
            self.update_general_mute_state(is_muted_general.into());
        }
    }

    /// Changes underlying transceiver direction to
    /// [`TransceiverDirection::Sendonly`] if this [`Receiver`]s general mute
    /// state is [`StableMuteState::Unmuted`].
    pub fn maybe_enable(&self) {
        if self.is_general_unmuted()
            && self.transceiver_direction.get()
                != TransceiverDirection::Sendonly
        {
            self.set_transceiver_direction(TransceiverDirection::Sendonly);
        }
    }

    /// Checks whether general mute state of the [`Sender`] is in
    /// [`StableMuteState::Muted`].
    #[cfg(feature = "mockable")]
    pub fn is_general_muted(&self) -> bool {
        self.general_mute_state.get() == StableMuteState::Muted
    }

    /// Checks whether general mute state of the [`Sender`] is in
    /// [`MuteState::Unmuted`].
    fn is_general_unmuted(&self) -> bool {
        self.general_mute_state.get() == StableMuteState::Unmuted
    }

    /// Sets provided [`TransceiverDirection`] of this [`Sender`]'s
    /// [`RtcRtpTransceiver`].
    fn set_transceiver_direction(&self, direction: TransceiverDirection) {
        self.transceiver.set_direction(direction.into());
        self.transceiver_direction.set(direction);
        let _ = self.peer_events_sender.unbounded_send(PeerEvent::TransceiverStatusUpdated { peer_id: self.peer_id });
    }

    /// Drops [`MediaStreamTrack`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to `None`.
    async fn remove_track(&self) {
        self.track.borrow_mut().take();
        // cannot fail
        JsFuture::from(self.transceiver.sender().replace_track(None))
            .await
            .unwrap();
    }

    /// Emits [`PeerEvent::NewLocalStreamRequired`] if [`Sender`] does not have
    /// a track to send.
    fn maybe_request_track(&self) {
        if self.track.borrow().is_none() {
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::NewLocalStreamRequired {
                    peer_id: self.peer_id,
                },
            );
        }
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
