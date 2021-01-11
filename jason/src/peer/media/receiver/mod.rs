//! Implementation of the `MediaTrack` with a `Recv` direction.

mod component;

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::channel::mpsc;
use medea_client_api_proto as proto;
use medea_client_api_proto::{MediaSourceKind, MemberId, TrackPatchCommand};
use proto::TrackId;
use web_sys as sys;

use crate::{
    media::{track::remote, MediaKind, TrackConstraints},
    peer::{
        transceiver::Transceiver, MediaConnections, MediaExchangeState,
        MediaStateControllable, PeerEvent, TransceiverDirection,
    },
};

use super::{
    transitable_state::{
        media_exchange_state, MediaExchangeStateController,
        MuteStateController, TransitableStateController,
    },
    TransceiverSide,
};

pub use self::component::{Component, State};

/// Representation of a remote [`remote::Track`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`Transceiver`] and the actual
/// [`remote::Track`] only when [`remote::Track`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    caps: TrackConstraints,
    sender_id: MemberId,
    transceiver: RefCell<Option<Transceiver>>,
    mid: RefCell<Option<String>>,
    track: RefCell<Option<remote::Track>>,
    // general_media_exchange_state: Cell<media_exchange_state::Stable>,
    is_track_notified: Cell<bool>,
    // media_exchange_state_controller: Rc<MediaExchangeStateController>,
    enabled_general: Cell<bool>,
    enabled_individual: Cell<bool>,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    track_events_sender: mpsc::UnboundedSender<TrackEvent>
}
use crate::peer::media::TrackEvent;

impl Receiver {
    /// Creates new [`Transceiver`] if provided `mid` is `None`, otherwise
    /// creates [`Receiver`] without [`Transceiver`]. It will be injected
    /// when [`remote::Track`] arrives.
    ///
    /// Created [`Transceiver`] direction is set to
    /// [`TransceiverDirection::INACTIVE`] if `enabled_individual` is `false`.
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`remote::Track`] data
    /// arrives.
    pub fn new(
        media_connections: &MediaConnections,
        track_id: TrackId,
        caps: TrackConstraints,
        sender_id: MemberId,
        mid: Option<String>,
        enabled_general: bool,
        enabled_individual: bool,
        track_events_sender: mpsc::UnboundedSender<TrackEvent>,
    ) -> Self {
        let connections = media_connections.0.borrow();
        let kind = MediaKind::from(&caps);
        let transceiver_direction = if enabled_individual {
            TransceiverDirection::RECV
        } else {
            TransceiverDirection::INACTIVE
        };

        let transceiver = if mid.is_none() {
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

        Self {
            track_id,
            caps,
            sender_id,
            transceiver: RefCell::new(transceiver),
            mid: RefCell::new(mid),
            track: RefCell::new(None),
            // general_media_exchange_state: Cell::new(
            //     media_exchange_state::Stable::from(enabled_general),
            // ),
            is_track_notified: Cell::new(false),
            // media_exchange_state_controller: TransitableStateController::new(
            //     media_exchange_state::Stable::from(enabled_individual),
            // ),
            peer_events_sender: connections.peer_events_sender.clone(),
            enabled_general: Cell::new(true),
            enabled_individual: Cell::new(true),
            track_events_sender,
        }
    }

    /// Returns [`TrackConstraints`] of this [`Receiver`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

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

    /// Returns `true` if this [`Receiver`] is receives media data.
    pub fn is_receiving(&self) -> bool {
        let is_recv_direction =
            self.transceiver.borrow().as_ref().map_or(false, |trnsvr| {
                trnsvr.has_direction(TransceiverDirection::RECV)
            });

        self.enabled_individual.get() && is_recv_direction
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

    /// Adds provided [`sys::MediaStreamTrack`] and [`Transceiver`] to this
    /// [`Receiver`].
    ///
    /// Sets [`sys::MediaStreamTrack::enabled`] same as [`Receiver::enabled`] of
    /// this [`Receiver`].
    pub fn set_remote_track(
        &self,
        transceiver: Transceiver,
        new_track: sys::MediaStreamTrack,
    ) {
        if let Some(old_track) = self.track.borrow().as_ref() {
            if old_track.id() == new_track.id() {
                return;
            }
        }

        let new_track =
            remote::Track::new(new_track, self.caps.media_source_kind());

        if self.enabled() {
            transceiver.add_direction(TransceiverDirection::RECV);
        } else {
            transceiver.sub_direction(TransceiverDirection::RECV);
        }
        new_track.set_enabled(self.enabled());

        self.transceiver.replace(Some(transceiver));
        self.track.replace(Some(new_track));
        self.maybe_notify_track();
    }

    /// Replaces [`Receiver`]'s [`Transceiver`] with a provided [`Transceiver`].
    ///
    /// Doesn't update [`TransceiverDirection`] of the [`Transceiver`].
    ///
    /// No-op if provided with the same [`Transceiver`] as already exists in
    /// this [`Receiver`].
    pub fn replace_transceiver(&self, transceiver: Transceiver) {
        if self.mid.borrow().as_ref() == transceiver.mid().as_ref() {
            self.transceiver.replace(Some(transceiver));
        }
    }

    /// Checks whether general media exchange state of the [`Receiver`] is in
    /// [`media_exchange_state::Stable::Disabled`].
    #[cfg(feature = "mockable")]
    pub fn is_general_disabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == media_exchange_state::Stable::Disabled
    }

    /// Returns [`Transceiver`] of this [`Receiver`].
    ///
    /// Returns [`None`] if this [`Receiver`] doesn't have [`Transceiver`].
    #[inline]
    pub fn transceiver(&self) -> Option<Transceiver> {
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
            let _ = self.peer_events_sender.unbounded_send(
                PeerEvent::NewRemoteTrack {
                    sender_id: self.sender_id.clone(),
                    track: track.clone(),
                },
            );
            self.is_track_notified.set(true);
        }
    }

    /// Indicates whether this [`Receiver`] is enabled.
    #[inline]
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled_individual.get()
        // self.media_exchange_state_controller.enabled()
    }
}
