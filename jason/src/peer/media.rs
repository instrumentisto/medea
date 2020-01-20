//! Media tracks.

use std::{borrow::ToOwned, cell::RefCell, collections::HashMap, rc::Rc};

use derive_more::{Display, From};
use futures::future;
use medea_client_api_proto::{Direction, PeerId, Track, TrackId};
use tracerr::Traced;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack, RtcRtpTransceiver, RtcRtpTransceiverDirection,
};

use crate::{
    media::TrackConstraints,
    peer::track::MutedState,
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
}

type Result<T> = std::result::Result<T, Traced<MediaConnectionsError>>;

/// Indicator of audio being switched on or off.
#[derive(Clone, Copy, Debug, Display, Eq, From, PartialEq)]
pub struct EnabledAudio(pub bool);

/// Indicator of video being switched on or off.
#[derive(Clone, Copy, Debug, Display, Eq, From, PartialEq)]
pub struct EnabledVideo(pub bool);

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`MediaTrack`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`MediaTrack`] to its [`Receiver`].
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
    pub fn new(peer: Rc<RtcPeerConnection>) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }))
    }

    /// Enables or disables all [`Sender`]s with [`TransceiverKind::Audio`].
    pub fn change_audio_muted_state(&self, new_state: MutedState) {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Audio)
            .for_each(|s| s.change_muted_state(new_state));
    }

    /// Enables or disables all [`Sender`]s with [`TransceiverKind::Video`].
    pub fn change_video_muted_state(&self, new_state: MutedState) {
        self.0
            .borrow()
            .iter_senders_with_kind(TransceiverKind::Video)
            .for_each(|s| s.change_muted_state(new_state));
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

        // TODO: maybe toggle muted state here???

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
                            MutedState::Unmuted,
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
    pub fn get_track_by_id(
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
}

/// Representation of a local [`MediaTrack`] that is being sent to some remote
/// peer.
pub struct Sender {
    track_id: TrackId,
    caps: TrackConstraints,
    track: RefCell<Option<Rc<MediaTrack>>>,
    transceiver: RtcRtpTransceiver,
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
    ) -> Result<Rc<Self>> {
        let kind = TransceiverKind::from(&caps);
        let transceiver = match mid {
            None => peer.add_transceiver(kind, TransceiverDirection::Sendonly),
            Some(mid) => peer
                .get_transceiver_by_mid(&mid)
                .ok_or(MediaConnectionsError::TransceiverNotFound(mid))
                .map_err(tracerr::wrap!())?,
        };
        Ok(Rc::new(Self {
            track_id,
            caps,
            track: RefCell::new(None),
            transceiver,
        }))
    }

    /// Returns kind of [`RtcRtpTransceiver`] this [`Sender`].
    fn kind(&self) -> TransceiverKind {
        TransceiverKind::from(&self.caps)
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
        sender.track.borrow_mut().replace(track);

        Ok(())
    }

    /// Enables or disables this [`Sender`]s track.
    fn change_muted_state(&self, new_state: MutedState) {
        if let Some(track) = self.track.borrow_mut().as_mut() {
            track.change_muted_state(new_state);
        }
    }

    /// Checks is sender has track and it is enabled.
    fn is_track_enabled(&self) -> bool {
        match self.track.borrow().as_ref() {
            None => false,
            Some(track) => MutedState::Unmuted == track.get_muted_state(),
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
