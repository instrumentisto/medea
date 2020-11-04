//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use futures::{channel::mpsc, future::LocalBoxFuture, StreamExt};
use medea_client_api_proto::{
    MediaSourceKind, PeerId, TrackId, TrackPatchEvent,
};
use wasm_bindgen_futures::spawn_local;

use crate::{
    media::{
        LocalTracksConstraints, MediaKind, MediaStreamTrack, TrackConstraints,
        VideoSource,
    },
    peer::{
        media::TransceiverSide,
        transceiver::{Transceiver, TransceiverDirection},
        PeerEvent,
    },
};

use super::{
    transitable_state::{StableMediaExchangeState, TransitableStateController},
    Disableable, MediaConnections, MediaConnectionsError, Result,
};
use crate::peer::media::transitable_state::{
    MediaExchangeStateController, MuteState, MuteStateController,
    StableMuteState, TrackMediaState,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub media_connections: &'a MediaConnections,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub mid: Option<String>,
    pub media_exchange_state: StableMediaExchangeState,
    pub mute_state: StableMuteState,
    pub is_required: bool,
    pub send_constraints: LocalTracksConstraints,
}

impl<'a> SenderBuilder<'a> {
    /// Builds new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// retrieves existing [`RtcRtpTransceiver`] via provided `mid` from a
    /// provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`] lookup
    /// fails.
    pub fn build(self) -> Result<Rc<Sender>> {
        let connections = self.media_connections.0.borrow();
        let kind = MediaKind::from(&self.caps);
        let transceiver = match self.mid {
            // Try to find rcvr transceiver that can be used as sendrecv.
            None => connections
                .receivers
                .values()
                .find(|rcvr| {
                    rcvr.caps().media_kind() == self.caps.media_kind()
                        && rcvr.caps().media_source_kind()
                            == self.caps.media_source_kind()
                })
                .and_then(|rcvr| rcvr.transceiver())
                .unwrap_or_else(|| {
                    connections
                        .add_transceiver(kind, TransceiverDirection::INACTIVE)
                }),
            Some(mid) => connections
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };

        let media_exchange_state_controller =
            TransitableStateController::new(self.media_exchange_state);
        let mut media_exchange_state_rx =
            media_exchange_state_controller.on_stabilize();
        let mute_state_controller =
            TransitableStateController::new(self.mute_state);
        let mut mute_state_rx = mute_state_controller.on_stabilize();
        let this = Rc::new(Sender {
            peer_id: connections.peer_id,
            track_id: self.track_id,
            caps: self.caps,
            general_media_exchange_state: Cell::new(self.media_exchange_state),
            mute_state: mute_state_controller,
            transceiver,
            media_exchange_state: media_exchange_state_controller,
            is_required: self.is_required,
            peer_events_sender: connections.peer_events_sender.clone(),
            send_constraints: self.send_constraints,
        });
        spawn_local({
            let weak_this = Rc::downgrade(&this);
            async move {
                while let Some(media_exchange_state) =
                    media_exchange_state_rx.next().await
                {
                    if let Some(this) = weak_this.upgrade() {
                        match media_exchange_state {
                            StableMediaExchangeState::Enabled => {
                                this.maybe_request_track();
                            }
                            StableMediaExchangeState::Disabled => {
                                this.remove_track().await;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        });
        spawn_local({
            let weak_this = Rc::downgrade(&this);
            async move {
                while let Some(mute_state) = mute_state_rx.next().await {
                    if let Some(this) = weak_this.upgrade() {
                        match mute_state {
                            StableMuteState::Unmuted => {
                                this.transceiver.set_sender_enabled(true);
                            }
                            StableMuteState::Muted => {
                                this.transceiver.set_sender_enabled(false);
                            }
                        }
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
    transceiver: Transceiver,
    media_exchange_state: Rc<MediaExchangeStateController>,
    mute_state: Rc<MuteStateController>,
    general_media_exchange_state: Cell<StableMediaExchangeState>,
    is_required: bool,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    send_constraints: LocalTracksConstraints,
}

impl Sender {
    /// Returns [`TrackConstraints`] of this [`Sender`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns `true` if this [`Sender`] is publishing media traffic.
    pub fn is_publishing(&self) -> bool {
        self.transceiver.has_direction(TransceiverDirection::SEND)
    }

    pub fn source_kind(&self) -> MediaSourceKind {
        self.caps.media_source_kind()
    }

    /// Updates [`Sender`]s general media exchange state based on the provided
    /// [`StableMediaExchangeState`].
    ///
    /// Sets [`Sender`]s underlying transceiver direction to
    /// [`TransceiverDirection::Inactive`] if provided media exchange state is
    /// [`StableMediaExchangeState::Disabled`].
    ///
    /// Emits [`PeerEvent::NewLocalStreamRequired`] if new state is
    /// [`StableMediaExchangeState::Enabled`] and [`Sender`] does not have a
    /// track to send.
    fn update_general_media_exchange_state(
        &self,
        media_exchange_state: StableMediaExchangeState,
    ) {
        if self.general_media_exchange_state.get() != media_exchange_state {
            self.general_media_exchange_state.set(media_exchange_state);
            match media_exchange_state {
                StableMediaExchangeState::Enabled => {
                    self.maybe_request_track();
                    if self.is_enabled_in_cons() {
                        self.transceiver
                            .add_direction(TransceiverDirection::SEND);
                    }
                }
                StableMediaExchangeState::Disabled => {
                    self.transceiver.sub_direction(TransceiverDirection::SEND);
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
        if let Some(current_track) = self.transceiver.send_track() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        new_track.set_enabled(
            self.mute_state.media_exchange_state().cancel_transition()
                == MuteState::Stable(StableMuteState::Unmuted),
        );

        self.transceiver
            .set_send_track(Some(new_track))
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertLocalTrack)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Indicates whether this [`Sender`] is enabled in
    /// [`LocalStreamConstraints`].
    fn is_enabled_in_cons(&self) -> bool {
        self.send_constraints.is_track_enabled(
            self.caps.media_kind(),
            self.caps.media_source_kind(),
        )
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`TrackPatchEvent`].
    pub fn update(&self, track: &TrackPatchEvent) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted {
            self.mute_state.update(is_muted);
        }
        if let Some(is_disabled) = track.is_disabled_individual {
            self.media_exchange_state.update(is_disabled);
        }
        if let Some(is_disabled_general) = track.is_disabled_general {
            self.update_general_media_exchange_state(
                is_disabled_general.into(),
            );
        }
    }

    /// Changes underlying transceiver direction to
    /// [`TransceiverDirection::Sendonly`] if this [`Receiver`]s general media
    /// exchange state is [`StableMediaExchangeState::Enabled`].
    pub fn maybe_enable(&self) {
        if self.is_general_enabled()
            && !self.transceiver.has_direction(TransceiverDirection::SEND)
            && self.is_enabled_in_cons()
        {
            self.transceiver.add_direction(TransceiverDirection::SEND);
        }
    }

    /// Returns [`Transceiver`] of this [`Sender`].
    pub fn transceiver(&self) -> Transceiver {
        self.transceiver.clone()
    }

    /// Checks whether general media exchange state of the [`Sender`] is in
    /// [`StableMediaExchangeState::Disabled`].
    #[cfg(feature = "mockable")]
    pub fn is_general_disabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == StableMediaExchangeState::Disabled
    }

    /// Checks whether general media exchange state of the [`Sender`] is in
    /// [`MediaExchangeState::Enabled`].
    fn is_general_enabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == StableMediaExchangeState::Enabled
    }

    /// Drops [`MediaStreamTrack`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to `None`.
    async fn remove_track(&self) {
        // cannot fail
        self.transceiver.set_send_track(None).await.unwrap();
    }

    /// Emits [`PeerEvent::NewLocalStreamRequired`] if [`Sender`] does not have
    /// a track to send.
    fn maybe_request_track(&self) {
        if self.transceiver.send_track().is_none() {
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::NewLocalStreamRequired {
                    peer_id: self.peer_id,
                },
            );
        }
    }

    pub fn mute_state(&self) -> MuteState {
        self.mute_state.media_exchange_state()
    }

    pub fn mute_state_transition_to(&self, desired_state: StableMuteState) {
        self.mute_state.transition_to(desired_state);
    }

    pub fn when_mute_state_stable(
        &self,
        desired_state: StableMuteState,
    ) -> LocalBoxFuture<'static, Result<()>> {
        self.mute_state
            .when_media_exchange_state_stable(desired_state)
    }
}

impl TransceiverSide for Sender {
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    fn kind(&self) -> MediaKind {
        MediaKind::from(&self.caps)
    }

    fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }

    fn is_transitable(&self) -> bool {
        match &self.caps {
            TrackConstraints::Video(VideoSource::Device(_)) => {
                self.send_constraints.inner().get_device_video().is_some()
            }
            TrackConstraints::Video(VideoSource::Display(_)) => {
                self.send_constraints.inner().get_display_video().is_some()
            }
            TrackConstraints::Audio(_) => true,
        }
    }
}

impl Disableable for Sender {
    #[inline]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        self.media_exchange_state.clone()
    }

    #[inline]
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        self.mute_state.clone()
    }

    /// Sets current [`MediaExchangeState`] to
    /// [`MediaExchangeState::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::SenderIsRequired`] is returned if [`Sender`] is
    /// required for the call and can't be disabled.
    fn media_exchange_state_transition_to(
        &self,
        desired_state: TrackMediaState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            // TODO(evdokimovs): is_required важен для mute_state??
            match desired_state {
                TrackMediaState::MediaExchange(desired_state) => {
                    self.media_exchange_state_controller()
                        .transition_to(desired_state);
                }
                TrackMediaState::Mute(desired_state) => {
                    self.mute_state_controller().transition_to(desired_state);
                }
            }
            Ok(())
        }
    }
}
