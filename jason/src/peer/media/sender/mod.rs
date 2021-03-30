//! Implementation of the `MediaTrack` with a `Send` direction.

mod component;

use std::{cell::Cell, rc::Rc};

use futures::channel::mpsc;
use medea_client_api_proto::TrackId;

use crate::{
    media::{
        track::local, LocalTracksConstraints, MediaKind, TrackConstraints,
    },
    peer::{
        transceiver::{Transceiver, TransceiverDirection},
        TrackEvent,
    },
};

use super::{
    media_exchange_state, mute_state, MediaConnections, MediaConnectionsError,
    MediaStateControllable, Result,
};

pub use self::component::{Component, State};

/// Representation of a [`local::Track`] that is being sent to some remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    transceiver: Transceiver,
    muted: Cell<bool>,
    enabled_individual: Cell<bool>,
    enabled_general: Cell<bool>,
    send_constraints: LocalTracksConstraints,
    track_events_sender: mpsc::UnboundedSender<TrackEvent>,
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
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
    ) -> Result<Rc<Self>> {
        let enabled_in_cons = send_constraints.enabled(state.media_type());
        let muted_in_cons = send_constraints.muted(state.media_type());
        let media_disabled = state.is_muted()
            || !state.is_enabled_individual()
            || !enabled_in_cons
            || muted_in_cons;
        if state.media_type().required() && media_disabled {
            return Err(tracerr::new!(
                MediaConnectionsError::CannotDisableRequiredSender
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
                    connections
                        .add_transceiver(kind, TransceiverDirection::INACTIVE)
                }),
            Some(mid) => connections
                .get_transceiver_by_mid(mid)
                .ok_or_else(|| {
                    MediaConnectionsError::TransceiverNotFound(mid.to_string())
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
        self.transceiver.has_direction(TransceiverDirection::SEND)
    }

    /// Drops [`local::Track`] used by this [`Sender`]. Sets track used by
    /// sending side of inner transceiver to [`None`].
    #[allow(clippy::missing_panics_doc)]
    #[inline]
    pub async fn remove_track(&self) {
        // cannot fail TODO: why? describe it properly
        self.transceiver.set_send_track(None).await.unwrap();
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
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = self.transceiver.send_track() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        let new_track = new_track.fork();

        new_track.set_enabled(!self.muted.get());

        self.transceiver
            .set_send_track(Some(Rc::new(new_track)))
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertLocalTrack)
            .map_err(tracerr::wrap!())?;

        Ok(())
    }

    /// Returns [`Transceiver`] of this [`Sender`].
    #[inline]
    #[must_use]
    pub fn transceiver(&self) -> Transceiver {
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
