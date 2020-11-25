//! [`crate::peer::PeerConnection`] media management.

pub mod media_exchange_state;
mod receiver;
mod sender;

use std::{cell::RefCell, collections::HashMap, convert::From, rc::Rc};

use derive_more::Display;
use futures::{channel::mpsc, future, future::LocalBoxFuture};
use medea_client_api_proto as proto;
use medea_reactive::DroppedError;
use proto::{Direction, MediaSourceKind, TrackId};
use tracerr::Traced;
use web_sys::RtcTrackEvent;

use crate::{
    media::{
        LocalTracksConstraints, MediaKind, MediaStreamTrack, RecvConstraints,
    },
    peer::{
        LocalStreamUpdateCriteria, PeerEvent, Transceiver, TransceiverDirection,
    },
    utils::{JasonError, JsCaused, JsError},
};

use super::{conn::RtcPeerConnection, tracks_request::TracksRequest};

use self::sender::SenderBuilder;

pub use self::{receiver::Receiver, sender::Sender};

/// Transceiver's sending ([`Sender`]) or receiving ([`Receiver`]) side.
pub trait TransceiverSide: Disableable {
    /// Returns [`TrackId`] of this [`TransceiverSide`].
    fn track_id(&self) -> TrackId;

    /// Returns [`MediaKind`] of this [`TransceiverSide`].
    fn kind(&self) -> MediaKind;

    /// Returns [`MediaSourceKind`] of this [`TransceiverSide`].
    fn source_kind(&self) -> MediaSourceKind;

    /// Returns [`mid`] of this [`TransceiverSide`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    fn mid(&self) -> Option<String>;

    /// Returns `true` if this [`TransceiverSide`] currently can be
    /// disabled/enabled without [`LocalTracksConstraints`] updating.
    fn is_transitable(&self) -> bool;
}

/// Default functions for dealing with [`media_exchange_state::Controller`] for
/// objects that use it.
pub trait Disableable {
    /// Returns reference to the [`media_exchange_state::Controller`].
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<media_exchange_state::Controller>;

    /// Returns [`media_exchange_state::State`] of this [`Disableable`].
    #[inline]
    fn media_exchange_state(&self) -> media_exchange_state::State {
        self.media_exchange_state_controller()
            .media_exchange_state()
    }

    /// Sets current [`media_exchange_state::State`] to
    /// [`media_exchange_state::State::Transition`].
    ///
    /// # Errors
    ///
    /// Implementors might return [`MediaConnectionsError`] if transition could
    /// not be made for some reason.
    #[inline]
    fn media_exchange_state_transition_to(
        &self,
        desired_state: media_exchange_state::Stable,
    ) -> Result<()> {
        self.media_exchange_state_controller()
            .transition_to(desired_state);

        Ok(())
    }

    /// Returns [`Future`] which will be resolved when
    /// [`media_exchange_state::State`] of this [`Disableable`] will be
    /// [`media_exchange_state::State::Stable`] or it is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaExchangeStateTransitsIntoOppositeState`]
    /// is returned if [`media_exchange_state::State`] transits into the
    /// opposite to the `desired_state`.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_media_exchange_state_stable(
        &self,
        desired_state: media_exchange_state::Stable,
    ) -> LocalBoxFuture<'static, Result<()>> {
        self.media_exchange_state_controller()
            .when_media_exchange_state_stable(desired_state)
    }

    /// Stops state transition timer of this [`Disableable`].
    #[inline]
    fn stop_media_exchange_state_transition_timeout(&self) {
        self.media_exchange_state_controller()
            .stop_transition_timeout()
    }

    /// Resets state transition timer of this [`Disableable`].
    #[inline]
    fn reset_media_exchange_state_transition_timeout(&self) {
        self.media_exchange_state_controller()
            .reset_transition_timeout()
    }

    /// Indicates whether media exchange state of the [`Disableable`] is in
    /// [`media_exchange_state::Stable::Disabled`].
    #[inline]
    #[cfg(feature = "mockable")]
    fn disabled(&self) -> bool {
        self.media_exchange_state_controller().disabled()
    }

    /// Indicates whether media exchange state of the [`Disableable`] is in
    /// [`media_exchange_state::Stable::Enabled`].
    #[inline]
    fn enabled(&self) -> bool {
        self.media_exchange_state_controller().enabled()
    }
}

/// Direction of the `MediaTrack`.
#[derive(Debug, Clone, Copy)]
pub enum TrackDirection {
    /// Sends media data.
    Send,

    /// Receives media data.
    Recv,
}

/// Errors that may occur in [`MediaConnections`] storage.
#[derive(Clone, Debug, Display, JsCaused)]
pub enum MediaConnectionsError {
    /// Occurs when the provided [`MediaStreamTrack`] cannot be inserted into
    /// provided [`Sender`]s transceiver.
    #[display(fmt = "Failed to insert Track to a sender: {}", _0)]
    CouldNotInsertLocalTrack(JsError),

    /// Occurs when [`MediaStreamTrack`] discovered by [`RtcPeerConnection`]
    /// could not be inserted into [`Receiver`].
    #[display(
        fmt = "Could not insert remote track with mid: {:?} into media \
               connections",
        _0
    )]
    CouldNotInsertRemoteTrack(String),

    /// Could not find [`RtcRtpTransceiver`] by `mid`.
    ///
    /// [`RtcRtpTransceiver`]: web_sys::RtcRtpTransceiver
    #[display(fmt = "Unable to find Transceiver with provided mid: {}", _0)]
    TransceiverNotFound(String),

    /// Occurs when cannot get the `mid` from the [`Sender`].
    #[display(fmt = "Peer has senders without mid")]
    SendersWithoutMid,

    /// Occurs when cannot get the `mid` from the [`Receiver`].
    #[display(fmt = "Peer has receivers without mid")]
    ReceiversWithoutMid,

    /// Occurs when not enough [`MediaStreamTrack`]s are inserted into senders.
    #[display(
        fmt = "Provided stream does not have all necessary MediaStreamTracks"
    )]
    InvalidMediaTracks,

    /// Occurs when [`MediaStreamTrack`] of inserted `MediaStream` does
    /// not satisfy [`Sender`] constraints.
    #[display(fmt = "Provided Track does not satisfy senders constraints")]
    InvalidMediaTrack,

    /// Occurs when [`media_exchange_state::State`] of [`Sender`] was dropped.
    #[display(fmt = "MediaExchangeState of Sender was dropped.")]
    MediaExchangeStateDropped,

    /// Occurs when [`media_exchange_state::State`] of [`Sender`] transits into
    /// opposite to expected [`media_exchange_state::State`].
    #[display(fmt = "MediaExchangeState of Sender transits into opposite to \
                     expected MediaExchangeState")]
    MediaExchangeStateTransitsIntoOppositeState,

    /// Invalid [`medea_client_api_proto::TrackPatchEvent`] for
    /// [`MediaStreamTrack`].
    #[display(fmt = "Invalid TrackPatch for Track with {} ID.", _0)]
    InvalidTrackPatch(TrackId),

    /// Some [`Sender`] can't be disabled because it required.
    #[display(fmt = "MediaExchangeState of Sender can't be transited into \
                     disabled state, because this Sender is required.")]
    CannotDisableRequiredSender,
}

impl From<DroppedError> for MediaConnectionsError {
    #[inline]
    fn from(_: DroppedError) -> Self {
        Self::MediaExchangeStateDropped
    }
}

type Result<T> = std::result::Result<T, Traced<MediaConnectionsError>>;

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// [`TrackId`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`TrackId`] to its [`Receiver`].
    receivers: HashMap<TrackId, Rc<Receiver>>,
}

impl InnerMediaConnections {
    /// Returns [`Iterator`] over [`Sender`]s with provided [`MediaKind`]
    /// and [`MediaSourceKind`].
    fn iter_senders_with_kind_and_source_kind(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> impl Iterator<Item = &Rc<Sender>> {
        self.senders
            .values()
            .filter(move |sender| sender.kind() == kind)
            .filter(move |sender| match source_kind {
                None => true,
                Some(source_kind) => {
                    sender.caps().media_source_kind() == source_kind
                }
            })
    }

    /// Returns [`Iterator`] over [`Receiver`]s with provided
    /// [`MediaKind`].
    fn iter_receivers_with_kind(
        &self,
        kind: MediaKind,
    ) -> impl Iterator<Item = &Rc<Receiver>> {
        self.receivers.values().filter(move |s| s.kind() == kind)
    }

    /// Returns all [`TransceiverSide`]s by provided [`TrackDirection`],
    /// [`MediaKind`] and [`MediaSourceKind`].
    fn get_transceivers_by_direction_and_kind(
        &self,
        direction: TrackDirection,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> Vec<Rc<dyn TransceiverSide>> {
        match direction {
            TrackDirection::Send => self
                .iter_senders_with_kind_and_source_kind(kind, source_kind)
                .map(|tx| Rc::clone(&tx) as Rc<dyn TransceiverSide>)
                .collect(),
            TrackDirection::Recv => self
                .iter_receivers_with_kind(kind)
                .map(|rx| Rc::clone(&rx) as Rc<dyn TransceiverSide>)
                .collect(),
        }
    }

    /// Creates [`Transceiver`] and adds it to the [`RtcPeerConnection`].
    fn add_transceiver(
        &self,
        kind: MediaKind,
        direction: TransceiverDirection,
    ) -> Transceiver {
        Transceiver::from(self.peer.add_transceiver(kind, direction))
    }

    /// Lookups [`Transceiver`] by the provided [`mid`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    fn get_transceiver_by_mid(&self, mid: &str) -> Option<Transceiver> {
        self.peer.get_transceiver_by_mid(mid).map(Transceiver::from)
    }
}

/// Storage of [`RtcPeerConnection`]'s [`Sender`] and [`Receiver`] tracks.
pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    /// Instantiates new [`MediaConnections`] storage for a given
    /// [`RtcPeerConnection`].
    #[inline]
    pub fn new(
        peer: Rc<RtcPeerConnection>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer,
            peer_events_sender,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }))
    }

    /// Returns all [`Sender`]s and [`Receiver`]s from this [`MediaConnections`]
    /// with provided [`MediaKind`], [`TrackDirection`] and
    /// [`MediaSourceKind`].
    pub fn get_transceivers_sides(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
    ) -> Vec<Rc<dyn TransceiverSide>> {
        self.0.borrow().get_transceivers_by_direction_and_kind(
            direction,
            kind,
            source_kind,
        )
    }

    /// Indicates whether all [`TransceiverSide`]s with provided [`MediaKind`],
    /// [`TrackDirection`] and [`MediaSourceKind`] is in the provided
    /// [`media_exchange_state::State`].
    pub fn is_all_tracks_in_media_exchange_state(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
        media_exchange_state: media_exchange_state::Stable,
    ) -> bool {
        let transceivers =
            self.0.borrow().get_transceivers_by_direction_and_kind(
                direction,
                kind,
                source_kind,
            );
        for transceiver in transceivers {
            if !transceiver.is_transitable() {
                continue;
            }
            if transceiver.media_exchange_state() != media_exchange_state.into()
            {
                return false;
            }
        }

        true
    }

    /// Returns mapping from a [`MediaStreamTrack`] ID to a `mid` of
    /// this track's [`RtcRtpTransceiver`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::SendersWithoutMid`] if some
    /// [`Sender`] doesn't have [mid].
    ///
    /// Errors with [`MediaConnectionsError::ReceiversWithoutMid`] if some
    /// [`Receiver`] doesn't have [mid].
    ///
    /// [`RtcRtpTransceiver`]: web_sys::RtcRtpTransceiver
    /// [mid]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver/mid
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let inner = self.0.borrow();
        let mut mids =
            HashMap::with_capacity(inner.senders.len() + inner.receivers.len());
        for (track_id, sender) in &inner.senders {
            mids.insert(
                *track_id,
                sender
                    .mid()
                    .ok_or(MediaConnectionsError::SendersWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        for (track_id, receiver) in &inner.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .ok_or(MediaConnectionsError::ReceiversWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        Ok(mids)
    }

    /// Returns activity statuses of the all [`Sender`]s and [`Receiver`]s from
    /// this [`MediaConnections`].
    pub fn get_transceivers_statuses(&self) -> HashMap<TrackId, bool> {
        let inner = self.0.borrow();

        let mut out = HashMap::new();
        for (track_id, sender) in &inner.senders {
            out.insert(*track_id, sender.is_publishing());
        }
        for (track_id, receiver) in &inner.receivers {
            out.insert(*track_id, receiver.is_receiving());
        }
        out
    }

    /// Returns [`Rc`] to [`TransceiverSide`] with a provided [`TrackId`].
    ///
    /// Returns `None` if [`TransceiverSide`] with a provided [`TrackId`]
    /// doesn't exists in this [`MediaConnections`].
    pub fn get_transceiver_side_by_id(
        &self,
        track_id: TrackId,
    ) -> Option<Rc<dyn TransceiverSide>> {
        let inner = self.0.borrow();
        inner
            .senders
            .get(&track_id)
            .map(|sndr| Rc::clone(&sndr) as Rc<dyn TransceiverSide>)
            .or_else(|| {
                inner
                    .receivers
                    .get(&track_id)
                    .map(|rcvr| Rc::clone(&rcvr) as Rc<dyn TransceiverSide>)
            })
    }

    /// Creates new [`Sender`]s and [`Receiver`]s for each new [`proto::Track`].
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if could not create
    /// new [`Sender`] cause transceiver with specified `mid` does not
    /// exist.
    pub fn create_tracks<I: IntoIterator<Item = proto::Track>>(
        &self,
        tracks: I,
        send_constraints: &LocalTracksConstraints,
        recv_constraints: &RecvConstraints,
    ) -> Result<()> {
        for track in tracks {
            let required = track.required();
            match track.direction {
                Direction::Send { mid, .. } => {
                    let media_exchange_state = if send_constraints
                        .enabled(&track.media_type)
                    {
                        media_exchange_state::Stable::Enabled
                    } else if required {
                        let e = tracerr::new!(
                            MediaConnectionsError::CannotDisableRequiredSender
                        );
                        let _ =
                            self.0.borrow().peer_events_sender.unbounded_send(
                                PeerEvent::FailedLocalMedia {
                                    error: JasonError::from(e.clone()),
                                },
                            );
                        return Err(e);
                    } else {
                        media_exchange_state::Stable::Disabled
                    };
                    let sndr = SenderBuilder {
                        media_connections: self,
                        track_id: track.id,
                        caps: track.media_type.into(),
                        mid,
                        media_exchange_state,
                        required,
                        send_constraints: send_constraints.clone(),
                    }
                    .build()
                    .map_err(tracerr::wrap!())?;
                    self.0.borrow_mut().senders.insert(track.id, sndr);
                }
                Direction::Recv { sender, mid } => {
                    let recv = Rc::new(Receiver::new(
                        self,
                        track.id,
                        track.media_type.into(),
                        sender,
                        mid,
                        recv_constraints,
                    ));
                    self.0.borrow_mut().receivers.insert(track.id, recv);
                }
            }
        }
        Ok(())
    }

    /// Updates [`Sender`]s and [`Receiver`]s of this [`super::PeerConnection`]
    /// with [`proto::TrackPatchEvent`].
    ///
    /// Returns [`MediaKind`] and [`MediaSourceKind`] for which local media
    /// stream should be updated.
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::InvalidTrackPatch`] if
    /// [`MediaStreamTrack`] with ID from [`proto::TrackPatchEvent`] doesn't
    /// exist.
    pub async fn patch_tracks(
        &self,
        tracks: Vec<proto::TrackPatchEvent>,
    ) -> Result<LocalStreamUpdateCriteria> {
        let mut result = LocalStreamUpdateCriteria::empty();
        for track_proto in tracks {
            if let Some(sender) = self.get_sender_by_id(track_proto.id) {
                if sender.update(&track_proto).await {
                    result.add(sender.kind(), sender.source_kind());
                }
            } else if let Some(receiver) =
                self.0.borrow_mut().receivers.get_mut(&track_proto.id)
            {
                receiver.update(&track_proto);
            } else {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidTrackPatch(track_proto.id)
                ));
            }
        }
        Ok(result)
    }

    /// Returns [`TracksRequest`] based on [`Sender`]s in this
    /// [`MediaConnections`]. [`Sender`]s are chosen based on provided
    /// [`LocalStreamUpdateCriteria`].
    pub fn get_tracks_request(
        &self,
        kinds: LocalStreamUpdateCriteria,
    ) -> Option<TracksRequest> {
        let mut stream_request = None;
        for sender in self.0.borrow().senders.values() {
            if kinds.has(sender.kind(), sender.source_kind()) {
                stream_request
                    .get_or_insert_with(TracksRequest::default)
                    .add_track_request(
                        sender.track_id(),
                        sender.caps().clone(),
                    );
            }
        }
        stream_request
    }

    /// Inserts provided tracks into [`Sender`]s based on track IDs.
    ///
    ///  [`MediaStreamTrack`]s are inserted into [`Sender`]'s
    /// [`RtcRtpTransceiver`]s via [`replaceTrack` method][1], changing its
    /// direction to `sendonly`.
    ///
    /// Returns [`HashMap`] with [`media_exchange_state::State`]s updates for
    /// the [`Sender`]s.
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::InvalidMediaTracks`] if provided
    /// [`HashMap`] doesn't contain required [`MediaStreamTrack`].
    ///
    /// With [`MediaConnectionsError::InvalidMediaTrack`] if some
    /// [`MediaStreamTrack`] cannot be inserted into associated [`Sender`]
    /// because of constraints mismatch.
    ///
    /// With [`MediaConnectionsError::CouldNotInsertLocalTrack`] if some
    /// [`MediaStreamTrack`] cannot be inserted into provided [`Sender`]s
    /// transceiver.
    ///
    /// [`RtcRtpTransceiver`]: web_sys::RtcRtpTransceiver
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_tracks(
        &self,
        tracks: &HashMap<TrackId, MediaStreamTrack>,
    ) -> Result<HashMap<TrackId, media_exchange_state::Stable>> {
        let inner = self.0.borrow();

        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track = Vec::with_capacity(inner.senders.len());
        let mut media_exchange_state_updates = HashMap::new();
        for sender in inner.senders.values() {
            if let Some(track) = tracks.get(&sender.track_id()).cloned() {
                if sender.caps().satisfies(&track) {
                    media_exchange_state_updates.insert(
                        sender.track_id(),
                        media_exchange_state::Stable::Enabled,
                    );
                    sender_and_track.push((sender, track));
                } else {
                    return Err(tracerr::new!(
                        MediaConnectionsError::InvalidMediaTrack
                    ));
                }
            } else if sender.caps().required() {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidMediaTracks
                ));
            } else {
                media_exchange_state_updates.insert(
                    sender.track_id(),
                    media_exchange_state::Stable::Disabled,
                );
            }
        }

        future::try_join_all(sender_and_track.into_iter().map(
            |(sender, track)| async move {
                Rc::clone(sender).insert_track(track).await?;
                sender.maybe_enable();
                Ok::<(), Traced<MediaConnectionsError>>(())
            },
        ))
        .await?;

        Ok(media_exchange_state_updates)
    }

    /// Handles [`RtcTrackEvent`] by adding new track to the corresponding
    /// [`Receiver`].
    ///
    /// Returns ID of associated [`Sender`] and provided track [`TrackId`], if
    /// any.
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::CouldNotInsertRemoteTrack`] if
    /// could not find [`Receiver`] by transceivers `mid`.
    pub fn add_remote_track(&self, track_event: &RtcTrackEvent) -> Result<()> {
        let inner = self.0.borrow();
        let transceiver = Transceiver::from(track_event.transceiver());
        let track = track_event.track();
        // Cannot fail, since transceiver is guaranteed to be negotiated at this
        // point.
        let mid = transceiver.mid().unwrap();

        for receiver in inner.receivers.values() {
            if let Some(recv_mid) = &receiver.mid() {
                if recv_mid == &mid {
                    receiver.set_remote_track(transceiver, track);
                    return Ok(());
                }
            }
        }
        Err(tracerr::new!(
            MediaConnectionsError::CouldNotInsertRemoteTrack(mid)
        ))
    }

    /// Iterates over all [`Receiver`]s with [`mid`] and without
    /// [`Transceiver`], trying to find the corresponding [`Transceiver`] in
    /// [`RtcPeerConnection`] and to insert it into the [`Receiver`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    pub fn sync_receivers(&self) {
        let inner = self.0.borrow();
        for receiver in inner
            .receivers
            .values()
            .filter(|rcvr| rcvr.transceiver().is_none())
        {
            if let Some(mid) = receiver.mid() {
                if let Some(trnscvr) = inner.peer.get_transceiver_by_mid(&mid) {
                    receiver.replace_transceiver(trnscvr.into())
                }
            }
        }
    }

    /// Returns [`Sender`] from this [`MediaConnections`] by [`TrackId`].
    #[inline]
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<Sender>> {
        self.0.borrow().senders.get(&id).cloned()
    }

    /// Returns all references to the [`TransceiverSide`]s from this
    /// [`MediaConnections`].
    fn get_all_transceivers_sides(&self) -> Vec<Rc<dyn TransceiverSide>> {
        let inner = self.0.borrow();
        inner
            .senders
            .values()
            .map(|s| Rc::clone(s) as Rc<dyn TransceiverSide>)
            .chain(
                inner
                    .receivers
                    .values()
                    .map(|r| Rc::clone(&r) as Rc<dyn TransceiverSide>),
            )
            .collect()
    }

    /// Stops all [`TransceiverSide`]s state transitions expiry timers.
    pub fn stop_state_transitions_timers(&self) {
        self.get_all_transceivers_sides()
            .into_iter()
            .for_each(|t| t.stop_media_exchange_state_transition_timeout())
    }

    /// Resets all [`TransceiverSide`]s state transitions expiry timers.
    pub fn reset_state_transitions_timers(&self) {
        self.get_all_transceivers_sides()
            .into_iter()
            .for_each(|t| t.reset_media_exchange_state_transition_timeout());
    }
}

#[cfg(feature = "mockable")]
impl MediaConnections {
    /// Indicates whether all [`Receiver`]s with [`MediaKind::Video`] are
    /// enabled.
    pub fn is_recv_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(MediaKind::Video)
            .find(|s| s.disabled())
            .is_none()
    }

    /// Indicates whether if all [`Receiver`]s with [`MediaKind::Audio`] are
    /// enabled.
    pub fn is_recv_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(MediaKind::Audio)
            .find(|s| s.disabled())
            .is_none()
    }

    /// Returns [`Receiver`] with the provided [`TrackId`].
    pub fn get_receiver_by_id(&self, id: TrackId) -> Option<Rc<Receiver>> {
        self.0.borrow().receivers.get(&id).cloned()
    }

    /// Indicates whether all [`Sender`]s with [`MediaKind::Audio`] are enabled.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind_and_source_kind(MediaKind::Audio, None)
            .all(|s| s.enabled())
    }

    /// Indicates whether all [`Sender`]s with [`MediaKind::Video`] are enabled.
    pub fn is_send_video_enabled(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind_and_source_kind(
                MediaKind::Video,
                source_kind,
            )
            .all(|s| s.enabled())
    }
}
