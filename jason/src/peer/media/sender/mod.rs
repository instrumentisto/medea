//! Implementation of the `MediaTrack` with a `Send` direction.

mod component;

use std::{cell::Cell, rc::Rc};

use derive_more::{Display, From};
use futures::channel::mpsc;
use medea_client_api_proto::TrackId;
use tracerr::Traced;

use crate::{
    media::{
        track::local, LocalTracksConstraints, MediaKind, TrackConstraints,
    },
    peer::TrackEvent,
    platform,
    utils::JsCaused,
};

use super::{
    media_exchange_state, mute_state, MediaConnections, MediaStateControllable,
};

#[doc(inline)]
pub use self::component::{Component, State};

/// Errors occurring when creating a new [`Sender`].
#[derive(Clone, Debug, Display, JsCaused)]
#[js(error = "platform::Error")]
pub enum CreateError {
    /// [`Sender`] cannot be disabled because it's marked as `required`.
    #[display(fmt = "MediaExchangeState of Sender cannot transit to \
                     disabled state, because this Sender is required.")]
    CannotDisableRequiredSender,

    /// Could not find a [`platform::Transceiver`] by `mid`.
    #[display(fmt = "Unable to find Transceiver with mid: {}", _0)]
    TransceiverNotFound(String),
}

/// Error occuring in [`RTCRtpSender.replaceTrack()`][1] method.
///
/// [1]: https://w3.org/TR/webrtc#dom-rtcrtpsender-replacetrack
#[derive(Clone, Debug, Display, From, JsCaused)]
#[js(error = "platform::Error")]
#[display(fmt = "MediaManagerHandle is in detached state")]
pub struct InsertTrackError(platform::Error);

/// Representation of a [`local::Track`] that is being sent to some remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    transceiver: platform::Transceiver,
    muted: Cell<bool>,
    enabled_individual: Cell<bool>,
    enabled_general: Cell<bool>,
    send_constraints: LocalTracksConstraints,
    track_events_sender: mpsc::UnboundedSender<TrackEvent>,
}

impl Sender {
    /// Creates a new [`platform::Transceiver`] if the provided `mid` is
    /// [`None`], otherwise retrieves an existing [`platform::Transceiver`] via
    /// the provided `mid` from the provided [`MediaConnections`].
    ///
    /// # Errors
    ///
    /// With a [`CreateError::TransceiverNotFound`] if [`State`] has [`Some`]
    /// [`mid`], but this [`mid`] isn't found in the [`MediaConnections`].
    ///
    /// With a [`CreateError::CannotDisableRequiredSender`] if the provided
    /// [`LocalTracksConstraints`] are configured to disable this [`Sender`],
    /// but it cannot be disabled according to the provide [`State`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    pub fn new(
        state: &State,
        media_connections: &MediaConnections,
        send_constraints: LocalTracksConstraints,
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
    ) -> Result<Rc<Self>, Traced<CreateError>> {
        let enabled_in_cons = send_constraints.enabled(state.media_type());
        let muted_in_cons = send_constraints.muted(state.media_type());
        let media_disabled = state.is_muted()
            || !state.is_enabled_individual()
            || !enabled_in_cons
            || muted_in_cons;
        if state.media_type().required() && media_disabled {
            return Err(tracerr::new!(
                CreateError::CannotDisableRequiredSender
            ));
        }

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
                    connections.add_transceiver(
                        kind,
                        platform::TransceiverDirection::INACTIVE,
                    )
                }),
            Some(mid) => connections
                .get_transceiver_by_mid(mid)
                .ok_or_else(|| {
                    CreateError::TransceiverNotFound(mid.to_string())
                })
                .map_err(tracerr::wrap!())?,
        };

        let this = Rc::new(Sender {
            track_id: state.id(),
            caps,
            transceiver,
            enabled_general: Cell::new(state.is_enabled_general()),
            enabled_individual: Cell::new(state.is_enabled_individual()),
            muted: Cell::new(state.is_muted()),
            track_events_sender,
            send_constraints,
        });

        if !enabled_in_cons {
            state.media_exchange_state_controller().transition_to(
                media_exchange_state::Stable::from(enabled_in_cons),
            );
        }
        if muted_in_cons {
            state
                .mute_state_controller()
                .transition_to(mute_state::Stable::from(muted_in_cons));
        }

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
        self.transceiver
            .has_direction(platform::TransceiverDirection::SEND)
    }

    /// Drops [`local::Track`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to [`None`].
    ///
    /// # Panics
    ///
    /// If [replaceTrack()][2] call fails. This might happen if an underlying
    /// [RTCRtpSender][1] is stopped. [replaceTrack()][2] with `null` track
    /// should never fail for any other reason.
    ///
    /// [1]: https://w3c.github.io/webrtc-pc/#dom-rtcrtpsender
    /// [2]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    #[inline]
    pub async fn remove_track(&self) {
        self.transceiver.drop_send_track().await;
    }

    /// Indicates whether this [`Sender`] has [`local::Track`].
    #[inline]
    #[must_use]
    pub fn has_track(&self) -> bool {
        self.transceiver.has_send_track()
    }

    /// Inserts provided [`local::Track`] into provided [`Sender`]s
    /// transceiver. No-op if provided track already being used by this
    /// [`Sender`].
    pub(super) async fn insert_track(
        self: Rc<Self>,
        new_track: Rc<local::Track>,
    ) -> Result<(), Traced<InsertTrackError>> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.transceiver.send_track() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        let new_track = new_track.fork();

        new_track.set_enabled(!self.muted.get());

        self.transceiver
            .set_send_track(Rc::new(new_track))
            .await
            .map_err(InsertTrackError::from)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Returns [`platform::Transceiver`] of this [`Sender`].
    #[inline]
    #[must_use]
    pub fn transceiver(&self) -> platform::Transceiver {
        self.transceiver.clone()
    }

    /// Returns [`mid`] of this [`Sender`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    #[inline]
    #[must_use]
    pub fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }

    /// Indicates whether this [`Sender`] is enabled in
    /// [`LocalTracksConstraints`].
    fn enabled_in_cons(&self) -> bool {
        self.send_constraints.is_track_enabled(
            self.caps.media_kind(),
            self.caps.media_source_kind(),
        )
    }

    /// Sends [`TrackEvent::MediaExchangeIntention`] with the provided
    /// [`media_exchange_state`].
    #[inline]
    pub fn send_media_exchange_state_intention(
        &self,
        state: media_exchange_state::Transition,
    ) {
        let _ = self.track_events_sender.unbounded_send(
            TrackEvent::MediaExchangeIntention {
                id: self.track_id,
                enabled: matches!(
                    state,
                    media_exchange_state::Transition::Enabling(_)
                ),
            },
        );
    }

    /// Sends [`TrackEvent::MuteUpdateIntention`] with the provided
    /// [`mute_state`].
    #[inline]
    pub fn send_mute_state_intention(&self, state: mute_state::Transition) {
        let _ = self.track_events_sender.unbounded_send(
            TrackEvent::MuteUpdateIntention {
                id: self.track_id,
                muted: matches!(state, mute_state::Transition::Muting(_)),
            },
        );
    }
}

#[cfg(feature = "mockable")]
impl Sender {
    /// Indicates whether general media exchange state of this [`Sender`] is in
    /// [`StableMediaExchangeState::Disabled`].
    #[inline]
    #[must_use]
    pub fn general_disabled(&self) -> bool {
        !self.enabled_general.get()
    }

    /// Indicates whether this [`Sender`] is disabled.
    #[inline]
    #[must_use]
    pub fn disabled(&self) -> bool {
        !self.enabled_individual.get()
    }

    /// Indicates whether this [`Sender`] is muted.
    #[inline]
    #[must_use]
    pub fn muted(&self) -> bool {
        self.muted.get()
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        if !self.transceiver.is_stopped() {
            self.transceiver
                .sub_direction(platform::TransceiverDirection::SEND);
            platform::spawn(self.transceiver.drop_send_track());
        }
    }
}
