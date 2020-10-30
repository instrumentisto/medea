//! Implementation of the `MediaTrack` with a `Recv` direction.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::channel::mpsc;
use medea_client_api_proto as proto;
use medea_client_api_proto::{MemberId, TrackPatchEvent};
use proto::TrackId;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{
    media::{MediaKind, MediaStreamTrack, RecvConstraints, TrackConstraints},
    peer::{
        media::{
            media_exchange_state::MediaExchangeStateController, TransceiverSide,
        },
        transceiver::Transceiver,
        Disableable, MediaConnections, PeerEvent, TransceiverDirection,
    },
};

use super::media_exchange_state::StableMediaExchangeState;

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    caps: TrackConstraints,
    sender_id: MemberId,
    transceiver: RefCell<Option<Transceiver>>,
    mid: RefCell<Option<String>>,
    track: RefCell<Option<MediaStreamTrack>>,
    general_media_exchange_state: Cell<StableMediaExchangeState>,
    is_track_notified: Cell<bool>,
    media_exchange_state_controller: Rc<MediaExchangeStateController>,
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`, otherwise
    /// creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be injected
    /// when [`MediaStreamTrack`] arrives.
    ///
    /// Created [`RtcRtpTransceiver`] direction is set to
    /// [`TransceiverDirection::Inactive`] if media receiving is disabled in
    /// provided [`RecvConstraints`].
    ///
    /// `track` field in the created [`Receiver`] will be `None`, since
    /// [`Receiver`] must be created before the actual [`MediaStreamTrack`] data
    /// arrives.
    pub fn new(
        media_connections: &MediaConnections,
        track_id: TrackId,
        caps: TrackConstraints,
        sender_id: MemberId,
        mid: Option<String>,
        recv_constraints: &RecvConstraints,
    ) -> Self {
        let connections = media_connections.0.borrow();
        let kind = MediaKind::from(&caps);
        let enabled = match kind {
            MediaKind::Audio => recv_constraints.is_audio_enabled(),
            MediaKind::Video => recv_constraints.is_video_enabled(),
        };
        let transceiver_direction = if enabled {
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
            general_media_exchange_state: Cell::new(
                StableMediaExchangeState::from(!enabled),
            ),
            is_track_notified: Cell::new(false),
            media_exchange_state_controller: MediaExchangeStateController::new(
                StableMediaExchangeState::from(!enabled),
            ),
            peer_events_sender: connections.peer_events_sender.clone(),
        }
    }

    /// Returns [`TrackConstraints`] of this [`Receiver`].
    #[inline]
    pub fn caps(&self) -> &TrackConstraints {
        &self.caps
    }

    /// Returns `true` if this [`Receiver`] is receives media data.
    pub fn is_receiving(&self) -> bool {
        let is_enabled = self.media_exchange_state_controller.is_enabled();
        let is_recv_direction =
            self.transceiver.borrow().as_ref().map_or(false, |trnsvr| {
                trnsvr.has_direction(TransceiverDirection::RECV)
            });

        is_enabled && is_recv_direction
    }

    /// Adds provided [`SysMediaStreamTrack`] and [`RtcRtpTransceiver`] to this
    /// [`Receiver`].
    ///
    /// Sets [`MediaStreamTrack::enabled`] same as [`Receiver::enabled`] of this
    /// [`Receiver`].
    pub fn set_remote_track(
        &self,
        transceiver: Transceiver,
        new_track: SysMediaStreamTrack,
    ) {
        if let Some(old_track) = self.track.borrow().as_ref() {
            if old_track.id() == new_track.id() {
                return;
            }
        }

        let new_track =
            MediaStreamTrack::new(new_track, self.caps.media_source_kind());

        if self.is_not_disabled() {
            transceiver.add_direction(TransceiverDirection::RECV);
        } else {
            transceiver.sub_direction(TransceiverDirection::RECV);
        }
        new_track.set_enabled(self.is_not_disabled());

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

    /// Updates [`Receiver`] based on the provided [`TrackPatchEvent`].
    pub fn update(&self, track_patch: &TrackPatchEvent) {
        if self.track_id != track_patch.id {
            return;
        }
        if let Some(is_disabled_general) = track_patch.is_disabled_general {
            self.update_general_mute_state(is_disabled_general.into());
        }
        if let Some(is_disabled) = track_patch.is_disabled_individual {
            self.media_exchange_state_controller.update(is_disabled);
        }
    }

    /// Checks whether general media exchange state of the [`Receiver`] is in
    /// [`StableMediaExchangeState::Disabled`].
    #[cfg(feature = "mockable")]
    pub fn is_general_disabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == StableMediaExchangeState::Disabled
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

    /// Updates [`TransceiverDirection`] and underlying [`MediaStreamTrack`]
    /// based on the provided [`StableMediaExchangeState`].
    ///
    /// Updates [`InnerReceiver::general_mute_state`].
    fn update_general_mute_state(&self, mute_state: StableMediaExchangeState) {
        if self.general_media_exchange_state.get() != mute_state {
            self.general_media_exchange_state.set(mute_state);
            match mute_state {
                StableMediaExchangeState::Disabled => {
                    if let Some(track) = self.track.borrow().as_ref() {
                        track.set_enabled(false);
                    }
                    if let Some(trnscvr) = self.transceiver.borrow().as_ref() {
                        trnscvr.sub_direction(TransceiverDirection::RECV);
                    }
                }
                StableMediaExchangeState::Enabled => {
                    if let Some(track) = self.track.borrow().as_ref() {
                        track.set_enabled(true);
                    }
                    if let Some(trnscvr) = self.transceiver.borrow().as_ref() {
                        trnscvr.add_direction(TransceiverDirection::RECV);
                    }
                }
            }
            self.maybe_notify_track();
        }
    }

    /// Checks whether general media exchange state of this [`Receiver`] is in
    /// [`StableMediaExchangeState::Enabled`].
    fn is_not_disabled(&self) -> bool {
        self.general_media_exchange_state.get()
            == StableMediaExchangeState::Enabled
    }
}

impl Disableable for Receiver {
    #[inline]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController> {
        self.media_exchange_state_controller.clone()
    }
}

impl TransceiverSide for Receiver {
    #[inline]
    fn track_id(&self) -> TrackId {
        self.track_id
    }

    #[inline]
    fn kind(&self) -> MediaKind {
        MediaKind::from(&self.caps)
    }

    fn mid(&self) -> Option<String> {
        if self.mid.borrow().is_none() && self.transceiver.borrow().is_some() {
            if let Some(transceiver) =
                self.transceiver.borrow().as_ref().cloned()
            {
                self.mid.replace(Some(transceiver.mid()?));
            }
        }
        self.mid.borrow().clone()
    }

    #[inline]
    fn is_transitable(&self) -> bool {
        true
    }
}
