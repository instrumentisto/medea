//! Media tracks.

use std::{borrow::ToOwned, cell::RefCell, collections::HashMap, rc::Rc};

use futures::{future, Future};
use medea_client_api_proto::{Direction, MediaType, Track};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    MediaStreamTrack, RtcRtpTransceiver, RtcRtpTransceiverDirection,
};

use crate::{
    media::{MediaStream, MediaTrack, StreamRequest, TrackId},
    utils::WasmErr,
};

use super::conn::{RtcPeerConnection, TransceiverDirection, TransceiverKind};

/// Actual data of [`MediaConnections`] storage.
struct InnerMediaConnections {
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`MediaTrack`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`MediaTrack`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,

    /// Are senders audio tracks muted.
    enabled_audio: bool,

    /// Are senders video tracks muted.
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
            .filter(|sender| sender.kind == kind)
            .for_each(|sender| sender.set_track_enabled(enabled))
    }

    /// Returns true if all [`MediaTrack`]s of all [`Senders`] with given
    /// [`TransceiverKind`] are enabled or false otherwise.
    pub fn are_senders_enabled(&self, kind: TransceiverKind) -> bool {
        let conn = self.0.borrow();
        for sender in conn.senders.values().filter(|sender| sender.kind == kind)
        {
            if !sender.is_track_enabled() {
                return false;
            }
        }

        true
    }

    /// Returns mapping from a [`MediaTrack`] ID to a `mid` of
    /// this track's [`RtcRtpTransceiver`].
    pub fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        let mut s = self.0.borrow_mut();
        let mut mids =
            HashMap::with_capacity(s.senders.len() + s.receivers.len());
        for (track_id, sender) in &s.senders {
            mids.insert(
                *track_id,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::from("Peer has senders without mid")
                })?,
            );
        }
        for (track_id, receiver) in &mut s.receivers {
            mids.insert(
                *track_id,
                receiver.mid().map(ToOwned::to_owned).ok_or_else(|| {
                    WasmErr::from("Peer has receivers without mid")
                })?,
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
    ) -> Result<Option<StreamRequest>, WasmErr> {
        let mut s = self.0.borrow_mut();
        let mut stream_request = None;
        for track in tracks {
            match track.direction {
                Direction::Send { mid, .. } => {
                    stream_request
                        .get_or_insert_with(StreamRequest::default)
                        .add_track_request(track.id, track.media_type.clone());
                    let sndr =
                        Sender::new(track.id, &track.media_type, &s.peer, mid)?;
                    s.senders.insert(track.id, sndr);
                }
                Direction::Recv { sender, mid } => {
                    let recv = Receiver::new(
                        track.id,
                        track.media_type,
                        sender,
                        &s.peer,
                        mid,
                    );
                    s.receivers.insert(track.id, recv);
                }
            }
        }
        Ok(stream_request)
    }

    /// Inserts tracks from a provided [`MediaStream`] into [`Sender`]s
    /// basing on track IDs.
    ///
    /// Enables or disables tracks in provided stream based on current media
    /// connections state
    ///
    /// Provided [`MediaStream`] must have all required [`MediaTrack`]s.
    /// [`MediaTrack`]s are inserted into [`Sender`]'s [`RtcRtpTransceiver`]s
    /// via [`replaceTrack` method][1], so changing [`RtcRtpTransceiver`]
    /// direction to `sendonly`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub fn insert_local_stream(
        &self,
        stream: &MediaStream,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let s = self.0.borrow();

        // Check that provided stream have all tracks that we need.
        for sender in s.senders.values() {
            if !stream.has_track(sender.track_id) {
                return future::Either::A(future::err(WasmErr::from(
                    "Stream does not have all necessary tracks",
                )));
            }
        }

        stream.toggle_audio_tracks(s.enabled_audio);
        stream.toggle_video_tracks(s.enabled_video);

        let promises = s.senders.values().filter_map(|sndr| {
            match stream.get_track_by_id(sndr.track_id) {
                Some(track) => Some(Sender::insert_and_enable_track(
                    Rc::clone(sndr),
                    track,
                )),
                None => None,
            }
        });
        let promises: Vec<_> = promises.collect();
        future::Either::B(future::join_all(promises).map(|_| ()))
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
    ) -> Option<u64> {
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
        sender_id: u64,
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

    /// Returns track by its id and direction.
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
    track: RefCell<Option<Rc<MediaTrack>>>,
    transceiver: RtcRtpTransceiver,
    kind: TransceiverKind,
}

impl Sender {
    /// Creates new [`RtcRtpTransceiver`] if provided `mid` is `None`,
    /// otherwise retrieves existing [`RtcRtpTransceiver`] via provided `mid`
    /// from a provided [`RtcPeerConnection`]. Errors if [`RtcRtpTransceiver`]
    /// lookup fails.
    fn new(
        track_id: TrackId,
        caps: &MediaType,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Result<Rc<Self>, WasmErr> {
        let kind = match caps {
            MediaType::Audio(_) => TransceiverKind::Audio,
            MediaType::Video(_) => TransceiverKind::Video,
        };
        let transceiver = match mid {
            None => peer.add_transceiver(kind, TransceiverDirection::Sendonly),
            Some(mid) => {
                peer.get_transceiver_by_mid(&mid).ok_or_else(|| {
                    WasmErr::from(format!(
                        "Unable to find transceiver with provided mid {}",
                        mid
                    ))
                })?
            }
        };
        Ok(Rc::new(Self {
            track_id,
            track: RefCell::new(None),
            transceiver,
            kind,
        }))
    }

    /// Inserts provided [`MediaTrack`] into provided [`Sender`]s transceiver
    /// and enables transceivers sender by changing its direction to sendonly;
    fn insert_and_enable_track(
        sender: Rc<Self>,
        track: Rc<MediaTrack>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        JsFuture::from(
            sender
                .transceiver
                .sender()
                .replace_track(Some(track.track())),
        )
        .and_then(move |_| {
            // TODO: also do RTCRtpSender.setStreams when its
            //       implemented
            sender
                .transceiver
                .set_direction(RtcRtpTransceiverDirection::Sendonly);
            sender.track.borrow_mut().replace(track);
            Ok(())
        })
        .map_err(WasmErr::from)
    }

    /// Enable or disable this [`Sender`]s track.
    fn set_track_enabled(&self, enabled: bool) {
        if let Some(track) = self.track.borrow().as_ref() {
            track.set_enabled(enabled);
        }
    }

    /// Is sender has track and it is enabled.
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
    caps: MediaType,
    sender_id: u64,
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
        caps: MediaType,
        sender_id: u64,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Self {
        let transceiver = match mid {
            None => match caps {
                MediaType::Audio(_) => Some(peer.add_transceiver(
                    TransceiverKind::Audio,
                    TransceiverDirection::Recvonly,
                )),
                MediaType::Video(_) => Some(peer.add_transceiver(
                    TransceiverKind::Video,
                    TransceiverDirection::Recvonly,
                )),
            },
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
