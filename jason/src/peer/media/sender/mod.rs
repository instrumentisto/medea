//! Implementation of the `MediaTrack` with a `Send` direction.

mod component;

use std::{cell::Cell, rc::Rc};

use crate::peer::{PeerEvent, TrackEvent};
use futures::channel::mpsc;
use medea_client_api_proto::{MediaSourceKind, TrackId, TrackPatchCommand};

use crate::{
    media::{
        track::local, LocalTracksConstraints, MediaKind, TrackConstraints,
        VideoSource,
    },
    peer::{
        transceiver::{Transceiver, TransceiverDirection},
        MediaExchangeState, MuteState,
    },
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

/// Builder of the [`Sender`].
pub(super) struct Builder<'a> {
    pub media_connections: &'a MediaConnections,
    pub track_id: TrackId,
    pub caps: TrackConstraints,
    pub mid: Option<String>,
    pub media_exchange_state: media_exchange_state::Stable,
    pub mute_state: mute_state::Stable,
    pub required: bool,
    pub send_constraints: LocalTracksConstraints,
    pub track_events_sender: mpsc::UnboundedSender<TrackEvent>,
}

impl<'a> Builder<'a> {
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
            transceiver,
            required: self.required,
            send_constraints: self.send_constraints,
            enabled_general: Cell::new(
                self.media_exchange_state
                    == media_exchange_state::Stable::Enabled,
            ),
            enabled_individual: Cell::new(
                self.media_exchange_state
                    == media_exchange_state::Stable::Enabled,
            ),
            muted: Cell::new(self.mute_state == mute_state::Stable::Muted),
            track_events_sender: self.track_events_sender,
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
    required: bool,
    muted: Cell<bool>,
    enabled_individual: Cell<bool>,
    enabled_general: Cell<bool>,
    send_constraints: LocalTracksConstraints,
    track_events_sender: mpsc::UnboundedSender<TrackEvent>,
}

impl Sender {
    pub fn set_enabled_general(&self, enabled_general: bool) {
        self.enabled_general.set(enabled_general);
    }

    pub fn set_enabled_individual(&self, enable_individual: bool) {
        self.enabled_individual.set(enable_individual);
    }

    pub fn set_muted(&self, muted: bool) {
        self.muted.set(muted);
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

    /// Indicates whether this [`Sender`] is enabled in
    /// [`LocalTracksConstraints`].
    fn enabled_in_cons(&self) -> bool {
        self.send_constraints.is_track_enabled(
            self.caps.media_kind(),
            self.caps.media_source_kind(),
        )
    }

    /// Changes underlying transceiver direction to
    /// [`TransceiverDirection::SEND`] if this [`Sender`]s general media
    /// exchange state is [`media_exchange_state::Stable::Enabled`].
    pub fn maybe_enable(&self) {
        if self.enabled_general.get()
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

    pub fn mid(&self) -> Option<String> {
        self.transceiver.mid()
    }

    pub fn send_media_exchange_state_intention(
        &self,
        state: media_exchange_state::Transition,
    ) {
        match state {
            media_exchange_state::Transition::Enabling(_) => {
                self.track_events_sender.unbounded_send(
                    TrackEvent::MediaExchangeIntention {
                        id: self.track_id,
                        enabled: true,
                    },
                );
            }
            media_exchange_state::Transition::Disabling(_) => {
                self.track_events_sender.unbounded_send(
                    TrackEvent::MediaExchangeIntention {
                        id: self.track_id,
                        enabled: false,
                    },
                );
            }
        }
    }

    pub fn send_mute_state_intention(&self, state: mute_state::Transition) {
        match state {
            mute_state::Transition::Muting(_) => {
                self.track_events_sender.unbounded_send(
                    TrackEvent::MuteUpdateIntention {
                        id: self.track_id,
                        muted: true,
                    },
                );
            }
            mute_state::Transition::Unmuting(_) => {
                self.track_events_sender.unbounded_send(
                    TrackEvent::MuteUpdateIntention {
                        id: self.track_id,
                        muted: false,
                    },
                );
            }
        }
    }
}

#[cfg(feature = "mockable")]
impl Sender {
    /// Indicates whether general media exchange state of this [`Sender`] is in
    /// [`StableMediaExchangeState::Disabled`].
    #[inline]
    #[must_use]
    pub fn general_disabled(&self) -> bool {
        // self.general_media_exchange_state.get()
        //     == media_exchange_state::Stable::Disabled
        todo!()
    }

    /// Indicates whether this [`Sender`] is disabled.
    #[inline]
    #[must_use]
    pub fn disabled(&self) -> bool {
        // self.media_exchange_state.disabled()
        todo!()
    }

    /// Indicates whether this [`Sender`] is muted.
    #[inline]
    #[must_use]
    pub fn muted(&self) -> bool {
        // self.mute_state.muted()
        todo!()
    }
}
