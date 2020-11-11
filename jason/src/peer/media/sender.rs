//! Implementation of the `MediaTrack` with a `Send` direction.

use std::{cell::Cell, rc::Rc};

use medea_client_api_proto::{MediaSourceKind, TrackId, TrackPatchEvent};

use crate::{
    media::{
        LocalTracksConstraints, MediaKind, MediaStreamTrack, TrackConstraints,
        VideoSource,
    },
    peer::{
        media::TransceiverSide,
        transceiver::{Transceiver, TransceiverDirection},
        MediaExchangeState,
    },
};

use super::{
    media_exchange_state::{
        MediaExchangeStateController, StableMediaExchangeState,
    },
    Disableable, MediaConnections, MediaConnectionsError, Result,
};

/// Builder of the [`Sender`].
pub struct SenderBuilder<'a> {
    pub media_connections: &'a MediaConnections,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub mid: Option<String>,
    pub media_exchange_state: StableMediaExchangeState,
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
            MediaExchangeStateController::new(self.media_exchange_state);
        let this = Rc::new(Sender {
            track_id: self.track_id,
            caps: self.caps,
            general_media_exchange_state: Cell::new(self.media_exchange_state),
            transceiver,
            media_exchange_state: media_exchange_state_controller,
            is_required: self.is_required,
            send_constraints: self.send_constraints,
        });

        Ok(this)
    }
}

/// Representation of a local [`MediaStreamTrack`] that is being sent to some
/// remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    transceiver: Transceiver,
    media_exchange_state: Rc<MediaExchangeStateController>,
    general_media_exchange_state: Cell<StableMediaExchangeState>,
    is_required: bool,
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
    ///
    /// Returns `true` if media stream update should be performed for this
    /// [`Sender`].
    pub async fn update(&self, track: &TrackPatchEvent) -> bool {
        if track.id != self.track_id {
            return false;
        }

        let mut requires_media_update = false;
        if let Some(is_enabled) = track.is_enabled_individual {
            let state_before = self.media_exchange_state.media_exchange_state();
            self.media_exchange_state.update(is_enabled);
            if let (
                MediaExchangeState::Stable(before),
                MediaExchangeState::Stable(after),
            ) = (
                state_before,
                self.media_exchange_state.media_exchange_state(),
            ) {
                requires_media_update = before != after
                    && after == StableMediaExchangeState::Enabled;
            }

            if !is_enabled {
                self.remove_track().await;
            }
        }
        if let Some(is_enabled_general) = track.is_enabled_general {
            self.update_general_media_exchange_state(is_enabled_general.into());
        }

        requires_media_update
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

impl Disableable for Sender {
    #[inline]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        self.media_exchange_state.clone()
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
        desired_state: StableMediaExchangeState,
    ) -> Result<()> {
        if self.is_required {
            Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
            ))
        } else {
            self.media_exchange_state.transition_to(desired_state);
            Ok(())
        }
    }
}
