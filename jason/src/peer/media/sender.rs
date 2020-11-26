//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use medea_client_api_proto::{MediaSourceKind, TrackId, TrackPatchEvent};

use crate::{
    media::{
        track::local, LocalTracksConstraints, MediaKind, TrackConstraints,
        VideoSource,
    },
    peer::transceiver::{Transceiver, TransceiverDirection},
};

use super::{
    media_exchange_state, mute_state,
    transitable_state::{
        MediaExchangeState, MediaExchangeStateController, MediaState,
        MuteStateController,
    },
    MediaConnections, MediaConnectionsError, MediaStateControllable, Result,
    TransceiverSide,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub media_connections: &'a MediaConnections,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub mid: Option<String>,
    pub media_exchange_state: media_exchange_state::Stable,
    pub mute_state: mute_state::Stable,
    pub required: bool,
    pub send_constraints: LocalTracksConstraints,
}

impl<'a> SenderBuilder<'a> {
    /// Builds new [`Transceiver`] if provided `mid` is [`None`], otherwise
    /// retrieves existing [`Transceiver`] via provided `mid` from a
    /// provided [`MediaConnections`]. Errors if [`Transceiver`] lookup
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

        let media_exchange_state =
            MediaExchangeStateController::new(self.media_exchange_state);
        let this = Rc::new(Sender {
            track_id: self.track_id,
            caps: self.caps,
            general_media_exchange_state: Cell::new(self.media_exchange_state),
            mute_state: MuteStateController::new(self.mute_state),
            transceiver,
            media_exchange_state,
            required: self.required,
            send_constraints: self.send_constraints,
        });

        Ok(this)
    }
}

/// Representation of a [`local::Track`] that is being sent to some
/// remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    transceiver: Transceiver,
    media_exchange_state: Rc<MediaExchangeStateController>,
    mute_state: Rc<MuteStateController>,
    general_media_exchange_state: Cell<media_exchange_state::Stable>,
    required: bool,
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

    /// Updates [`Sender`]s general media exchange state based on the provided
    /// [`media_exchange_state::Stable`].
    ///
    /// Sets [`Sender`]s underlying transceiver direction to
    /// [`TransceiverDirection::INACTIVE`] if provided media exchange state is
    /// [`media_exchange_state::Stable::Disabled`].
    fn update_general_media_exchange_state(
        &self,
        new_state: media_exchange_state::Stable,
    ) {
        if self.general_media_exchange_state.get() != new_state {
            self.general_media_exchange_state.set(new_state);
            match new_state {
                media_exchange_state::Stable::Enabled => {
                    if self.enabled_in_cons() {
                        self.transceiver
                            .add_direction(TransceiverDirection::SEND);
                    }
                }
                media_exchange_state::Stable::Disabled => {
                    self.transceiver.sub_direction(TransceiverDirection::SEND);
                }
            }
        }
    }

    /// Inserts provided [`local::Track`] into provided [`Sender`]s
    /// transceiver. No-op if provided track already being used by this
    /// [`Sender`].
    pub(super) async fn insert_track(
        self: Rc<Self>,
        new_track: Rc<local::Track>,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.transceiver.send_track() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        let new_track = new_track.fork();

        new_track.set_enabled(
            self.mute_state.state().cancel_transition()
                == mute_state::Stable::Unmuted.into(),
        );

        self.transceiver
            .set_send_track(Some(Rc::new(new_track)))
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertLocalTrack)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Indicates whether this [`Sender`] is enabled in
    /// [`LocalTracksConstraints`].
    fn enabled_in_cons(&self) -> bool {
        self.send_constraints.is_track_enabled(
            self.caps.media_kind(),
            self.caps.media_source_kind(),
        )
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`TrackPatchEvent`].
    ///
    /// Returns `true` if media stream update should be performed for this
    /// [`Sender`].
    pub async fn update(&self, track: &TrackPatchEvent) -> bool {
        if track.id != self.track_id {
            return false;
        }

        if let Some(muted) = track.muted {
            self.mute_state.update(muted.into());
            match mute_state::Stable::from(muted) {
                mute_state::Stable::Unmuted => {
                    self.transceiver.set_send_track_enabled(true);
                }
                mute_state::Stable::Muted => {
                    self.transceiver.set_send_track_enabled(false);
                }
            }
        }
        let mut requires_media_update = false;
        if let Some(enabled) = track.enabled_individual {
            let mute_state_before = self.media_exchange_state.state();
            self.media_exchange_state.update(enabled.into());
            if let (
                MediaExchangeState::Stable(before),
                MediaExchangeState::Stable(after),
            ) = (mute_state_before, self.media_exchange_state.state())
            {
                requires_media_update = before != after
                    && after == media_exchange_state::Stable::Enabled;
            }

            if !enabled {
                self.remove_track().await;
            }
        }
        if let Some(enabled) = track.enabled_individual {
            self.media_exchange_state.update(enabled.into());
        }
        if let Some(enabled) = track.enabled_general {
            self.update_general_media_exchange_state(enabled.into());
        }

        requires_media_update
    }

    /// Changes underlying transceiver direction to
    /// [`TransceiverDirection::SEND`] if this [`Sender`]s general media
    /// exchange state is [`media_exchange_state::Stable::Enabled`].
    pub fn maybe_enable(&self) {
        if self.is_general_enabled()
            && !self.transceiver.has_direction(TransceiverDirection::SEND)
            && self.enabled_in_cons()
        {
            self.transceiver.add_direction(TransceiverDirection::SEND);
        }
    }

    /// Returns [`Transceiver`] of this [`Sender`].
    pub fn transceiver(&self) -> Transceiver {
        self.transceiver.clone()
    }

    /// Checks whether general media exchange state of the [`Sender`] is in
    /// [`media_exchange_state::Stable::Enabled`].
    fn is_general_enabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Enabled
    }

    /// Drops [`local::Track`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to `None`.
    async fn remove_track(&self) {
        // cannot fail
        self.transceiver.set_send_track(None).await.unwrap();
    }
}

#[cfg(feature = "mockable")]
impl Sender {
    /// Indicates whether general media exchange state of this [`Sender`] is in
    /// [`StableMediaExchangeState::Disabled`].
    #[inline]
    #[must_use]
    pub fn general_disabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Disabled
    }

    /// Indicates whether this [`Sender`] is disabled.
    #[inline]
    #[must_use]
    pub fn disabled(&self) -> bool {
        self.media_exchange_state.disabled()
    }

    /// Indicates whether this [`Sender`] is muted.
    #[inline]
    #[must_use]
    pub fn muted(&self) -> bool {
        self.mute_state.muted()
    }

    /// Indicates whether this [`Sender`] is enabled.
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.media_exchange_state.enabled()
    }
}

impl TransceiverSide for Sender {
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    fn kind(&self) -> MediaKind {
        MediaKind::from(&self.caps)
    }

    fn source_kind(&self) -> MediaSourceKind {
        self.caps.media_source_kind()
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

impl MediaStateControllable for Sender {
    #[inline]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        Rc::clone(&self.media_exchange_state)
    }

    #[inline]
    fn mute_state_controller(&self) -> Rc<MuteStateController> {
        Rc::clone(&self.mute_state)
    }

    /// Sets current [`MediaExchangeState`] to
    /// [`media_exchange_state::Transition`].
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::CannotDisableRequiredSender`] is returned if
    /// [`Sender`] is required for the call and can't be disabled.
    fn media_state_transition_to(
        &self,
        desired_state: MediaState,
    ) -> Result<()> {
        if self.required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            match desired_state {
                MediaState::MediaExchange(desired_state) => {
                    self.media_exchange_state_controller()
                        .transition_to(desired_state);
                }
                MediaState::Mute(desired_state) => {
                    self.mute_state_controller().transition_to(desired_state);
                }
            }
            Ok(())
        }
    }
}
