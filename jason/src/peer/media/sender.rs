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
        PeerEvent,
    },
};

use super::{
    mute_state::{MuteState, MuteStateController, StableMuteState},
    HasMuteStateController, MediaConnectionsError, MuteableTrack, Result,
    Track,
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
        let mut individual_mute_state_rx =
            mute_state_observer.on_individual_update();
        let this = Rc::new(Sender {
            peer_id: self.peer_id,
            track_id: self.track_id,
            caps: self.caps,
            track: RefCell::new(None),
            general_mute_state: Cell::new(self.mute_state.into()),
            transceiver,
            mute_state_controller: mute_state_observer,
            is_required: self.is_required,
            transceiver_direction: Cell::new(TransceiverDirection::Inactive),
            peer_events_sender: self.peer_events_sender,
        });
        spawn_local({
            let weak_this = Rc::downgrade(&this);
            async move {
                while let Some(individual_mute_state) =
                    individual_mute_state_rx.next().await
                {
                    if let Some(this) = weak_this.upgrade() {
                        match individual_mute_state {
                            StableMuteState::Muted => {
                                this.disable().await;
                            }
                            StableMuteState::NotMuted => (),
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
    mute_state_controller: Rc<MuteStateController>,
    general_mute_state: Cell<StableMuteState>,
    is_required: bool,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
}

impl Sender {
    /// Returns [`TrackConstraints`] of this [`Sender`].
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns [`RtcRtpTransceiver`] of this [`Sender`].
    pub fn transceiver(&self) -> &RtcRtpTransceiver {
        &self.transceiver
    }

    /// Returns `true` if this [`Sender`] is publishes media traffic.
    pub fn is_publishing(&self) -> bool {
        match self.transceiver_direction.get() {
            TransceiverDirection::Recvonly | TransceiverDirection::Inactive => {
                false
            }
            TransceiverDirection::Sendonly => true,
        }
    }

    /// Checks whether general mute state of the [`Receiver`] is in
    /// [`MuteState::Muted`].
    #[cfg(feature = "mockable")]
    pub fn is_general_muted(&self) -> bool {
        self.general_mute_state.get() == StableMuteState::Muted
    }

    /// Updates [`TransceiverDirection`] and underlying [`MediaStreamTrack`]
    /// based on the provided [`StableMuteState`].
    ///
    /// Updates [`Sender::general_mute_state`].
    ///
    /// If old general mute state same as provided - nothing will be done.
    fn update_general_mute_state(&self, mute_state: StableMuteState) {
        if self.general_mute_state.get() != mute_state {
            self.general_mute_state.set(mute_state);
            match mute_state {
                StableMuteState::NotMuted => {
                    self.set_transceiver_direction(
                        TransceiverDirection::Sendonly,
                    );
                    self.request_track();
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

        // no-op if transceiver is not NotMuted
        if let MuteState::Stable(StableMuteState::NotMuted) =
            self.individual_mute_state()
        {
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

            if self.general_mute_state.get() == StableMuteState::NotMuted {
                self.set_transceiver_direction(TransceiverDirection::Sendonly);
            }
        }

        Ok(())
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`proto::TrackPatch`].
    pub fn update(&self, track: &proto::ServerTrackPatch) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted_individual {
            self.mute_state_controller.update_individual(is_muted);
        }
        if let Some(is_muted_general) = track.is_muted_general {
            self.update_general_mute_state(is_muted_general.into());
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

impl Track for Sender {
    /// Returns [`TrackId`] of this [`Sender`].
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    /// Returns kind of [`RtcRtpTransceiver`] this [`Sender`].
    #[inline]
    fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }
}

impl HasMuteStateController for Sender {
    /// Returns reference to the [`MuteStateController`] of this [`Sender`].
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.mute_state_controller.clone()
    }
}

impl MuteableTrack for Sender {
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
            self.mute_state_controller.transition_to(desired_state);
            Ok(())
        }
    }
}
