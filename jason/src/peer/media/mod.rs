//! [`PeerConnection`] media management.
//!
//! [`PeerConnection`]: crate::peer::PeerConnection

pub mod receiver;
pub mod sender;
mod transitable_state;

use std::{cell::RefCell, collections::HashMap, convert::From, rc::Rc};

use derive_more::Display;
use futures::{channel::mpsc, future, future::LocalBoxFuture};
use medea_client_api_proto as proto;
#[cfg(feature = "mockable")]
use medea_client_api_proto::{MediaType, MemberId};
use medea_reactive::DroppedError;
use proto::{MediaSourceKind, TrackId};
use tracerr::Traced;

#[cfg(feature = "mockable")]
use crate::media::{LocalTracksConstraints, RecvConstraints};
use crate::{
    media::{track::local, MediaKind},
    peer::{LocalStreamUpdateCriteria, PeerEvent},
    platform,
    utils::JsCaused,
};

use super::tracks_request::TracksRequest;

#[doc(inline)]
pub use self::{
    receiver::Receiver,
    sender::Sender,
    transitable_state::{
        media_exchange_state, mute_state, InStable, InTransition,
        MediaExchangeState, MediaExchangeStateController, MediaState,
        MuteState, MuteStateController, TransitableState,
        TransitableStateController,
    },
};

/// Transceiver's sending ([`Sender`]) or receiving ([`Receiver`]) side.
///
/// [`Sender`]: self::sender::Sender
/// [`Receiver`]: self::receiver::Receiver
pub trait TransceiverSide: MediaStateControllable {
    /// Returns [`TrackId`] of this [`TransceiverSide`].
    fn track_id(&self) -> TrackId;

    /// Returns [`MediaKind`] of this [`TransceiverSide`].
    fn kind(&self) -> MediaKind;

    /// Returns [`MediaSourceKind`] of this [`TransceiverSide`].
    fn source_kind(&self) -> MediaSourceKind;

    /// Returns `true` if this [`TransceiverSide`] currently can be
    /// disabled/enabled without [`LocalTracksConstraints`] updating.
    ///
    /// [`LocalTracksConstraints`]: super::LocalTracksConstraints
    fn is_transitable(&self) -> bool;
}

/// Default functions for dealing with [`MediaExchangeStateController`] and
/// [`MuteStateController`] for objects that use it.
pub trait MediaStateControllable {
    /// Returns reference to the [`MediaExchangeStateController`].
    #[must_use]
    fn media_exchange_state_controller(
        &self,
    ) -> Rc<MediaExchangeStateController>;

    /// Returns a reference to the [`MuteStateController`].
    #[must_use]
    fn mute_state_controller(&self) -> Rc<MuteStateController>;

    /// Returns [`MediaExchangeState`] of this [`MediaStateControllable`].
    #[inline]
    #[must_use]
    fn media_exchange_state(&self) -> MediaExchangeState {
        self.media_exchange_state_controller().state()
    }

    /// Returns [`MuteState`] of this [`MediaStateControllable`].
    #[inline]
    #[must_use]
    fn mute_state(&self) -> MuteState {
        self.mute_state_controller().state()
    }

    /// Sets current [`MediaState`] to [`TransitableState::Transition`].
    ///
    /// # Errors
    ///
    /// Implementors might return [`MediaConnectionsError`] if transition could
    /// not be made for some reason.
    #[inline]
    fn media_state_transition_to(
        &self,
        desired_state: MediaState,
    ) -> Result<()> {
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

    /// Indicates whether [`Room`] should subscribe to the [`MediaState`] update
    /// when updating [`MediaStateControllable`] to the provided [`MediaState`].
    ///
    /// [`Room`]: crate::room::Room
    #[must_use]
    fn is_subscription_needed(&self, desired_state: MediaState) -> bool {
        match desired_state {
            MediaState::MediaExchange(media_exchange) => {
                let current = self.media_exchange_state();
                match current {
                    MediaExchangeState::Transition(_) => true,
                    MediaExchangeState::Stable(stable) => {
                        stable != media_exchange
                    }
                }
            }
            MediaState::Mute(mute_state) => {
                let current = self.mute_state();
                match current {
                    MuteState::Transition(_) => true,
                    MuteState::Stable(stable) => stable != mute_state,
                }
            }
        }
    }

    /// Indicates whether [`Room`] should send [`TrackPatchCommand`] to the
    /// server when updating [`MediaStateControllable`] to the provided
    /// [`MediaState`].
    ///
    /// [`TrackPatchCommand`]: medea_client_api_proto::TrackPatchCommand
    /// [`Room`]: crate::room::Room
    #[must_use]
    fn is_track_patch_needed(&self, desired_state: MediaState) -> bool {
        match desired_state {
            MediaState::MediaExchange(media_exchange) => {
                let current = self.media_exchange_state();
                match current {
                    MediaExchangeState::Stable(stable) => {
                        stable != media_exchange
                    }
                    MediaExchangeState::Transition(transition) => {
                        transition.intended() != media_exchange
                    }
                }
            }
            MediaState::Mute(mute_state) => {
                let current = self.mute_state();
                match current {
                    MuteState::Stable(stable) => stable != mute_state,
                    MuteState::Transition(transition) => {
                        transition.intended() != mute_state
                    }
                }
            }
        }
    }

    /// Returns [`Future`] which will be resolved when [`MediaState`] of this
    /// [`MediaStateControllable`] will be [`TransitableState::Stable`] or it's
    /// dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MediaStateTransitsIntoOppositeState`]
    /// is returned if [`MediaState`] transits into the opposite to the
    /// `desired_state`.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    fn when_media_state_stable(
        &self,
        desired_state: MediaState,
    ) -> LocalBoxFuture<'static, Result<()>> {
        match desired_state {
            MediaState::Mute(desired_state) => self
                .mute_state_controller()
                .when_media_state_stable(desired_state),
            MediaState::MediaExchange(desired_state) => self
                .media_exchange_state_controller()
                .when_media_state_stable(desired_state),
        }
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
#[js(error = "platform::Error")]
pub enum MediaConnectionsError {
    /// Occurs when the provided [`local::Track`] cannot be inserted into
    /// provided [`Sender`]s transceiver.
    ///
    /// [`Sender`]: self::sender::Sender
    #[display(fmt = "Failed to insert Track to a sender: {}", _0)]
    CouldNotInsertLocalTrack(platform::Error),

    /// Occurs when [`remote::Track`] discovered by a
    /// [`platform::RtcPeerConnection`] couldn't be inserted into a
    /// [`Receiver`].
    ///
    /// [`remote::Track`]: crate::media::track::remote::Track
    /// [`Receiver`]: self::receiver::Receiver
    #[display(
        fmt = "Could not insert remote track with mid: {:?} into media \
               connections",
        _0
    )]
    CouldNotInsertRemoteTrack(String),

    /// Could not find a [`platform::Transceiver`] by `mid`.
    #[display(fmt = "Unable to find Transceiver with provided mid: {}", _0)]
    TransceiverNotFound(String),

    /// Occurs when cannot get the `mid` from the [`Sender`].
    ///
    /// [`Sender`]: self::sender::Sender
    #[display(fmt = "Peer has senders without mid")]
    SendersWithoutMid,

    /// Occurs when cannot get the `mid` from the [`Receiver`].
    ///
    /// [`Receiver`]: self::receiver::Receiver
    #[display(fmt = "Peer has receivers without mid")]
    ReceiversWithoutMid,

    /// Occurs when not enough [`local::Track`]s are inserted into senders.
    #[display(fmt = "Provided stream does not have all necessary Tracks")]
    InvalidMediaTracks,

    /// Occurs when [`local::Track`] does not satisfy [`Sender`] constraints.
    ///
    /// [`Sender`]: self::sender::Sender
    #[display(fmt = "Provided Track does not satisfy senders constraints")]
    InvalidMediaTrack,

    /// Occurs when [`MediaExchangeState`] of [`Sender`] was dropped.
    ///
    /// [`Sender`]: self::sender::Sender
    #[display(fmt = "MediaExchangeState of Sender was dropped.")]
    MediaExchangeStateDropped,

    /// Occurs when [`MediaState`] of [`Sender`] transits into opposite
    /// to expected [`MediaState`].
    ///
    /// [`Sender`]: self::sender::Sender
    #[display(fmt = "MediaState of Sender transits into opposite to \
                     expected MediaExchangeState")]
    MediaStateTransitsIntoOppositeState,

    /// Invalid [`medea_client_api_proto::TrackPatchEvent`] for
    /// [`local::Track`].
    #[display(fmt = "Invalid TrackPatch for Track with {} ID.", _0)]
    InvalidTrackPatch(TrackId),

    /// Some [`Sender`] can't be disabled because it required.
    ///
    /// [`Sender`]: self::sender::Sender
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
    /// Reference to the parent [`platform::RtcPeerConnection`].
    ///
    /// Used to generate transceivers for [`Sender`]s and [`Receiver`]s.
    ///
    /// [`Sender`]: self::sender::Sender
    /// [`Receiver`]: self::receiver::Receiver
    peer: Rc<platform::RtcPeerConnection>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// [`TrackId`] to its [`sender::Component`].
    senders: HashMap<TrackId, sender::Component>,

    /// [`TrackId`] to its [`receiver::Component`].
    receivers: HashMap<TrackId, receiver::Component>,
}

impl InnerMediaConnections {
    /// Returns [`Iterator`] over [`sender::Component`]s with provided
    /// [`MediaKind`] and [`MediaSourceKind`].
    fn iter_senders_with_kind_and_source_kind(
        &self,
        kind: MediaKind,
        source_kind: Option<MediaSourceKind>,
    ) -> impl Iterator<Item = &sender::Component> {
        self.senders
            .values()
            .filter(move |sender| sender.state().kind() == kind)
            .filter(move |sender| match source_kind {
                None => true,
                Some(source_kind) => {
                    sender.caps().media_source_kind() == source_kind
                }
            })
    }

    /// Returns [`Iterator`] over [`receiver::Component`]s with provided
    /// [`MediaKind`].
    fn iter_receivers_with_kind(
        &self,
        kind: MediaKind,
    ) -> impl Iterator<Item = &receiver::Component> {
        self.receivers
            .values()
            .filter(move |s| s.state().kind() == kind)
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
                .map(|tx| tx.state() as Rc<dyn TransceiverSide>)
                .collect(),
            TrackDirection::Recv => self
                .iter_receivers_with_kind(kind)
                .map(|rx| rx.state() as Rc<dyn TransceiverSide>)
                .collect(),
        }
    }

    /// Creates a [`platform::Transceiver`] and adds it to the
    /// [`platform::RtcPeerConnection`].
    fn add_transceiver(
        &self,
        kind: MediaKind,
        direction: platform::TransceiverDirection,
    ) -> platform::Transceiver {
        platform::Transceiver::from(self.peer.add_transceiver(kind, direction))
    }

    /// Lookups a [`platform::Transceiver`] by the provided [`mid`].
    ///
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    fn get_transceiver_by_mid(
        &self,
        mid: &str,
    ) -> Option<platform::Transceiver> {
        self.peer
            .get_transceiver_by_mid(mid)
            .map(platform::Transceiver::from)
    }
}

/// Storage of [`platform::RtcPeerConnection`]'s [`sender::Component`] and
/// [`receiver::Component`].
pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    /// Instantiates a new [`MediaConnections`] storage for the given
    /// [`platform::RtcPeerConnection`].
    #[inline]
    pub fn new(
        peer: Rc<platform::RtcPeerConnection>,
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
    ///
    /// [`Sender`]: self::sender::Sender
    /// [`Receiver`]: self::receiver::Receiver
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
    /// [`MediaExchangeState`].
    #[must_use]
    pub fn is_all_tracks_in_media_state(
        &self,
        kind: MediaKind,
        direction: TrackDirection,
        source_kind: Option<MediaSourceKind>,
        state: MediaState,
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

            let not_in_state = match state {
                MediaState::Mute(mute_state) => {
                    transceiver.mute_state() != mute_state.into()
                }
                MediaState::MediaExchange(media_exchange) => {
                    transceiver.media_exchange_state() != media_exchange.into()
                }
            };
            if not_in_state {
                return false;
            }
        }

        true
    }

    /// Returns mapping from a [`proto::Track`] ID to a `mid` of this track's
    /// [`platform::Transceiver`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::SendersWithoutMid`] if some
    /// [`Sender`] doesn't have [mid].
    ///
    /// Errors with [`MediaConnectionsError::ReceiversWithoutMid`] if some
    /// [`Receiver`] doesn't have [mid].
    ///
    /// [`Sender`]: self::sender::Sender
    /// [`Receiver`]: self::receiver::Receiver
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

    /// Returns activity statuses of the all the [`Sender`]s and [`Receiver`]s
    /// from these [`MediaConnections`].
    ///
    /// [`Sender`]: self::sender::Sender
    /// [`Receiver`]: self::receiver::Receiver
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
            .map(|sndr| sndr.state() as Rc<dyn TransceiverSide>)
            .or_else(|| {
                inner
                    .receivers
                    .get(&track_id)
                    .map(|rcvr| rcvr.state() as Rc<dyn TransceiverSide>)
            })
    }

    /// Inserts new [`sender::Component`] into [`MediaConnections`].
    #[inline]
    pub fn insert_sender(&self, sender: sender::Component) {
        self.0
            .borrow_mut()
            .senders
            .insert(sender.state().id(), sender);
    }

    /// Inserts new [`receiver::Component`] into [`MediaConnections`].
    #[inline]
    pub fn insert_receiver(&self, receiver: receiver::Component) {
        self.0
            .borrow_mut()
            .receivers
            .insert(receiver.state().id(), receiver);
    }

    /// Returns [`TracksRequest`] based on [`Sender`]s in this
    /// [`MediaConnections`]. [`Sender`]s are chosen based on provided
    /// [`LocalStreamUpdateCriteria`].
    ///
    /// [`Sender`]: self::sender::Sender
    pub fn get_tracks_request(
        &self,
        kinds: LocalStreamUpdateCriteria,
    ) -> Option<TracksRequest> {
        let mut stream_request = None;
        for sender in self.0.borrow().senders.values() {
            if kinds
                .has(sender.state().media_kind(), sender.state().media_source())
            {
                stream_request
                    .get_or_insert_with(TracksRequest::default)
                    .add_track_request(
                        sender.state().track_id(),
                        sender.caps().clone(),
                    );
            }
        }
        stream_request
    }

    /// Inserts provided tracks into [`Sender`]s based on track IDs.
    ///
    /// [`local::Track`]s are inserted into [`Sender`]'s
    /// [`platform::Transceiver`]s via a [`replaceTrack` method][1], changing
    /// its direction to `sendonly`.
    ///
    /// Returns [`HashMap`] with [`media_exchange_state::Stable`]s updates for
    /// the [`Sender`]s.
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::InvalidMediaTracks`] if provided
    /// [`HashMap`] doesn't contain required [`local::Track`].
    ///
    /// With [`MediaConnectionsError::InvalidMediaTrack`] if some
    /// [`local::Track`] cannot be inserted into associated [`Sender`] because
    /// of constraints mismatch.
    ///
    /// With [`MediaConnectionsError::CouldNotInsertLocalTrack`] if some
    /// [`local::Track`] cannot be inserted into provided [`Sender`]s
    /// transceiver.
    ///
    /// [`Sender`]: self::sender::Sender
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_tracks(
        &self,
        tracks: &HashMap<TrackId, Rc<local::Track>>,
    ) -> Result<HashMap<TrackId, media_exchange_state::Stable>> {
        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track =
            Vec::with_capacity(self.0.borrow().senders.len());
        let mut media_exchange_state_updates = HashMap::new();
        for sender in self.0.borrow().senders.values() {
            if let Some(track) = tracks.get(&sender.state().id()).cloned() {
                if sender.caps().satisfies(track.as_ref()) {
                    media_exchange_state_updates.insert(
                        sender.state().id(),
                        media_exchange_state::Stable::Enabled,
                    );
                    sender_and_track.push((sender.obj(), track));
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
                    sender.state().id(),
                    media_exchange_state::Stable::Disabled,
                );
            }
        }

        future::try_join_all(sender_and_track.into_iter().map(
            |(sender, track)| async move {
                Rc::clone(&sender).insert_track(track).await?;
                Ok::<(), Traced<MediaConnectionsError>>(())
            },
        ))
        .await?;

        Ok(media_exchange_state_updates)
    }

    /// Adds a new track to the corresponding [`Receiver`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::CouldNotInsertRemoteTrack`] if
    /// could not find [`Receiver`] by transceivers `mid`.
    ///
    /// # Panics
    ///
    /// If the provided [`platform::Transceiver`] doesn't have a [`mid`]. Not
    /// supposed to happen, since [`platform::MediaStreamTrack`] is only fired
    /// when a [`platform::Transceiver`] is negotiated, thus have a [`mid`].
    ///
    /// [`Sender`]: self::sender::Sender
    /// [`Receiver`]: self::receiver::Receiver
    /// [`mid`]: https://w3.org/TR/webrtc/#dom-rtptransceiver-mid
    pub fn add_remote_track(
        &self,
        track: platform::MediaStreamTrack,
        transceiver: platform::Transceiver,
    ) -> Result<()> {
        let inner = self.0.borrow();
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
    /// [`platform::Transceiver`], trying to find the corresponding
    /// [`platform::Transceiver`] in the [`platform::RtcPeerConnection`] and to
    /// insert it into the [`Receiver`].
    ///
    /// [`Receiver`]: self::receiver::Receiver
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

    /// Returns all [`Sender`]s which are matches provided
    /// [`LocalStreamUpdateCriteria`] and doesn't have [`local::Track`].
    ///
    /// [`Sender`]: self::sender::Sender
    pub fn get_senders_without_tracks_ids(
        &self,
        kinds: LocalStreamUpdateCriteria,
    ) -> Vec<TrackId> {
        self.0
            .borrow()
            .senders
            .values()
            .filter_map(|s| {
                if kinds.has(s.state().kind(), s.state().source_kind())
                    && s.state().enabled()
                    && !s.has_track()
                {
                    Some(s.state().id())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Drops [`local::Track`]s of all [`Sender`]s which are matches provided
    /// [`LocalStreamUpdateCriteria`].
    ///
    /// [`Sender`]: self::sender::Sender
    pub async fn drop_send_tracks(&self, kinds: LocalStreamUpdateCriteria) {
        let remove_tracks_fut = future::join_all(
            self.0.borrow().senders.values().filter_map(|s| {
                if kinds.has(s.state().kind(), s.state().source_kind()) {
                    let sender = s.obj();
                    Some(async move {
                        sender.remove_track().await;
                    })
                } else {
                    None
                }
            }),
        );
        remove_tracks_fut.await;
    }

    /// Removes a [`sender::Component`] or a [`receiver::Component`] with the
    /// provided [`TrackId`] from these [`MediaConnections`].
    pub fn remove_track(&self, track_id: TrackId) {
        let mut inner = self.0.borrow_mut();
        if inner.receivers.remove(&track_id).is_none() {
            inner.senders.remove(&track_id);
        }
    }
}

#[cfg(feature = "mockable")]
impl MediaConnections {
    /// Indicates whether all [`Receiver`]s with [`MediaKind::Video`] are
    /// enabled.
    ///
    /// [`Receiver`]: self::receiver::Receiver
    #[must_use]
    pub fn is_recv_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(MediaKind::Video)
            .find(|s| !s.state().enabled_individual())
            .is_none()
    }

    /// Indicates whether if all [`Receiver`]s with [`MediaKind::Audio`] are
    /// enabled.
    ///
    /// [`Receiver`]: self::receiver::Receiver
    #[must_use]
    pub fn is_recv_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_receivers_with_kind(MediaKind::Audio)
            .find(|s| !s.state().enabled_individual())
            .is_none()
    }

    /// Returns [`Receiver`] with the provided [`TrackId`].
    ///
    /// [`Receiver`]: self::receiver::Receiver
    #[must_use]
    pub fn get_receiver_by_id(
        &self,
        id: TrackId,
    ) -> Option<Rc<receiver::Receiver>> {
        self.0.borrow().receivers.get(&id).map(|r| r.obj())
    }

    /// Returns [`Sender`] with a provided [`TrackId`].
    ///
    /// [`Sender`]: self::sender::Sender
    #[must_use]
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<sender::Sender>> {
        self.0.borrow().senders.get(&id).map(|r| r.obj())
    }

    /// Indicates whether all [`Sender`]s with [`MediaKind::Audio`] are enabled.
    ///
    /// [`Sender`]: self::sender::Sender
    #[must_use]
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind_and_source_kind(MediaKind::Audio, None)
            .all(|s| s.state().enabled())
    }

    /// Indicates whether all [`Sender`]s with [`MediaKind::Video`] are enabled.
    ///
    /// [`Sender`]: self::sender::Sender
    #[must_use]
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
            .all(|s| s.state().enabled())
    }

    /// Indicates whether all [`Sender`]'s video tracks are unmuted.
    ///
    /// [`Sender`]: self::sender::Sender
    #[must_use]
    pub fn is_send_video_unmuted(
        &self,
        source_kind: Option<MediaSourceKind>,
    ) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind_and_source_kind(
                MediaKind::Video,
                source_kind,
            )
            .find(|s| s.muted())
            .is_none()
    }

    /// Indicates whether all [`Sender`]'s audio tracks are unmuted.
    ///
    /// [`Sender`]: self::sender::Sender
    #[must_use]
    pub fn is_send_audio_unmuted(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind_and_source_kind(MediaKind::Audio, None)
            .find(|s| s.muted())
            .is_none()
    }

    /// Creates new [`sender::Component`] with the provided data.
    pub fn create_sender(
        &self,
        id: TrackId,
        media_type: MediaType,
        mid: Option<String>,
        receivers: Vec<MemberId>,
        send_constraints: &LocalTracksConstraints,
    ) -> Result<sender::Component> {
        let sender_state = sender::State::new(
            id,
            mid.clone(),
            media_type.clone(),
            receivers,
            send_constraints.clone(),
        )?;
        let sender = sender::Sender::new(
            &sender_state,
            &self,
            send_constraints.clone(),
            mpsc::unbounded().0,
        )?;

        Ok(sender::Component::new(sender, Rc::new(sender_state)))
    }

    /// Creates new [`receiver::Component`] with the provided data.
    #[must_use]
    pub fn create_receiver(
        &self,
        id: TrackId,
        media_type: MediaType,
        mid: Option<String>,
        sender: MemberId,
        recv_constraints: &RecvConstraints,
    ) -> receiver::Component {
        let state = receiver::State::new(
            id,
            mid.clone(),
            media_type.clone(),
            sender.clone(),
        );
        let receiver = receiver::Receiver::new(
            &state,
            &self,
            mpsc::unbounded().0,
            recv_constraints,
        );

        receiver::Component::new(Rc::new(receiver), Rc::new(state))
    }

    /// Creates new [`sender::Component`]s/[`receiver::Component`]s from the
    /// provided [`proto::Track`]s.
    pub fn create_tracks(
        &self,
        tracks: Vec<proto::Track>,
        send_constraints: &LocalTracksConstraints,
        recv_constraints: &RecvConstraints,
    ) -> Result<()> {
        use medea_client_api_proto::Direction;
        for track in tracks {
            match track.direction {
                Direction::Send { mid, receivers } => {
                    let component = self.create_sender(
                        track.id,
                        track.media_type,
                        mid,
                        receivers,
                        send_constraints,
                    )?;
                    self.0.borrow_mut().senders.insert(track.id, component);
                }
                Direction::Recv { mid, sender } => {
                    let component = self.create_receiver(
                        track.id,
                        track.media_type,
                        mid,
                        sender,
                        recv_constraints,
                    );
                    self.0.borrow_mut().receivers.insert(track.id, component);
                }
            }
        }
        Ok(())
    }

    /// Returns all underlying [`Sender`]'s.
    ///
    /// [`Sender`]: self::sender::Sender
    pub fn get_senders(&self) -> Vec<Rc<sender::Sender>> {
        self.0
            .borrow()
            .senders
            .values()
            .map(|sndr| sndr.obj())
            .collect()
    }

    /// Returns [`sender::State`] with the provided [`TrackId`].
    #[inline]
    #[must_use]
    pub fn get_sender_state_by_id(
        &self,
        id: TrackId,
    ) -> Option<Rc<sender::State>> {
        self.0.borrow().senders.get(&id).map(|r| r.state())
    }
}
