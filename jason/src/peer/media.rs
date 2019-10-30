//! Media tracks.

use std::{borrow::ToOwned, cell::RefCell, collections::HashMap, rc::Rc};

use futures::future;
use medea_client_api_proto::{Direction, PeerId, Track, TrackId};
use thiserror::*;
use tracerr::Traced;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack, RtcRtpTransceiver, RtcRtpTransceiverDirection,
};

use crate::{media::TrackConstraints, utils::WasmErr};

use super::{
    conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind},
    stream::MediaStream,
    stream_request::StreamRequest,
    track::MediaTrack,
};

/// Describes errors that may occur in [`MediaConnections`] storage.
#[derive(Error, Debug)]
pub enum Error {
    #[error("failed insert track to sender {0}")]
    InsertTrack(WasmErr),
    #[error("unable to find transceiver with provided mid {0}")]
    NotFoundTransceiver(String),
    #[error("peer has senders without mid")]
    SendersWithoutMid,
    #[error("peer has receivers without mid")]
    ReceiversWithoutMid,
    #[error("provided stream does not have all necessary tracks")]
    InvalidMediaStream,
    #[error("provided track does not satisfy senders constraints")]
    InvalidMediaTrack,
}

type Result<T, E = Error> = std::result::Result<T, Traced<E>>;

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`MediaTrack`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`MediaTrack`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,

    /// Are senders audio tracks muted or not.
    enabled_audio: bool,

    /// Are senders video tracks muted or not.
    enabled_video: bool,
}

/// Storage of [`RtcPeerConnection`]'s [`Sender`] and [`Receiver`] tracks.
#[allow(clippy::module_name_repetitions)]
pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    /// Instantiates new [`MediaConnections`] storage for a given
    /// [`RtcPeerConnection`].
    #[inline]
    pub fn new(
        peer: Rc<RtcPeerConnection>,
        enabled_audio: bool,
        enabled_video: bool,
    ) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer,
            senders: HashMap::new(),
            receivers: HashMap::new(),
            enabled_audio,
            enabled_video,
        }))
    }

    /// Enables or disables all [`Sender`]s with specified [`TransceiverKind`]
    /// [`MediaTrack`]s.
    pub fn toggle_send_media(&self, kind: TransceiverKind, enabled: bool) {
        let mut s = self.0.borrow_mut();
        match kind {
            TransceiverKind::Audio => s.enabled_audio = enabled,
            TransceiverKind::Video => s.enabled_video = enabled,
        };
        s.senders
            .values()
            .filter(|sender| sender.kind() == kind)
            .for_each(|sender| sender.set_track_enabled(enabled))
    }

    /// Returns `true` if all [`MediaTrack`]s of all [`Senders`] with given
    /// [`TransceiverKind`] are enabled or `false` otherwise.
    pub fn are_senders_enabled(&self, kind: TransceiverKind) -> bool {
        let conn = self.0.borrow();
        for sender in
            conn.senders.values().filter(|sender| sender.kind() == kind)
        {
            if !sender.is_track_enabled() {
                return false;
            }
        }
        true
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
                    .ok_or(Error::SendersWithoutMid)
                    .map_err(tracerr::wrap!())?,
            );
        }
        for (track_id, receiver) in &mut s.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .map(ToOwned::to_owned)
                    .ok_or(Error::ReceiversWithoutMid)
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
    /// basing on track IDs.
    ///
    /// Enables or disables tracks in provided [`MediaStream`] basing on current
    /// media connections state.
    ///
    /// Provided [`MediaStream`] must have all required [`MediaTrack`]s.
    /// [`MediaTrack`]s are inserted into [`Sender`]'s [`RtcRtpTransceiver`]s
    /// via [`replaceTrack` method][1], so changing [`RtcRtpTransceiver`]
    /// direction to `sendonly`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub async fn insert_local_stream(
        &self,
        stream: &MediaStream,
    ) -> Result<()> {
        let s = self.0.borrow();

        // Build sender to track pairs to catch errors before inserting.
        let mut sender_and_track = Vec::new();
        for sender in s.senders.values() {
            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                if sender.caps.satisfies(&track.track()) {
                    sender_and_track.push((sender, track));
                } else {
                    return Err(tracerr::new!(Error::InvalidMediaTrack));
                }
            } else {
                return Err(tracerr::new!(Error::InvalidMediaStream));
            }
        }

        stream.toggle_audio_tracks(s.enabled_audio);
        stream.toggle_video_tracks(s.enabled_video);

        let mut futures = Vec::new();
        for (sender, track) in sender_and_track {
            futures
                .push(Sender::insert_and_enable_track(Rc::clone(sender), track))
        }
        for res in future::join_all(futures).await {
            res.map_err(tracerr::wrap!())?;
        }

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
                .ok_or(Error::NotFoundTransceiver(mid))
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
        .map_err(Error::InsertTrack)
        .map_err(tracerr::wrap!())?;

        sender
            .transceiver
            .set_direction(RtcRtpTransceiverDirection::Sendonly);
        sender.track.borrow_mut().replace(track);

        Ok(())
    }

    /// Enables or disables this [`Sender`]s track.
    fn set_track_enabled(&self, enabled: bool) {
        if let Some(track) = self.track.borrow().as_ref() {
            track.set_enabled(enabled);
        }
    }

    /// Checks is sender has track and it is enabled.
    fn is_track_enabled(&self) -> bool {
        match self.track.borrow().as_ref() {
            None => false,
            Some(track) => track.is_enabled(),
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
