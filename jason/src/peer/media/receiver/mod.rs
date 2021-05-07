//! Implementation of the `MediaTrack` with a `Recv` direction.

mod component;

use std::cell::{Cell, RefCell};

use futures::channel::mpsc;
use medea_client_api_proto::{self as proto, MediaType, MemberId};
use proto::TrackId;

use crate::{
    media::{track::remote, MediaKind, RecvConstraints, TrackConstraints},
    peer::{
        media::media_exchange_state, MediaConnections, MediaStateControllable,
        PeerEvent, TrackEvent,
    },
    platform,
};

use super::TransceiverSide;

#[doc(inline)]
pub use self::component::{Component, State};

/// Representation of a remote [`remote::Track`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`platform::Transceiver`] and the actual
/// [`remote::Track`] only when [`remote::Track`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    caps: TrackConstraints,
    sender_id: MemberId,
    transceiver: RefCell<Option<platform::Transceiver>>,
    mid: RefCell<Option<String>>,
    track: RefCell<Option<remote::Track>>,
    is_track_notified: Cell<bool>,
    enabled_general: Cell<bool>,
    enabled_individual: Cell<bool>,
    muted: Cell<bool>,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    track_events_sender: mpsc::UnboundedSender<TrackEvent>,
}

impl Receiver {
    /// Creates a new [`platform::Transceiver`] if provided `mid` is [`None`],
    /// otherwise creates a [`Receiver`] without a [`platform::Transceiver`]. It
    /// will be injected when a [`remote::Track`] will arrive.
    ///
    /// Created [`platform::Transceiver`] direction is set to
    /// [`TransceiverDirection::INACTIVE`][1] if `enabled_individual` is
    /// `false`.
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`remote::Track`] data
    /// arrives.
    ///
    /// [1]: platform::TransceiverDirection::INACTIVE
    pub fn new(
        state: &State,
        media_connections: &MediaConnections,
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let connections = media_connections.0.borrow();
        let caps = TrackConstraints::from(state.media_type().clone());
        let kind = MediaKind::from(&caps);
        let transceiver_direction = if state.enabled_individual() {
            platform::TransceiverDirection::RECV
        } else {
            platform::TransceiverDirection::INACTIVE
        };

        let transceiver = if state.mid().is_none() {
            // Try to find send transceiver that can be used as sendrecv.
            let mut senders = connections.senders.values();
            let sender = senders.find(|sndr| {
                sndr.caps().media_kind() == caps.media_kind()
                    && sndr.caps().media_source_kind()
                        == caps.media_source_kind()
            });
            Some(sender.map_or_else(
                || connections.add_transceiver(kind, transceiver_direction),
                |sender| {
                    let trnsvr = sender.transceiver();
                    trnsvr.add_direction(transceiver_direction);

                    trnsvr
                },
            ))
        } else {
            None
        };

        let this = Self {
            track_id: state.track_id(),
            caps,
            sender_id: state.sender_id().clone(),
            transceiver: RefCell::new(transceiver),
            mid: RefCell::new(state.mid().map(ToString::to_string)),
            track: RefCell::new(None),
            is_track_notified: Cell::new(false),
            peer_events_sender: connections.peer_events_sender.clone(),
            enabled_general: Cell::new(state.enabled_individual()),
            enabled_individual: Cell::new(state.enabled_general()),
            muted: Cell::new(state.muted()),
            track_events_sender,
        };

        let enabled_in_cons = match &state.media_type() {
            MediaType::Audio(_) => recv_constraints.is_audio_enabled(),
            MediaType::Video(_) => recv_constraints.is_video_enabled(),
        };
        if !enabled_in_cons {
            state
                .media_exchange_state_controller()
                .transition_to(enabled_in_cons.into());
        }

        this
    }

    /// Returns [`TrackConstraints`] of this [`Receiver`].
    #[inline]
    #[must_use]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns [`mid`] of this [`Receiver`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    #[must_use]
    pub fn mid(&self) -> Option<String> {
        if self.mid.borrow().is_none() && self.transceiver.borrow().is_some() {
            if let Some(transceiver) =
                self.transceiver.borrow().as_ref().cloned()
            {
                self.mid.replace(Some(transceiver.mid()?));
            }
        }
        self.mid.borrow().clone()
    }

    /// Indicates whether this [`Receiver`] receives media data.
    #[must_use]
    pub fn is_receiving(&self) -> bool {
        let is_recv_direction =
            self.transceiver.borrow().as_ref().map_or(false, |trnsvr| {
                trnsvr.has_direction(platform::TransceiverDirection::RECV)
            });

        self.enabled_individual.get() && is_recv_direction
    }

    /// Sends [`TrackEvent::MediaExchangeIntention`] with the provided
    /// [`media_exchange_state`].
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

    /// Adds the provided [`platform::MediaStreamTrack`] and
    /// [`platform::Transceiver`] to this [`Receiver`].
    ///
    /// Sets [`platform::MediaStreamTrack::enabled`] same as
    /// [`Receiver::enabled_individual`] of this [`Receiver`].
    pub fn set_remote_track(
        &self,
        transceiver: platform::Transceiver,
        new_track: platform::MediaStreamTrack,
    ) {
        if let Some(old_track) = self.track.borrow().as_ref() {
            if old_track.id() == new_track.id() {
                return;
            }
        }

        let new_track = remote::Track::new(
            new_track,
            self.caps.media_source_kind(),
            self.enabled_individual.get(),
            self.muted.get(),
        );

        if self.enabled_individual.get() {
            transceiver.add_direction(platform::TransceiverDirection::RECV);
        } else {
            transceiver.sub_direction(platform::TransceiverDirection::RECV);
        }

        self.transceiver.replace(Some(transceiver));
        if let Some(prev_track) = self.track.replace(Some(new_track)) {
            prev_track.stop();
        };
        self.maybe_notify_track();
    }

    /// Replaces [`Receiver`]'s [`platform::Transceiver`] with the provided
    /// [`platform::Transceiver`].
    ///
    /// Doesn't update [`platform::TransceiverDirection`] of the
    /// [`platform::Transceiver`].
    ///
    /// No-op if provided with the same [`platform::Transceiver`] as already
    /// exists in this [`Receiver`].
    pub fn replace_transceiver(&self, transceiver: platform::Transceiver) {
        if self.mid.borrow().as_ref() == transceiver.mid().as_ref() {
            self.transceiver.replace(Some(transceiver));
        }
    }

    /// Returns a [`platform::Transceiver`] of this [`Receiver`].
    ///
    /// Returns [`None`] if this [`Receiver`] doesn't have a
    /// [`platform::Transceiver`].
    #[inline]
    pub fn transceiver(&self) -> Option<platform::Transceiver> {
        self.transceiver.borrow().clone()
    }

    /// Emits [`PeerEvent::NewRemoteTrack`] if [`Receiver`] is receiving media
    /// and has not notified yet.
    fn maybe_notify_track(&self) {
        if self.is_track_notified.get() {
            return;
        }
        if !self.is_receiving() {
            return;
        }
        if let Some(track) = self.track.borrow().as_ref() {
            drop(self.peer_events_sender.unbounded_send(
                PeerEvent::NewRemoteTrack {
                    sender_id: self.sender_id.clone(),
                    track: track.clone(),
                },
            ));
            self.is_track_notified.set(true);
        }
    }
}

#[cfg(feature = "mockable")]
impl Receiver {
    /// Returns current `enabled_general` status of the [`Receiver`].
    #[inline]
    #[must_use]
    pub fn enabled_general(&self) -> bool {
        self.enabled_general.get()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if let Some(transceiver) = self.transceiver.borrow().as_ref() {
            if !transceiver.is_stopped() {
                transceiver.sub_direction(platform::TransceiverDirection::RECV);
            }
        }
        if let Some(recv_track) = self.track.borrow_mut().take() {
            recv_track.stop();
        }
    }
}
