//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::RefCell, rc::Rc, time::Duration};

use futures::{channel::mpsc, future, future::Either, StreamExt};
use medea_client_api_proto as proto;
use medea_reactive::ObservableCell;
use proto::{PeerId, TrackId};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::RtcRtpTransceiver;

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::{
        conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
        PeerEvent,
    },
    utils::{resettable_delay_for, ResettableDelayHandle},
};

use super::{
    publish_state::{PublishState, StablePublishState},
    MediaConnectionsError, Result,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub peer_id: PeerId,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub peer: &'a RtcPeerConnection,
    pub peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    pub mid: Option<String>,
    pub publish_state: StablePublishState,
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

        let publish_state = ObservableCell::new(self.publish_state.into());
        // we dont care about initial state, cause transceiver is inactive atm
        let mut publish_state_changes = publish_state.subscribe().skip(1);
        let this = Rc::new(Sender {
            track_id: self.track_id,
            caps: self.caps,
            track: RefCell::new(None),
            transceiver,
            publish_state,
            publish_transition_timeout_handle: RefCell::new(None),
            is_required: self.is_required,
            transceiver_direction: RefCell::new(TransceiverDirection::Inactive),
        });

        let weak_this = Rc::downgrade(&this);
        let peer_events_sender = self.peer_events_sender;
        let peer_id = self.peer_id;
        spawn_local(async move {
            while let Some(publish_state) = publish_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match publish_state {
                        PublishState::Stable(stable) => {
                            match stable {
                                StablePublishState::Enabled => {
                                    let _ = peer_events_sender.unbounded_send(
                                        PeerEvent::NewLocalStreamRequired {
                                            peer_id,
                                        },
                                    );
                                }
                                StablePublishState::Disabled => {
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
                        PublishState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions =
                                    this.publish_state.subscribe().skip(1);
                                let (timeout, timeout_handle) =
                                    resettable_delay_for(
                                        Sender::MUTE_TRANSITION_TIMEOUT,
                                    );
                                this.publish_transition_timeout_handle
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
                                                .publish_state
                                                .get()
                                                .cancel_transition();
                                            this.publish_state.set(stable);
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
    pub(super) publish_state: ObservableCell<PublishState>,
    pub(super) publish_transition_timeout_handle:
        RefCell<Option<ResettableDelayHandle>>,
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

    /// Stops enable/disable timeout of this [`Sender`].
    pub fn stop_publish_state_transition_timeout(&self) {
        if let Some(timer) = &*self.publish_transition_timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets mute/unmute timeout of this [`Sender`].
    pub fn reset_publish_state_transition_timeout(&self) {
        if let Some(timer) = &*self.publish_transition_timeout_handle.borrow() {
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

    /// Returns [`PublishState`] of this [`Sender`].
    pub fn publish_state(&self) -> PublishState {
        self.publish_state.get()
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

        // no-op if transceiver is not Enabled
        if let PublishState::Stable(StablePublishState::Enabled) =
            sender.publish_state()
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

    /// Checks whether [`Sender`] is in [`PublishState::Disabled`].
    pub fn is_disabled(&self) -> bool {
        self.publish_state.get() == StablePublishState::Disabled.into()
    }

    /// Checks whether [`Sender`] is in [`PublishState::Enabled`].
    pub fn is_enabled(&self) -> bool {
        self.publish_state.get() == StablePublishState::Enabled.into()
    }

    /// Sets current [`PublishState`] to [`PublishState::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaconnectionsError::SenderIsRequired`] is returned if [`Sender`] is
    /// required for the call and can't be disabled.
    pub fn publish_state_transition_to(
        &self,
        desired_state: StablePublishState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            let current_publish_state = self.publish_state.get();
            self.publish_state
                .set(current_publish_state.transition_to(desired_state));
            Ok(())
        }
    }

    /// Cancels [`PublishState`] transition.
    pub fn cancel_transition(&self) {
        let publish_state = self.publish_state.get();
        self.publish_state.set(publish_state.cancel_transition());
    }

    /// Returns [`Future`] which will be resolved when [`PublishState`] of this
    /// [`Sender`] will be [`PublishState::Stable`] or the [`Sender`] is
    /// dropped.
    ///
    /// Succeeds if [`Sender`]'s [`PublishState`] transits into the
    /// `desired_state` or the [`Sender`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::PublishStateTransitsIntoOppositeState`] is
    /// returned if [`Sender`]'s [`PublishState`] transits into the opposite to
    /// the `desired_state`.
    pub async fn when_publish_state_stable(
        self: Rc<Self>,
        desired_state: StablePublishState,
    ) -> Result<()> {
        let mut publish_states = self.publish_state.subscribe();
        while let Some(state) = publish_states.next().await {
            match state {
                PublishState::Transition(_) => continue,
                PublishState::Stable(s) => {
                    return if s == desired_state {
                        Ok(())
                    } else {
                        Err(tracerr::new!(
                                MediaConnectionsError::
                                PublishStateTransitsIntoOppositeState
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

        if let Some(is_enabled) = track.is_enabled {
            let new_publish_state = StablePublishState::from(is_enabled);
            let current_publish_state = self.publish_state.get();

            let publish_state_update: PublishState = match current_publish_state
            {
                PublishState::Stable(_) => new_publish_state.into(),
                PublishState::Transition(t) => {
                    if t.intended() == new_publish_state {
                        new_publish_state.into()
                    } else {
                        t.set_inner(new_publish_state).into()
                    }
                }
            };

            self.publish_state.set(publish_state_update);
        }
    }
}
