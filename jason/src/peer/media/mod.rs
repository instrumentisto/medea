//! [`crate::peer::PeerConnection`] media management.

mod mute_state;

use std::{
    borrow::ToOwned, cell::RefCell, collections::HashMap, convert::From,
    future::Future, rc::Rc, time::Duration,
};

use derive_more::Display;
use futures::{future, future::Either, StreamExt};
use medea_client_api_proto as proto;
use medea_reactive::{DroppedError, ObservableCell};
use proto::{Direction, PeerId, Track, TrackId};
use tracerr::Traced;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    MediaStreamTrack, RtcRtpTransceiver, RtcRtpTransceiverDirection,
};

use crate::{
    media::TrackConstraints,
    utils::{console_error, delay_for, JsCaused, JsError},
};

use super::{
    conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
    stream::MediaStream,
    stream_request::StreamRequest,
    track::MediaTrack,
};

pub use self::mute_state::{MuteState, MuteStateTransition, StableMuteState};

/// Errors that may occur in [`MediaConnections`] storage.
#[derive(Debug, Display, JsCaused)]
pub enum MediaConnectionsError {
    /// Occurs when the provided [`MediaTrack`] cannot be inserted into
    /// provided [`Sender`]s transceiver.
    #[display(fmt = "Failed to insert Track to a sender: {}", _0)]
    CouldNotInsertTrack(JsError),

    /// Occurs when creates new [`Sender`] on not existed into a
    /// [`RtcPeerConnection`] the [`RtcRtpTransceiver`].
    #[display(fmt = "Unable to find Transceiver with provided mid: {}", _0)]
    TransceiverNotFound(String),

    /// Occurs when cannot get the "mid" from the [`Sender`].
    #[display(fmt = "Peer has senders without mid")]
    SendersWithoutMid,

    /// Occurs when cannot get the "mid" from the [`Receiver`].
    #[display(fmt = "Peer has receivers without mid")]
    ReceiversWithoutMid,

    /// Occurs when inserted [`MediaStream`] dont have all necessary
    /// [`MediaTrack`]s.
    #[display(fmt = "Provided stream does not have all necessary Tracks")]
    InvalidMediaStream,

    /// Occurs when [`MediaTrack`] of inserted [`MediaStream`] does not satisfy
    /// [`Sender`] constraints.
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

    /// Invalid [`medea_client_api_proto::TrackPatch`] for [`MediaTrack`].
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
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`MediaTrack`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`MediaTrack`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,

    js_track_id_to_medea_track_id: JsTrackIdToMedeaTrackId,
}

#[derive(Debug)]
struct JsTrackIdToMedeaTrackId {
    senders: HashMap<String, TrackId>,
    receivers: HashMap<String, TrackId>,
}

impl JsTrackIdToMedeaTrackId {
    pub fn new() -> Self {
        Self {
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    pub fn insert_sender(&mut self, sys_id: String, id: TrackId) {
        self.senders.insert(sys_id, id);
    }

    pub fn insert_receiver(&mut self, sys_id: String, id: TrackId) {
        self.receivers.insert(sys_id, id);
    }

    pub fn get_sender(&self, sys_id: &str) -> Option<TrackId> {
        self.senders.get(sys_id).copied()
    }

    pub fn get_receiver(&self, sys_id: &str) -> Option<TrackId> {
        self.receivers.get(sys_id).copied()
    }

    pub fn into_iter(&self) -> impl Iterator<Item = (String, TrackId)> {
        // TODO: temporary
        self.senders
            .clone()
            .into_iter()
            .chain(self.receivers.clone().into_iter())
    }
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
    pub fn new(peer: Rc<RtcPeerConnection>) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer,
            senders: HashMap::new(),
            receivers: HashMap::new(),
            js_track_id_to_medea_track_id: JsTrackIdToMedeaTrackId::new(),
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

    pub fn iter_tracks_ids(&self) -> impl Iterator<Item = (String, TrackId)> {
        self.0.borrow().js_track_id_to_medea_track_id.into_iter()
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

    /// Returns `true` if all [`MediaTrack`]s of all [`Sender`]s with
    /// [`TransceiverKind::Audio`] are enabled or `false` otherwise.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Audio)
            .find(|s| s.is_track_muted())
            .is_none()
    }

    /// Returns `true` if all [`MediaTrack`]s of all [`Sender`]s with
    /// [`TransceiverKind::Video`] are enabled or `false` otherwise.
    pub fn is_send_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Video)
            .find(|s| s.is_track_muted())
            .is_none()
    }

    /// Returns mapping from a [`MediaTrack`] ID to a `mid` of
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
    pub fn get_mids(&self) -> Result<HashMap<TrackId, String>> {
        let mut s = self.0.borrow_mut();
        let mut mids =
            HashMap::with_capacity(s.senders.len() + s.receivers.len());
        for (track_id, sender) in &s.senders {
            mids.insert(
                *track_id,
                sender
                    .transceiver
                    .mid()
                    .ok_or(MediaConnectionsError::SendersWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        for (track_id, receiver) in &mut s.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .map(ToOwned::to_owned)
                    .ok_or(MediaConnectionsError::ReceiversWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        Ok(mids)
    }

    /// Synchronizes local state with provided tracks. Creates new [`Sender`]s
    /// and [`Receiver`]s for each new [`Track`], and updates [`Track`] if
    /// its settings has been changed.
    ///
    /// # Errors
    ///
    /// Errors if creating new [`Sender`] or [`Receiver`] fails.
    // TODO: Doesnt really updates anything, but only generates new senders
    //       and receivers atm.
    pub fn update_tracks<I: IntoIterator<Item = Track>>(
        &self,
        tracks: I,
    ) -> Result<()> {
        let mut s = self.0.borrow_mut();
        for track in tracks {
            match track.direction {
                Direction::Send { mid, .. } => {
                    let sndr = Sender::new(
                        track.id,
                        track.media_type.into(),
                        &s.peer,
                        mid,
                        track.is_muted.into(),
                    )
                    .map_err(tracerr::wrap!())?;
                    s.senders.insert(track.id, sndr);
                }
                Direction::Recv { sender, mid } => {
                    let recv = Receiver::new(
                        track.id,
                        track.media_type.into(),
                        sender,
                        &s.peer,
                        mid,
                    );
                    s.receivers.insert(track.id, recv);
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
    /// [`MediaTrack`] with ID from [`proto::TrackPatch`] doesn't exist.
    pub fn update_senders(&self, tracks: Vec<proto::TrackPatch>) -> Result<()> {
        for track_proto in tracks {
            let track =
                self.get_sender_by_id(track_proto.id).ok_or_else(|| {
                    tracerr::new!(MediaConnectionsError::InvalidTrackPatch(
                        track_proto.id
                    ))
                })?;
            track.update(&track_proto);
        }
        Ok(())
    }

    /// Returns [`StreamRequest`] if this [`MediaConnections`] has [`Sender`]s.
    pub fn get_stream_request(&self) -> Option<StreamRequest> {
        let mut stream_request = None;
        for sender in self.0.borrow().senders.values() {
            stream_request
                .get_or_insert_with(StreamRequest::default)
                .add_track_request(sender.track_id, sender.caps.clone());
        }
        stream_request
    }

    /// Inserts tracks from a provided [`MediaStream`] into [`Sender`]s
    /// based on track IDs.
    ///
    /// Enables or disables tracks in provided [`MediaStream`] based on current
    /// media connections state.
    ///
    /// Provided [`MediaStream`] must have all required [`MediaTrack`]s.
    /// [`MediaTrack`]s are inserted into [`Sender`]'s [`RtcRtpTransceiver`]s
    /// via [`replaceTrack` method][1], changing its
    /// direction to `sendonly`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_stream(
        &self,
        stream: &MediaStream,
    ) -> Result<()> {
        let s = self.0.borrow();

        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track = Vec::with_capacity(s.senders.len());
        let mut tracks_ids = Vec::new();
        for sender in s.senders.values() {
            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                if sender.caps.satisfies(&track.track()) {
                    tracks_ids.push((track.track().id(), sender.track_id));
                    sender_and_track.push((Rc::clone(&sender), track));
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
        drop(s);

        let mut s = self.0.borrow_mut();
        for (sys_id, track_id) in tracks_ids {
            s.js_track_id_to_medea_track_id
                .insert_sender(sys_id, track_id);
        }

        future::try_join_all(
            sender_and_track
                .into_iter()
                .map(|(s, t)| Sender::insert_and_enable_track(s, t)),
        )
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
        let mut s = self.0.borrow_mut();
        if let Some(mid) = transceiver.mid() {
            let insert;
            let sender_id;

            {
                let receiver = s.receivers.values_mut().find(|recv| {
                    recv.mid
                        .as_ref()
                        .filter(|recv_mid| recv_mid == &&mid)
                        .is_some()
                });
                if let Some(receiver) = receiver {
                    insert = Some((track.id(), receiver.track_id));

                    let track = MediaTrack::new(
                        receiver.track_id,
                        track,
                        receiver.caps.clone(),
                    );
                    receiver.transceiver.replace(transceiver);
                    receiver.track.replace(track);

                    sender_id = Some(receiver.sender_id)
                } else {
                    sender_id = None;
                    insert = None;
                }
            }
            if let Some((sys_track_id, track_id)) = insert {
                s.js_track_id_to_medea_track_id
                    .insert_receiver(sys_track_id, track_id);
            }

            console_error(format!(
                "track_ids: {:?}",
                s.js_track_id_to_medea_track_id
            ));

            sender_id
        } else {
            None
        }
    }

    /// Returns [`MediaTrack`]s being received from a specified sender,
    /// but only if all receiving [`MediaTrack`]s are present already.
    pub fn get_tracks_by_sender(
        &self,
        sender_id: PeerId,
    ) -> Option<Vec<Rc<MediaTrack>>> {
        let s = self.0.borrow();
        let mut tracks: Vec<Rc<MediaTrack>> = Vec::new();
        for rcv in s.receivers.values() {
            if rcv.sender_id == sender_id {
                match rcv.track() {
                    None => return None,
                    Some(ref t) => tracks.push(Rc::clone(t)),
                }
            }
        }
        Some(tracks)
    }

    /// Returns [`MediaTrack`] by its [`TrackId`] and [`TransceiverDirection`].
    pub fn get_track_by_id_and_direction(
        &self,
        direction: TransceiverDirection,
        id: TrackId,
    ) -> Option<Rc<MediaTrack>> {
        let inner = self.0.borrow();
        match direction {
            TransceiverDirection::Sendonly => inner
                .senders
                .get(&id)
                .and_then(|sndr| sndr.track.borrow().clone()),
            TransceiverDirection::Recvonly => {
                inner.receivers.get(&id).and_then(|recv| recv.track.clone())
            }
        }
    }

    /// Returns [`Sender`] from this [`MediaConnections`] by [`TrackId`].
    #[inline]
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<Sender>> {
        self.0.borrow().senders.get(&id).cloned()
    }

    /// Returns [`MediaTrack`] from this [`MediaConnections`] by its
    /// [`TrackId`].
    pub fn get_track_by_id(&self, id: TrackId) -> Option<Rc<MediaTrack>> {
        let inner = self.0.borrow();
        inner
            .senders
            .get(&id)
            .and_then(|s| s.track.borrow().clone())
            .or_else(|| {
                inner.receivers.get(&id).and_then(|recv| recv.track.clone())
            })
    }
}

/// Representation of a local [`MediaTrack`] that is being sent to some remote
/// peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    track: RefCell<Option<Rc<MediaTrack>>>,
    transceiver: RtcRtpTransceiver,
    mute_state: ObservableCell<MuteState>,
}

impl Sender {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise retrieves existing [`RtcRtpTransceiver`] via provided `mid`
    /// from a provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`]
    /// lookup fails.
    fn new(
        track_id: TrackId,
        caps: TrackConstraints,
        peer: &RtcPeerConnection,
        mid: Option<String>,
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
        let mut subscription = mute_state.subscribe();
        let this = Rc::new(Self {
            track_id,
            caps,
            track: RefCell::new(None),
            transceiver,
            mute_state,
        });

        // TODO: remove when refactor muting to dropping tracks.
        let weak_this = Rc::downgrade(&this);
        spawn_local(async move {
            while let Some(mute_state_update) = subscription.next().await {
                if let Some(this) = weak_this.upgrade() {
                    match mute_state_update {
                        MuteState::Stable(stable) => {
                            if let Some(track) = this.track.borrow().as_ref() {
                                track.set_enabled_by_mute_state(stable);
                            }
                        }
                        MuteState::Transition(_) => {
                            let weak_this = Rc::downgrade(&this);
                            spawn_local(async move {
                                let mut transitions =
                                    this.mute_state.subscribe().skip(1);
                                let timeout = Box::pin(delay_for(
                                    Duration::from_secs(10).into(),
                                ));
                                match future::select(
                                    transitions.next(),
                                    timeout,
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

    /// Inserts provided [`MediaTrack`] into provided [`Sender`]s transceiver
    /// and enables transceivers sender by changing its direction to `sendonly`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    async fn insert_and_enable_track(
        sender: Rc<Self>,
        track: Rc<MediaTrack>,
    ) -> Result<()> {
        JsFuture::from(
            sender
                .transceiver
                .sender()
                .replace_track(Some(track.track())),
        )
        .await
        .map_err(Into::into)
        .map_err(MediaConnectionsError::CouldNotInsertTrack)
        .map_err(tracerr::wrap!())?;

        sender
            .transceiver
            .set_direction(RtcRtpTransceiverDirection::Sendonly);
        // TODO: this is temporary disabled, until we resolve case of muting
        //       senders, that were added after muting room.
        //       (instrumentisto/medea#85)

        //        let stable_mute_state = match sender.mute_state() {
        //            MuteState::Stable(stable) => stable,
        //            MuteState::Transition(transition) =>
        // transition.into_inner(),        };
        //        track.set_enabled_by_mute_state(stable_mute_state);
        sender.track.borrow_mut().replace(track);

        Ok(())
    }

    /// Checks that [`Sender`] is in [`MuteState::NotMuted`].
    pub fn is_track_muted(&self) -> bool {
        self.mute_state.get() == StableMuteState::Muted.into()
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
                        if s == desired_state {
                            return Ok(());
                        } else {
                            return Err(tracerr::new!(
                                MediaConnectionsError::
                                MuteStateTransitsIntoOppositeState
                            ));
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Updates this [`Track`] basing on the provided [`proto::TrackPatch`].
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

/// Representation of a remote [`MediaTrack`] that is being received from some
/// remote peer. It may have two states: `waiting` and `receiving`.
///
/// We can save related [`RtcRtpTransceiver`] and the actual [`MediaTrack`]
/// only when [`MediaTrack`] data arrives.
pub struct Receiver {
    track_id: TrackId,
    caps: TrackConstraints,
    sender_id: PeerId,
    transceiver: Option<RtcRtpTransceiver>,
    mid: Option<String>,
    track: Option<Rc<MediaTrack>>,
}

impl Receiver {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise creates [`Receiver`] without [`RtcRtpTransceiver`]. It will be
    /// injected when [`MediaTrack`] arrives.
    ///
    /// `track` field in the created [`Receiver`] will be `None`,
    /// since [`Receiver`] must be created before the actual [`MediaTrack`]
    /// data arrives.
    #[inline]
    fn new(
        track_id: TrackId,
        caps: TrackConstraints,
        sender_id: PeerId,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Self {
        let kind = TransceiverKind::from(&caps);
        let transceiver = match mid {
            None => {
                Some(peer.add_transceiver(kind, TransceiverDirection::Recvonly))
            }
            Some(_) => None,
        };
        Self {
            track_id,
            caps,
            sender_id,
            transceiver,
            mid,
            track: None,
        }
    }

    /// Returns associated [`MediaTrack`] with this [`Receiver`], if any.
    #[inline]
    pub(crate) fn track(&self) -> Option<&Rc<MediaTrack>> {
        self.track.as_ref()
    }

    /// Returns `mid` of this [`Receiver`].
    ///
    /// Tries to fetch it from the underlying [`RtcRtpTransceiver`] if current
    /// value is `None`.
    pub(crate) fn mid(&mut self) -> Option<&str> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid()
        }
        self.mid.as_deref()
    }
}
