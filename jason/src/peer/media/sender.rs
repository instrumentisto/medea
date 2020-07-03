//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{channel::mpsc, future, future::Either, StreamExt};
use medea_client_api_proto as proto;
use medea_client_api_proto::Mid;
use medea_reactive::ObservableCell;
use proto::{PeerId, TrackId};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        PeerEvent,
    },
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::{
    mute_state::{MuteState, StableMuteState},
    MediaConnectionsError, Result,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub peer_id: PeerId,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub peer: &'a RtcPeerConnection,
    pub peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    pub mid: Option<Mid>,
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

        let mute_state = ObservableCell::new(self.mute_state.into());
        // we dont care about initial state, cause transceiver is inactive atm
        let mut mute_state_changes = mute_state.subscribe().skip(1);
        let this = Rc::new(Sender {
            track_id: self.track_id,
            caps: self.caps,
            track: RefCell::new(None),
            transceiver,
            mute_state,
            mute_timeout_handle: RefCell::new(None),
            is_required: self.is_required,
            transceiver_direction: RefCell::new(TransceiverDirection::Inactive),
        });

        let weak_this = Rc::downgrade(&this);
        let peer_events_sender = self.peer_events_sender;
        let peer_id = self.peer_id;
        spawn_local(async move {
            while let Some(mute_state) = mute_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match mute_state {
                        MuteState::Stable(stable) => {
                            match stable {
                                StableMuteState::NotMuted => {
                                    let _ = peer_events_sender.unbounded_send(
                                        PeerEvent::NewLocalStreamRequired {
                                            peer_id,
                                        },
                                    );
                                }
                                StableMuteState::Muted => {
                                    // cannot fail
                                    this.track.borrow_mut().take();
                                    let _ = JsFuture::from(
                                        this.transceiver
                                            .sender()
                                            .replace_track(None),
                                    )
                                    .await;
                                }
                            }
                        }
                        MuteState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions =
                                    this.mute_state.subscribe().skip(1);
                                let (timeout, timeout_handle) =
                                    resettable_delay_for(
                                        Sender::MUTE_TRANSITION_TIMEOUT,
                                    );
                                this.mute_timeout_handle
                                    .borrow_mut()
                                    .replace(timeout_handle);
                                match future::select(
                                    transitions.next(),
                                    Box::pin(timeout),
                                )
                                .await
                                {
                                    Either::Left(_) => (),
                                    Either::Right(_) => {
                                        if let Some(this) = weak_this.upgrade()
                                        {
                                            let stable = this
                                                .mute_state
                                                .get()
                                                .cancel_transition();
                                            this.mute_state.set(stable);
                                        }
                                    }
                                }
                            });
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(this)
    }
}

/// Representation of a local [`MediaStreamTrack`] that is being sent to some
/// remote peer.
pub struct Sender {
    pub(super) track_id: TrackId,
    pub(super) caps: TrackConstraints,
    pub(super) track: RefCell<Option<MediaStreamTrack>>,
    pub(super) transceiver: RtcRtpTransceiver,
    pub(super) mute_state: ObservableCell<MuteState>,
    pub(super) mute_timeout_handle: RefCell<Option<ResettableDelayHandle>>,
    pub(super) is_required: bool,
    pub(super) transceiver_direction: RefCell<TransceiverDirection>,
}

impl Sender {
    #[cfg(not(feature = "mockable"))]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Returns `true` if this [`Sender`] is important and without it call
    /// session can't be started.
    pub fn is_required(&self) -> bool {
        self.caps.is_required()
    }

    /// Stops mute/unmute timeout of this [`Sender`].
    pub fn stop_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets mute/unmute timeout of this [`Sender`].
    pub fn reset_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Returns `true` if this [`Sender`] is publishes media traffic.
    pub fn is_publishing(&self) -> bool {
        let transceiver_direction = *self.transceiver_direction.borrow();
        match transceiver_direction {
            TransceiverDirection::Recvonly | TransceiverDirection::Inactive => {
                false
            }
            TransceiverDirection::Sendonly => true,
        }
    }

    /// Returns [`TrackId`] of this [`Sender`].
    pub fn track_id(&self) -> TrackId {
        self.track_id
    }

    /// Returns kind of [`RtcRtpTransceiver`] this [`Sender`].
    pub fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }

    /// Returns [`MuteState`] of this [`Sender`].
    pub fn mute_state(&self) -> MuteState {
        self.mute_state.get()
    }

    /// Returns [`Mid`] of this [`Sender`] from the underlying
    /// [`RtcRtpTransceiver`].
    pub(super) fn mid(&self) -> Option<Mid> {
        self.transceiver.mid().map(|mid| mid.into())
    }

    /// Inserts provided [`MediaStreamTrack`] into provided [`Sender`]s
    /// transceiver and enables transceivers sender by changing its
    /// direction to `sendonly`.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub(super) async fn insert_and_enable_track(
        sender: Rc<Self>,
        new_track: MediaStreamTrack,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = sender.track.borrow().as_ref() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        // no-op if transceiver is not NotMuted
        if let MuteState::Stable(StableMuteState::NotMuted) =
            sender.mute_state()
        {
            JsFuture::from(
                sender
                    .transceiver
                    .sender()
                    .replace_track(Some(new_track.as_ref())),
            )
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertTrack)
            .map_err(tracerr::wrap!())?;

            sender.track.borrow_mut().replace(new_track);

            sender.set_transceiver_direction(TransceiverDirection::Sendonly);
        }

        Ok(())
    }

    /// Sets provided [`TransceiverDirection`] of this [`Sender`]'s
    /// [`RtcRtpTransceiver`].
    ///
    /// Sets [`Sender::transceiver_direction`] to the provided
    /// [`TransceiverDirection`].
    fn set_transceiver_direction(&self, direction: TransceiverDirection) {
        self.transceiver.set_direction(direction.into());
        *self.transceiver_direction.borrow_mut() = direction;
    }

    /// Checks whether [`Sender`] is in [`MuteState::Muted`].
    pub fn is_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::Muted.into()
    }

    /// Checks whether [`Sender`] is in [`MuteState::NotMuted`].
    pub fn is_not_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::NotMuted.into()
    }

    /// Sets current [`MuteState`] to [`MuteState::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaconnectionsError::SenderIsRequired`] is returned if [`Sender`] is
    /// required for the call and can't be muted.
    pub fn mute_state_transition_to(
        &self,
        desired_state: StableMuteState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            let current_mute_state = self.mute_state.get();
            self.mute_state
                .set(current_mute_state.transition_to(desired_state));
            Ok(())
        }
    }

    /// Cancels [`MuteState`] transition.
    pub fn cancel_transition(&self) {
        let mute_state = self.mute_state.get();
        self.mute_state.set(mute_state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when [`MuteState`] of this
    /// [`Sender`] will be [`MuteState::Stable`] or the [`Sender`] is dropped.
    ///
    /// Succeeds if [`Sender`]'s [`MuteState`] transits into the `desired_state`
    /// or the [`Sender`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MuteStateTransitsIntoOppositeState`] is
    /// returned if [`Sender`]'s [`MuteState`] transits into the opposite to
    /// the `desired_state`.
    pub async fn when_mute_state_stable(
        self: Rc<Self>,
        desired_state: StableMuteState,
    ) -> Result<()> {
        let mut mute_states = self.mute_state.subscribe();
        while let Some(state) = mute_states.next().await {
            match state {
                MuteState::Transition(_) => continue,
                MuteState::Stable(s) => {
                    return if s == desired_state {
                        Ok(())
                    } else {
                        Err(tracerr::new!(
                                MediaConnectionsError::
                                MuteStateTransitsIntoOppositeState
                            ))
                    }
                }
            }
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
            let new_mute_state = StableMuteState::from(is_muted);
            let current_mute_state = self.mute_state.get();

            let mute_state_update: MuteState = match current_mute_state {
                MuteState::Stable(_) => new_mute_state.into(),
                MuteState::Transition(t) => {
                    if t.intended() == new_mute_state {
                        new_mute_state.into()
                    } else {
                        t.set_inner(new_mute_state).into()
                    }
                }
            };

            self.mute_state.set(mute_state_update);
        }
    }
}

impl Drop for Sender {
    /// Sets underlying [`RtcRtpTransceiver`]'s direction to the
    /// [`RtcRtpTransceiverDirection::Inactive`].
    ///
    /// Replaces sender in the underlying [`RtcRtpTransceiver`] with `None`.
    fn drop(&mut self) {
        if !self.transceiver.stopped() {
            self.transceiver
                .set_direction(RtcRtpTransceiverDirection::Inactive);
            let fut =
                JsFuture::from(self.transceiver.sender().replace_track(None));
            spawn_local(async move {
                let _ = fut.await;
            });
        }
    }
}
