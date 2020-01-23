//! Media tracks.

use std::{
    borrow::ToOwned, cell::RefCell, collections::HashMap, convert::From,
    future::Future, ops::Not, rc::Rc,
};

use derive_more::Display;
use futures::{future, StreamExt};
use medea_client_api_proto as proto;
use medea_client_api_proto::{Direction, PeerId, Track, TrackId};
use medea_reactive::{Dropped, Reactive};
use tracerr::Traced;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    MediaStreamTrack, RtcRtpTransceiver, RtcRtpTransceiverDirection,
};

use crate::{
    media::TrackConstraints,
    utils::{JsCaused, JsError},
};

use super::{
    conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
    stream::MediaStream,
    stream_request::StreamRequest,
    track::MediaTrack,
};

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

    /// [`MutedState`] of [`MediaTrack`] was dropped.
    #[display(fmt = "'MutedState' of 'MediaTrack' was dropped.")]
    MutedStateDropped,

    /// No [`MediaTrack`] in [`Sender`].
    #[display(fmt = "No 'MediaTrack' in 'Sender'.")]
    NoTrack,
}

impl From<Dropped> for MediaConnectionsError {
    fn from(_: Dropped) -> Self {
        Self::MutedStateDropped
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
    /* TODO: few fileds were deleted, they were used to mute tracks that
     *       were added after mute_room call how do you handle this now? */
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
        }))
    }

    /// Returns all [`Sender`]s with provided [`TransceiverKind`] and
    /// [`MutedState`] from this [`MediaConnections`].
    pub fn get_senders_by_kind_and_mute_state(
        &self,
        kind: TransceiverKind,
        mute_state: MuteState,
    ) -> Vec<Rc<Sender>> {
        self.0
            .borrow()
            .iter_senders_with_kind(kind)
            .filter(|sender| sender.muted_state() == mute_state)
            .cloned()
            .collect()
    }

    /// Returns `true` if all [`Sender`]s with provided [`TransceiverKind`] is
    /// in [`MutedState::Muted`].
    pub fn is_all_tracks_with_kind_muted(&self, kind: TransceiverKind) -> bool {
        for sender in self.0.borrow().iter_senders_with_kind(kind) {
            if sender.muted_state() != MuteState::Muted {
                return false;
            }
        }
        true
    }

    /// Returns `true` if all [`Sender`]s with provided [`TransceiverKind`] is
    /// in [`MutedState::Unmuted`].
    pub fn is_all_tracks_with_kind_unmuted(
        &self,
        kind: TransceiverKind,
    ) -> bool {
        for sender in self.0.borrow().iter_senders_with_kind(kind) {
            if sender.muted_state() != MuteState::NotMuted {
                return false;
            }
        }
        true
    }

    /// Returns `true` if all [`MediaTrack`]s of all [`Senders`] with
    /// [`TransceiverKind::Audio`] are enabled or `false` otherwise.
    pub fn is_send_audio_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Audio)
            .skip_while(|s| s.is_track_enabled())
            .next()
            .is_none()
    }

    /// Returns `true` if all [`MediaTrack`]s of all [`Senders`] with
    /// [`TransceiverKind::Video`] are enabled or `false` otherwise.
    pub fn is_send_video_enabled(&self) -> bool {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Video)
            .skip_while(|s| s.is_track_enabled())
            .next()
            .is_none()
    }

    /// Returns mapping from a [`MediaTrack`] ID to a `mid` of
    /// this track's [`RtcRtpTransceiver`].
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
    /// Returns [`StreamRequest`] in case a new local [`MediaStream`]
    /// is required.
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
        for sender in s.senders.values() {
            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                if sender.caps.satisfies(&track.track()) {
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

        future::try_join_all(
            sender_and_track
                .into_iter()
                .map(|(s, t)| Sender::insert_and_enable_track(Rc::clone(s), t)),
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
            for receiver in &mut s.receivers.values_mut() {
                if let Some(recv_mid) = &receiver.mid() {
                    if recv_mid == &mid {
                        let track = MediaTrack::new(
                            receiver.track_id,
                            track,
                            receiver.caps.clone(),
                        );
                        receiver.transceiver.replace(transceiver);
                        receiver.track.replace(track);
                        return Some(receiver.sender_id);
                    }
                }
            }
        }
        None
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
    pub fn get_sender_by_id(&self, id: TrackId) -> Option<Rc<Sender>> {
        self.0.borrow().senders.get(&id).cloned()
    }

    /// Returns [`MediaTrack`] by its [`TrackId`].
    pub fn get_track_by_id(&self, id: TrackId) -> Option<Rc<MediaTrack>> {
        let inner = self.0.borrow();

        inner
            .senders
            .get(&id)
            .and_then(|sender| sender.track.borrow().clone())
            .or_else(|| {
                inner.receivers.get(&id).and_then(|recv| recv.track.clone())
            })
    }
}

/// Mute state of [`Sender`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MuteState {
    /// [`Sender`] is not muted.
    NotMuted,

    /// [`Sender`] should be unmuted, but awaits server permission.
    Unmuting,

    /// [`Sender`] should be muted, but awaits server permission.
    Muting,

    /// [`Sender`] is muted.
    Muted,
}

impl MuteState {
    /// Returns [`MutedState`] which should be set while transition to this
    /// [`MutedState`].
    pub fn proccessing_state(self) -> Self {
        match self {
            Self::NotMuted => Self::Unmuting,
            Self::Muted => Self::Muting,
            _ => self,
        }
    }
}

impl From<bool> for MuteState {
    fn from(is_muted: bool) -> Self {
        if is_muted {
            Self::Muted
        } else {
            Self::NotMuted
        }
    }
}

impl Not for MuteState {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Muted => Self::NotMuted,
            Self::NotMuted => Self::Muted,
            Self::Unmuting => Self::Muting,
            Self::Muting => Self::Unmuting,
        }
    }
}

/// Representation of a local [`MediaTrack`] that is being sent to some remote
/// peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    track: RefCell<Option<Rc<MediaTrack>>>,
    transceiver: RtcRtpTransceiver,
    muted_state: RefCell<Reactive<MuteState>>,
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
        muted_state: MuteState,
    ) -> Result<Rc<Self>> {
        let kind = TransceiverKind::from(&caps);
        let transceiver = match mid {
            None => peer.add_transceiver(kind, TransceiverDirection::Sendonly),
            Some(mid) => peer
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };

        let muted_state = RefCell::new(Reactive::new(muted_state));
        let mut subscription = muted_state.borrow().subscribe();
        let this = Rc::new(Self {
            track_id,
            caps,
            track: RefCell::new(None),
            transceiver,
            muted_state,
        });
        let weak_this = Rc::downgrade(&this);
        spawn_local(async move {
            while let Some(mute_state_update) = subscription.next().await {
                if let Some(this) = weak_this.upgrade() {
                    if let Some(track) = this.track.borrow().as_ref() {
                        track.set_enabled_by_muted_state(mute_state_update);
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
    fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
    }

    /// Returns [`MutedState`] of underlying [`MediaTrack`] of this [`Sender`].
    pub fn muted_state(&self) -> MuteState {
        **self.muted_state.borrow()
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
        track.set_enabled_by_muted_state(sender.muted_state());
        sender.track.borrow_mut().replace(track);

        Ok(())
    }

    /// Changes [`MutedState`] of this [`Sender`]'s underlying [`MediaTrack`].
    pub fn change_muted_state(&self, new_state: MuteState) {
        *self.muted_state.borrow_mut().borrow_mut() = new_state;
    }

    /// Checks that [`Sender`] has a track, and it's unmuted.
    fn is_track_enabled(&self) -> bool {
        **self.muted_state.borrow() == MuteState::NotMuted
    }

    /// Resolves when [`MutedState`] of underlying [`MediaTrack`] of this
    /// [`Sender`] will become equal to provided [`MutedState`].
    pub fn on_muted_state(
        &self,
        state: MuteState,
    ) -> impl Future<Output = Result<()>> {
        let subscription = self.muted_state.borrow().when_eq(state);
        async move {
            subscription.await.map_err(|_| {
                tracerr::new!(MediaConnectionsError::MutedStateDropped)
            })
        }
    }

    /// Updates this [`Track`] based on provided
    /// [`medea_client_api_proto::TrackUpdate`].
    pub fn update(&self, track: &proto::TrackUpdate) {
        if let Some(is_muted) = track.is_muted {
            if is_muted {
                *self.muted_state.borrow_mut().borrow_mut() = MuteState::Muted;
            } else {
                *self.muted_state.borrow_mut().borrow_mut() =
                    MuteState::NotMuted;
            }
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
        self.mid.as_ref().map(String::as_str)
    }
}
