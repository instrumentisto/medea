//! Implementation of the `MediaTrack` with a `Send` direction.

mod component;

use std::{cell::Cell, rc::Rc};

use medea_client_api_proto::{MediaSourceKind, TrackId};

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
        MediaExchangeStateController, MediaState, MuteStateController,
    },
    MediaConnections, MediaConnectionsError, MediaStateControllable, Result,
    TransceiverSide,
};

pub use self::component::{Component, State};

/// Representation of a [`local::Track`] that is being sent to some remote peer.
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
    /// Creates new [`Transceiver`] if provided `mid` is [`None`], otherwise
    /// retrieves existing [`Transceiver`] via provided `mid` from a
    /// provided [`MediaConnections`]. Errors if [`Transceiver`] lookup
    /// fails.
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::TransceiverNotFound`] if [`State`]
    /// has [`Some`] [`mid`], but this [`mid`] isn't found in the
    /// [`MediaConnections`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    pub fn new(
        state: &State,
        media_connections: &MediaConnections,
        send_constraints: LocalTracksConstraints,
    ) -> Result<Rc<Self>> {
        let connections = media_connections.0.borrow();
        let caps = TrackConstraints::from(state.media_type().clone());
        let kind = MediaKind::from(&caps);
        let transceiver = match state.mid() {
            // Try to find rcvr transceiver that can be used as sendrecv.
            None => connections
                .receivers
                .values()
                .find(|rcvr| {
                    rcvr.caps().media_kind() == caps.media_kind()
                        && rcvr.caps().media_source_kind()
                            == caps.media_source_kind()
                })
                .and_then(|rcvr| rcvr.transceiver())
                .unwrap_or_else(|| {
                    connections
                        .add_transceiver(kind, TransceiverDirection::INACTIVE)
                }),
            Some(mid) => connections
                .get_transceiver_by_mid(mid)
                .ok_or_else(|| {
                    MediaConnectionsError::TransceiverNotFound(mid.to_owned())
                })
                .map_err(tracerr::wrap!())?,
        };
        let media_exchange_state =
            media_exchange_state::Stable::from(!state.is_enabled_individual());
        let mute_state = mute_state::Stable::from(state.is_muted());

        let this = Rc::new(Sender {
            track_id: state.id(),
            caps,
            general_media_exchange_state: Cell::new(media_exchange_state),
            mute_state: MuteStateController::new(mute_state),
            transceiver,
            media_exchange_state: MediaExchangeStateController::new(
                media_exchange_state,
            ),
            required: state.media_type().required(),
            send_constraints,
        });

        Ok(this)
    }

    /// Returns [`TrackConstraints`] of this [`Sender`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Indicates whether this [`Sender`] is publishing media traffic.
    #[inline]
    #[must_use]
    pub fn is_publishing(&self) -> bool {
        self.transceiver.has_direction(TransceiverDirection::SEND)
    }

    /// Drops [`local::Track`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to [`None`].
    #[inline]
    pub async fn remove_track(&self) {
        // cannot fail
        self.transceiver.set_send_track(None).await.unwrap();
    }

    /// Indicates whether this [`Sender`] has [`local::Track`].
    #[inline]
    #[must_use]
    pub fn has_track(&self) -> bool {
        self.transceiver.has_send_track()
    }

    /// Indicates whether this [`Sender`] is enabled.
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.media_exchange_state.enabled()
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

    /// Updates general [`media_exchange_state`] of this [`Sender`] by the
    /// provided [`bool`].
    #[inline]
    pub fn set_enabled_general(&self, enabled: bool) {
        self.update_general_media_exchange_state(enabled.into());
    }

    /// Updates individual [`media_exchange_state`] of this [`Sender`] by the
    /// provided [`bool`].
    #[inline]
    pub fn set_enabled_individual(&self, enabled: bool) {
        self.media_exchange_state.update(enabled.into());
    }

    /// Updates [`mute_state`] of this [`Sender`] with a provided [`bool`].
    ///
    /// Calls [`Transceiver::set_send_track_enabled()`] with the provided
    /// [`bool`] value being inverted.
    #[inline]
    pub fn set_muted(&self, muted: bool) {
        self.mute_state.update(muted.into());
        self.transceiver.set_send_track_enabled(!muted);
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
    #[inline]
    #[must_use]
    pub fn transceiver(&self) -> Transceiver {
        self.transceiver.clone()
    }

    /// Checks whether general media exchange state of the [`Sender`] is in
    /// [`media_exchange_state::Stable::Enabled`].
    fn is_general_enabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Enabled
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

    /// Sets current [`MediaState`] to the transition.
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
