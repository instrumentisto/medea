//! [`crate::peer::PeerConnection`] media management.

mod mute_state;

use std::{
    cell::RefCell, collections::HashMap, convert::From, future::Future, rc::Rc,
    time::Duration,
};

use derive_more::Display;
use futures::{channel::mpsc, future, future::Either, StreamExt};
use medea_client_api_proto as proto;
use medea_reactive::{DroppedError, ObservableCell};
use proto::{Direction, PeerId, Track, TrackId};
use tracerr::Traced;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

use crate::{
    media::{MediaStreamTrack, TrackConstraints},
    peer::PeerEvent,
    utils::{resettable_delay_for, JsCaused, JsError, ResettableDelayHandle},
};

use super::{
    conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
    stream::PeerMediaStream,
    stream_request::StreamRequest,
};

pub use self::mute_state::{MuteState, MuteStateTransition, StableMuteState};
use crate::utils::console_error;
use medea_client_api_proto::Mid;
use std::collections::HashSet;

/// Errors that may occur in [`MediaConnections`] storage.
#[derive(Debug, Display, JsCaused)]
pub enum MediaConnectionsError {
    /// Occurs when the provided [`MediaStreamTrack`] cannot be inserted into
    /// provided [`Sender`]s transceiver.
    #[display(fmt = "Failed to insert Track to a sender: {}", _0)]
    CouldNotInsertTrack(JsError),

    /// Could not find [`RtcRtpTransceiver`] by `mid`.
    #[display(fmt = "Unable to find Transceiver with provided mid: {}", _0)]
    TransceiverNotFound(Mid),

    /// Occurs when cannot get the `mid` from the [`Sender`].
    #[display(fmt = "Peer has senders without mid")]
    SendersWithoutMid,

    /// Occurs when cannot get the `mid` from the [`Receiver`].
    #[display(fmt = "Peer has receivers without mid")]
    ReceiversWithoutMid,

    /// Occurs when inserted [`PeerMediaStream`] dont have all necessary
    /// [`MediaStreamTrack`]s.
    #[display(fmt = "Provided stream does not have all necessary Tracks")]
    InvalidMediaStream,

    /// Occurs when [`MediaStreamTrack`] of inserted [`PeerMediaStream`] does
    /// not satisfy [`Sender`] constraints.
    #[display(fmt = "Provided Track does not satisfy senders constraints")]
    InvalidMediaTrack,

    /// Occurs when [`MuteState`] of [`Sender`] was dropped.
    #[display(fmt = "MuteState of Sender was dropped.")]
    MuteStateDropped,

    /// Occurs when [`MuteState`] of [`Sender`] transits into opposite to
    /// expected [`MuteState`].
    #[display(fmt = "MuteState of Sender transits into opposite to expected \
                     MuteState")]
    MuteStateTransitsIntoOppositeState,

    /// Invalid [`medea_client_api_proto::TrackPatch`] for
    /// [`MediaStreamTrack`].
    #[display(fmt = "Invalid TrackPatch for Track with {} ID.", _0)]
    InvalidTrackPatch(TrackId),
}

impl From<DroppedError> for MediaConnectionsError {
    #[inline]
    fn from(_: DroppedError) -> Self {
        Self::MuteStateDropped
    }
}

type Result<T> = std::result::Result<T, Traced<MediaConnectionsError>>;

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// [`PeerId`] of peer that owns this [`MediaConnections`].
    peer_id: PeerId,

    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`PeerEvent`]s tx.
    peer_events_sender: mpsc::UnboundedSender<PeerEvent>,

    /// [`TrackId`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`TrackId`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,
}

impl InnerMediaConnections {
    /// Returns [`Iterator`] over [`Sender`]s with provided [`TransceiverKind`].
    pub fn iter_senders_with_kind(
        &self,
        kind: TransceiverKind,
    ) -> impl Iterator<Item = &Rc<Sender>> {
        self.senders.values().filter(move |s| s.kind() == kind)
    }
}

/// Storage of [`RtcPeerConnection`]'s [`Sender`] and [`Receiver`] tracks.
pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    /// Instantiates new [`MediaConnections`] storage for a given
    /// [`RtcPeerConnection`].
    #[inline]
    pub fn new(
        peer_id: PeerId,
        peer: Rc<RtcPeerConnection>,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
    ) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer_id,
            peer,
            peer_events_sender,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }))
    }

    /// Returns all [`Sender`]s from this [`MediaConnections`] with provided
    /// [`TransceiverKind`].
    pub fn get_senders(&self, kind: TransceiverKind) -> Vec<Rc<Sender>> {
        self.0
            .borrow()
            .iter_senders_with_kind(kind)
            .cloned()
            .collect()
    }

    /// Returns `true` if all [`Sender`]s with provided [`TransceiverKind`] is
    /// in provided [`MuteState`].
    pub fn is_all_senders_in_mute_state(
        &self,
        kind: TransceiverKind,
        mute_state: StableMuteState,
    ) -> bool {
        for sender in self.0.borrow().iter_senders_with_kind(kind) {
            if sender.mute_state() != mute_state.into() {
                return false;
            }
        }
        true
    }

    /// Returns `true` if all [`Sender`]s with
    /// [`TransceiverKind::Audio`] are enabled or `false` otherwise.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Audio)
            .find(|s| s.is_muted())
            .is_none()
    }

    /// Returns `true` if all [`Sender`]s with
    /// [`TransceiverKind::Video`] are enabled or `false` otherwise.
    pub fn is_send_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Video)
            .find(|s| s.is_muted())
            .is_none()
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
    /// [mid]:
    /// https://developer.mozilla.org/en-US/docs/Web/API/RTCRtpTransceiver/mid
    pub fn get_mids(&self) -> Result<HashMap<TrackId, Mid>> {
        let mut inner = self.0.borrow_mut();
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
        for (track_id, receiver) in &mut inner.receivers {
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

    /// Creates new [`Sender`]s and [`Receiver`]s for each new [`Track`].
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::TransceiverNotFound`] if could not create
    /// new [`Sender`] cause transceiver with specified `mid` does not
    /// exist.
    pub fn create_tracks<I: IntoIterator<Item = Track>>(
        &self,
        tracks: I,
    ) -> Result<()> {
        let mut inner = self.0.borrow_mut();
        for track in tracks {
            match track.direction {
                Direction::Send { mid, .. } => {
                    let sndr = Sender::new(
                        inner.peer_id,
                        track.id,
                        track.media_type.into(),
                        &inner.peer,
                        inner.peer_events_sender.clone(),
                        mid,
                        track.is_muted.into(),
                    )
                    .map_err(tracerr::wrap!())?;
                    inner.senders.insert(track.id, sndr);
                }
                Direction::Recv { sender, mid } => {
                    let recv = Receiver::new(
                        track.id,
                        &(track.media_type.into()),
                        sender,
                        &inner.peer,
                        mid,
                    );
                    inner.receivers.insert(track.id, recv);
                }
            }
        }
        Ok(())
    }

    /// Updates [`Sender`]s of this [`super::PeerConnection`] with
    /// [`proto::TrackPatch`].
    ///
    /// # Errors
    ///
    /// Errors with [`MediaConnectionsError::InvalidTrackPatch`] if
    /// [`MediaStreamTrack`] with ID from [`proto::TrackPatch`] doesn't exist.
    pub fn update_senders(&self, tracks: Vec<proto::TrackPatch>) -> Result<()> {
        for track_proto in tracks {
            let sender =
                self.get_sender_by_id(track_proto.id).ok_or_else(|| {
                    tracerr::new!(MediaConnectionsError::InvalidTrackPatch(
                        track_proto.id
                    ))
                })?;
            sender.update(&track_proto);
        }
        Ok(())
    }

    pub fn remove_tracks(&self, mids: &HashSet<Mid>) {
        let mut inner = self.0.borrow_mut();

        let mut senders_to_remove = HashSet::new();
        for (track_id, sender) in &inner.senders {
            if let Some(mid) = sender.mid() {
                if mids.contains(&mid) {
                    senders_to_remove.insert(*track_id);
                }
            }
        }

        let mut receivers_to_remove = HashSet::new();
        for (track_id, receiver) in &mut inner.receivers {
            if let Some(mid) = receiver.mid() {
                if mids.contains(&mid) {
                    receivers_to_remove.insert(*track_id);
                }
            }
        }

        for sender_id in senders_to_remove {
            inner.senders.remove(&sender_id);
        }
        for receiver_id in receivers_to_remove {
            inner.receivers.remove(&receiver_id);
        }
    }

    /// Returns [`StreamRequest`] if this [`MediaConnections`] has [`Sender`]s.
    pub fn get_stream_request(&self) -> Option<StreamRequest> {
        let mut stream_request = None;
        for sender in self.0.borrow().senders.values() {
            if let MuteState::Stable(StableMuteState::NotMuted) =
                sender.mute_state.get()
            {
                stream_request
                    .get_or_insert_with(StreamRequest::default)
                    .add_track_request(sender.track_id, sender.caps.clone());
            }
        }
        stream_request
    }

    /// Inserts tracks from a provided [`PeerMediaStream`] into [`Sender`]s
    /// based on track IDs.
    ///
    /// Provided [`PeerMediaStream`] must have all required
    /// [`MediaStreamTrack`]s. [`MediaStreamTrack`]s are inserted into
    /// [`Sender`]'s [`RtcRtpTransceiver`]s via [`replaceTrack` method][1],
    /// changing its direction to `sendonly`.
    ///
    /// # Errors
    ///
    /// With [`MediaConnectionsError::InvalidMediaStream`] if provided
    /// [`PeerMediaStream`] doesn't contain required [`MediaStreamTrack`].
    ///
    /// With [`MediaConnectionsError::InvalidMediaTrack`] if some
    /// [`MediaStreamTrack`] cannot be inserted into associated [`Sender`]
    /// because of constraints mismatch.
    ///
    /// With [`MediaConnectionsError::CouldNotInsertTrack`] if some
    /// [`MediaStreamTrack`] from provided [`PeerMediaStream`] cannot be
    /// inserted into provided [`Sender`]s transceiver.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_stream(
        &self,
        stream: &PeerMediaStream,
    ) -> Result<()> {
        let inner = self.0.borrow();

        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track = Vec::with_capacity(inner.senders.len());
        for sender in inner.senders.values() {
            // skip senders that are not NotMuted
            if !sender.is_not_muted() {
                continue;
            }

            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                if sender.caps.satisfies(&track) {
                    sender_and_track.push((sender, track));
                } else {
                    return Err(tracerr::new!(
                        MediaConnectionsError::InvalidMediaTrack
                    ));
                }
            } else {
                return Err(tracerr::new!(
                    MediaConnectionsError::InvalidMediaStream
                ));
            }
        }

        future::try_join_all(sender_and_track.into_iter().map(
            |(sender, track)| {
                Sender::insert_and_enable_track(Rc::clone(sender), track)
            },
        ))
        .await?;

        Ok(())
    }

    /// Adds provided [`MediaStreamTrack`] and [`RtcRtpTransceiver`] to the
    /// stored [`Receiver`], which is associated with a given
    /// [`RtcRtpTransceiver`].
    ///
    /// Returns ID of associated [`Sender`] with a found [`Receiver`], if any.
    pub fn add_remote_track(
        &self,
        transceiver: RtcRtpTransceiver,
        track: MediaStreamTrack,
    ) -> Option<PeerId> {
        let mut inner = self.0.borrow_mut();
        if let Some(mid) = transceiver.mid() {
            for receiver in &mut inner.receivers.values_mut() {
                if let Some(recv_mid) = &receiver.mid() {
                    if &recv_mid.0 == &mid {
                        receiver.transceiver.replace(transceiver);
                        receiver.track.replace(track);
                        return Some(receiver.sender_id);
                    }
                }
            }
        }
        None
    }

    /// Returns [`MediaStreamTrack`]s being received from a specified sender,
    /// but only if all receiving [`MediaStreamTrack`]s are present already.
    pub fn get_stream_by_sender(
        &self,
        sender_id: PeerId,
    ) -> Option<PeerMediaStream> {
        let inner = self.0.borrow();
        let stream = PeerMediaStream::new();
        for rcv in inner.receivers.values() {
            if rcv.sender_id == sender_id {
                match rcv.track() {
                    None => return None,
                    Some(ref track) => {
                        stream.add_track(rcv.track_id, track.clone());
                    }
                }
            }
        }
        Some(stream)
    }

    /// Returns [`MediaStreamTrack`] by its [`TrackId`] and
    /// [`TransceiverDirection`].
    pub fn get_track_by_id_and_direction(
        &self,
        id: TrackId,
        direction: TransceiverDirection,
    ) -> Option<MediaStreamTrack> {
        let inner = self.0.borrow();
        match direction {
            TransceiverDirection::Sendonly => inner
                .senders
                .get(&id)
                .and_then(|sndr| sndr.track.borrow().clone()),
            TransceiverDirection::Recvonly => inner
                .receivers
                .get(&id)
                .and_then(|recvr| recvr.track.clone()),
        }
    }

    /// Returns [`Sender`] from this [`MediaConnections`] by [`TrackId`].
    #[inline]
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<Sender>> {
        self.0.borrow().senders.get(&id).cloned()
    }

    /// Returns [`MediaStreamTrack`] from this [`MediaConnections`] by its
    /// [`TrackId`].
    pub fn get_track_by_id(&self, id: TrackId) -> Option<MediaStreamTrack> {
        let inner = self.0.borrow();
        inner
            .senders
            .get(&id)
            .and_then(|s| s.track.borrow().clone())
            .or_else(|| {
                inner.receivers.get(&id).and_then(|recv| recv.track.clone())
            })
    }

    /// Stops all [`Sender`]s state transitions expiry timers.
    pub fn stop_state_transitions_timers(&self) {
        self.0
            .borrow()
            .senders
            .values()
            .for_each(|sender| sender.stop_mute_state_transition_timeout());
    }

    /// Resets all [`Sender`]s state transitions expiry timers.
    pub fn reset_state_transitions_timers(&self) {
        self.0
            .borrow()
            .senders
            .values()
            .for_each(|sender| sender.reset_mute_state_transition_timeout());
    }
}

/// Representation of a local [`MediaStreamTrack`] that is being sent to some
/// remote peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    track: RefCell<Option<MediaStreamTrack>>,
    transceiver: RtcRtpTransceiver,
    mute_state: ObservableCell<MuteState>,
    mute_timeout_handle: RefCell<Option<ResettableDelayHandle>>,
}

impl Sender {
    #[cfg(not(feature = "mockable"))]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(10);
    #[cfg(feature = "mockable")]
    const MUTE_TRANSITION_TIMEOUT: Duration = Duration::from_millis(500);

    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise retrieves existing [`RtcRtpTransceiver`] via provided `mid`
    /// from a provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`]
    /// lookup fails.
    fn new(
        peer_id: PeerId,
        track_id: TrackId,
        caps: TrackConstraints,
        peer: &RtcPeerConnection,
        peer_events_sender: mpsc::UnboundedSender<PeerEvent>,
        mid: Option<Mid>,
        mute_state: StableMuteState,
    ) -> Result<Rc<Self>> {
        let kind = TransceiverKind::from(&caps);
        let transceiver = match mid {
            None => peer.add_transceiver(kind, TransceiverDirection::Sendonly),
            Some(mid) => peer
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };

        let mute_state = ObservableCell::new(mute_state.into());
        // we dont care about initial state, cause transceiver is inactive atm
        let mut mute_state_changes = mute_state.subscribe().skip(1);
        let this = Rc::new(Self {
            track_id,
            caps,
            track: RefCell::new(None),
            transceiver,
            mute_state,
            mute_timeout_handle: RefCell::new(None),
        });

        let weak_this = Rc::downgrade(&this);
        spawn_local(async move {
            while let Some(mute_state) = mute_state_changes.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match mute_state {
                        MuteState::Stable(stable) => {
                            match stable {
                                StableMuteState::NotMuted => {
                                    let _ = peer_events_sender.unbounded_send(
                                        PeerEvent::NewLocalStreamRequired {
                                            peer_id,
                                        },
                                    );
                                }
                                StableMuteState::Muted => {
                                    // cannot fail
                                    this.track.borrow_mut().take();
                                    let _ = JsFuture::from(
                                        this.transceiver
                                            .sender()
                                            .replace_track(None),
                                    )
                                    .await;
                                }
                            }
                        }
                        MuteState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions =
                                    this.mute_state.subscribe().skip(1);
                                let (timeout, timeout_handle) =
                                    resettable_delay_for(
                                        Self::MUTE_TRANSITION_TIMEOUT,
                                    );
                                this.mute_timeout_handle
                                    .borrow_mut()
                                    .replace(timeout_handle);
                                match future::select(
                                    transitions.next(),
                                    Box::pin(timeout),
                                )
                                .await
                                {
                                    Either::Left(_) => (),
                                    Either::Right(_) => {
                                        if let Some(this) = weak_this.upgrade()
                                        {
                                            let stable = this
                                                .mute_state
                                                .get()
                                                .cancel_transition();
                                            this.mute_state.set(stable);
                                        }
                                    }
                                }
                            });
                        }
                    }
                } else {
                    break;
                }
            }
        });

        Ok(this)
    }

    /// Stops mute/unmute timeout of this [`Sender`].
    pub fn stop_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.stop();
        }
    }

    /// Resets mute/unmute timeout of this [`Sender`].
    pub fn reset_mute_state_transition_timeout(&self) {
        if let Some(timer) = &*self.mute_timeout_handle.borrow() {
            timer.reset();
        }
    }

    /// Returns [`TrackId`] of this [`Sender`].
    pub fn track_id(&self) -> TrackId {
        self.track_id
    }

    /// Returns kind of [`RtcRtpTransceiver`] this [`Sender`].
    pub fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }

    /// Returns [`MuteState`] of this [`Sender`].
    pub fn mute_state(&self) -> MuteState {
        self.mute_state.get()
    }

    pub fn mid(&self) -> Option<Mid> {
        self.transceiver.mid().map(|mid| mid.into())
    }

    /// Inserts provided [`MediaStreamTrack`] into provided [`Sender`]s
    /// transceiver and enables transceivers sender by changing its
    /// direction to `sendonly`.
    ///
    /// [1]: https://w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    async fn insert_and_enable_track(
        sender: Rc<Self>,
        new_track: MediaStreamTrack,
    ) -> Result<()> {
        // no-op if we try to insert same track
        if let Some(current_track) = sender.track.borrow().as_ref() {
            if new_track.id() == current_track.id() {
                return Ok(());
            }
        }

        // no-op if transceiver is not NotMuted
        if let MuteState::Stable(StableMuteState::NotMuted) =
            sender.mute_state()
        {
            JsFuture::from(
                sender
                    .transceiver
                    .sender()
                    .replace_track(Some(new_track.as_ref())),
            )
            .await
            .map_err(Into::into)
            .map_err(MediaConnectionsError::CouldNotInsertTrack)
            .map_err(tracerr::wrap!())?;

            sender.track.borrow_mut().replace(new_track);

            sender
                .transceiver
                .set_direction(RtcRtpTransceiverDirection::Sendonly);
        }

        Ok(())
    }

    /// Checks whether [`Sender`] is in [`MuteState::Muted`].
    pub fn is_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::Muted.into()
    }

    /// Checks whether [`Sender`] is in [`MuteState::NotMuted`].
    pub fn is_not_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::NotMuted.into()
    }

    /// Sets current [`MuteState`] to [`MuteState::Transition`].
    pub fn mute_state_transition_to(&self, desired_state: StableMuteState) {
        let current_mute_state = self.mute_state.get();
        self.mute_state
            .set(current_mute_state.transition_to(desired_state));
    }

    /// Returns [`Future`] which will be resolved when [`MuteState`] of this
    /// [`Sender`] will be [`MuteState::Stable`] or the [`Sender`] is dropped.
    ///
    /// Succeeds if [`Sender`]'s [`MuteState`] transits into the `desired_state`
    /// or the [`Sender`] is dropped.
    ///
    /// # Errors
    ///
    /// [`MediaConnectionsError::MuteStateTransitsIntoOppositeState`] is
    /// returned if [`Sender`]'s [`MuteState`] transits into the opposite to
    /// the `desired_state`.
    pub fn when_mute_state_stable(
        &self,
        desired_state: StableMuteState,
    ) -> impl Future<Output = Result<()>> {
        let mut mute_states = self.mute_state.subscribe();
        async move {
            while let Some(state) = mute_states.next().await {
                match state {
                    MuteState::Transition(_) => continue,
                    MuteState::Stable(s) => {
                        return if s == desired_state {
                            Ok(())
                        } else {
                            Err(tracerr::new!(
                                MediaConnectionsError::
                                MuteStateTransitsIntoOppositeState
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Updates this [`Sender`]s tracks based on the provided
    /// [`proto::TrackPatch`].
    pub fn update(&self, track: &proto::TrackPatch) {
        if track.id != self.track_id {
            return;
        }

        if let Some(is_muted) = track.is_muted {
            let new_mute_state = StableMuteState::from(is_muted);
            let current_mute_state = self.mute_state.get();

            let mute_state_update: MuteState = match current_mute_state {
                MuteState::Stable(_) => new_mute_state.into(),
                MuteState::Transition(t) => {
                    if t.intended() == new_mute_state {
                        new_mute_state.into()
                    } else {
                        t.set_inner(new_mute_state).into()
                    }
                }
            };

            self.mute_state.set(mute_state_update);
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        console_error("Sender dropped.");
        self.transceiver
            .set_direction(RtcRtpTransceiverDirection::Inactive);
        let fut = JsFuture::from(self.transceiver.sender().replace_track(None));
        spawn_local(async move {
            let _ = fut.await;
        });
    }
}

/// Representation of a remote [`MediaStreamTrack`] that is being received from
/// some remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual
/// [`MediaStreamTrack`] only when [`MediaStreamTrack`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    sender_id: PeerId,
    transceiver: Option<RtcRtpTransceiver>,
    mid: Option<Mid>,
    track: Option<MediaStreamTrack>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be
    /// injected when [`MediaStreamTrack`] arrives.
    ///
    /// `track` field in the created [`Receiver`] will be `None`,
    /// since [`Receiver`] must be created before the actual
    /// [`MediaStreamTrack`] data arrives.
    #[inline]
    fn new(
        track_id: TrackId,
        caps: &TrackConstraints,
        sender_id: PeerId,
        peer: &RtcPeerConnection,
        mid: Option<Mid>,
    ) -> Self {
        let kind = TransceiverKind::from(caps);
        let transceiver = match mid {
            None => {
                Some(peer.add_transceiver(kind, TransceiverDirection::Recvonly))
            }
            Some(_) => None,
        };
        Self {
            track_id,
            sender_id,
            transceiver,
            mid,
            track: None,
        }
    }

    /// Returns associated [`MediaStreamTrack`] with this [`Receiver`], if any.
    #[inline]
    pub(crate) fn track(&self) -> Option<MediaStreamTrack> {
        self.track.as_ref().cloned()
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub(crate) fn mid(&mut self) -> Option<Mid> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid().map(Into::into)
        }
        self.mid.clone()
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        console_error("Receiver dropped.");
        if let Some(transceiver) = &self.transceiver {
            transceiver.set_direction(RtcRtpTransceiverDirection::Inactive);
            // transceiver
            //     .receiver()
            //     .replace_track(None);
        }
    }
}
