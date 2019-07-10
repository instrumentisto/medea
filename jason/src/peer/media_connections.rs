//! [`MediaConnections`] is storage of [`RtcPeerConnection`]s [`Sender`]s and
//! [`Receiver`]s, where [`Sender`] is mapping between some local [`MediaTrack`]
//! being sent to remote peer and [`RtcRtpTransceiver`][1] used to send this
//! track and [`Receiver`] being mapping between some [`MediaTrack`] received
//! from remote peer and [`RtcRtpTransceiver`][1] used to receive this track.
//!
//! [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface

use std::{collections::HashMap, rc::Rc};

use std::cell::RefCell;

use futures::{
    future::{self, join_all},
    Future,
};
use medea_client_api_proto::{Direction, MediaType, Track};
use wasm_bindgen_futures::JsFuture;
use web_sys::{RtcRtpTransceiver, RtcRtpTransceiverDirection};

use crate::{
    media::{MediaStream, MediaTrack, StreamRequest, TrackId},
    peer::peer_con::{
        RtcPeerConnection, TransceiverDirection, TransceiverType,
    },
    utils::WasmErr,
};

struct InnerMediaConnections {
    /// Ref to parent [`RtcPeerConnection`]. Used to generate transceivers for
    /// [`Sender`]s and [`Receiver`]s.
    peer: Rc<RtcPeerConnection>,

    /// [`MediaTrack`] to its [`Sender`].
    senders: HashMap<TrackId, Rc<Sender>>,

    /// [`MediaTrack`] to its [`Receiver`].
    receivers: HashMap<TrackId, Receiver>,
}

pub struct MediaConnections(RefCell<InnerMediaConnections>);

impl MediaConnections {
    pub fn new(peer: Rc<RtcPeerConnection>) -> Self {
        Self(RefCell::new(InnerMediaConnections {
            peer,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }))
    }

    /// Returns map of [`MediaTrack`] Id to `mid` of [`RtcRtpTransceiver`][1]
    /// used to send/receive this track.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn get_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        let mut inner = self.0.borrow_mut();

        let mut mids = HashMap::new();
        for (track_id, sender) in &inner.senders {
            mids.insert(
                *track_id,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::from("Peer has senders without mid")
                })?,
            );
        }

        for (track_id, receiver) in &mut inner.receivers {
            mids.insert(
                *track_id,
                receiver
                    .mid()
                    .map(std::borrow::ToOwned::to_owned)
                    .ok_or_else(|| {
                        WasmErr::from("Peer has receivers without mid")
                    })?,
            );
        }

        Ok(mids)
    }

    /// Synchronize local state with provided tracks. Will create new
    /// [`Sender`]s and [`Receiver`]s for each new track, will update track (but
    /// doesnt atm) if track is known but its settings has changed. Returns
    /// [`StreamRequest`] in case new local [`MediaStream`] is required.
    // TODO: Doesnt really updates anything, but only generates new senders
    //       and receivers atm.
    pub fn update_tracks(
        &self,
        tracks: Vec<Track>,
    ) -> Result<Option<StreamRequest>, WasmErr> {
        let mut inner = self.0.borrow_mut();
        let mut stream_request = None;

        for track in tracks {
            match track.direction {
                Direction::Send { mid, .. } => {
                    stream_request
                        .get_or_insert_with(StreamRequest::default)
                        .add_track_request(track.id, track.media_type.clone());

                    let sender = Sender::new(
                        track.id,
                        &track.media_type,
                        &inner.peer,
                        mid,
                    )?;

                    inner.senders.insert(track.id, sender);
                }
                Direction::Recv { sender, mid } => {
                    let receiver = Receiver::new(
                        track.id,
                        track.media_type,
                        sender,
                        &inner.peer,
                        mid,
                    );

                    inner.receivers.insert(track.id, receiver);
                }
            }
        }

        Ok(stream_request)
    }

    /// Inserts tracks from provided [`MediaStream`] into [`Sender`]s
    /// based on track ids. Provided [`MediaStream`] must have all required
    /// [`MediaTrack`]s. [`MediaTrack`]s are inserted in [`Senders`]
    /// [`RTCRtpTransceiver`][1]s with [replaceTrack][2]
    /// changing transceivers direction to `sendonly`.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub fn insert_local_stream(
        &self,
        stream: &MediaStream,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let inner = self.0.borrow();

        // validate inner provided stream have all tracks that we need
        for sender in inner.senders.values() {
            if !stream.has_track(sender.track_id) {
                return future::Either::A(future::err(WasmErr::from(
                    "Stream does not have all necessary tracks",
                )));
            }
        }

        let mut promises = Vec::new();
        for sender in inner.senders.values() {
            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                let sender = Rc::clone(sender);
                promises.push(
                    JsFuture::from(
                        sender
                            .transceiver
                            .sender()
                            .replace_track(Some(track.track())),
                    )
                    .and_then(move |_| {
                        // TODO: also do RTCRtpSender.setStreams when its
                        //       implemented
                        sender.transceiver.set_direction(
                            RtcRtpTransceiverDirection::Sendonly,
                        );
                        Ok(())
                    })
                    .map_err(WasmErr::from),
                );
            }
        }

        future::Either::B(join_all(promises).map(|_| ()))
    }

    /// Find associated [`Receiver`] by transceiver's mid and update it with
    /// [`StreamTrack`] and [`RtcRtpTransceiver`][1] and its sender.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn add_remote_track(
        &self,
        transceiver: RtcRtpTransceiver,
        track: web_sys::MediaStreamTrack,
    ) -> Option<u64> {
        let mut inner = self.0.borrow_mut();

        if let Some(mid) = transceiver.mid() {
            for receiver in &mut inner.receivers.values_mut() {
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

    /// Returns [`MediaTrack`]s being received from specified sender only if
    /// already receiving all [`MediaTrack`]s.
    pub fn get_tracks_by_sender(
        &self,
        sender_id: u64,
    ) -> Option<Vec<Rc<MediaTrack>>> {
        let inner = self.0.borrow();

        let mut tracks: Vec<Rc<MediaTrack>> = Vec::new();
        for receiver in inner.receivers.values() {
            if receiver.sender_id == sender_id {
                match receiver.track() {
                    None => return None,
                    Some(ref track) => tracks.push(Rc::clone(track)),
                }
            }
        }

        Some(tracks)
    }
}

/// Local track representation, that is being sent to some remote peer.
pub struct Sender {
    track_id: TrackId,
    transceiver: RtcRtpTransceiver,
}

impl Sender {
    /// Creates new transceiver if mid is `None`, or retrieves existing
    /// transceiver by provided mid. Errors if transceiver lookup fails.
    fn new(
        track_id: TrackId,
        caps: &MediaType,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Result<Rc<Self>, WasmErr> {
        let transceiver = match mid {
            None => match caps {
                MediaType::Audio(_) => peer.add_transceiver(
                    &TransceiverType::Audio,
                    &TransceiverDirection::Sendonly,
                ),
                MediaType::Video(_) => peer.add_transceiver(
                    &TransceiverType::Video,
                    &TransceiverDirection::Sendonly,
                ),
            },
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
            transceiver,
        }))
    }
}

/// Remote track representation that is being received from some remote peer.
/// Basically, it can have two states: waiting and receiving. When track arrives
/// we can save related [`RtcRtpTransceiver`][1] and actual [`MediaTrack`].
///
/// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
pub struct Receiver {
    track_id: TrackId,
    caps: MediaType,
    sender_id: u64,
    transceiver: Option<RtcRtpTransceiver>,
    mid: Option<String>,
    track: Option<Rc<MediaTrack>>,
}

impl Receiver {
    /// Creates new transceiver if provided mid is `None`, or retrieves existing
    /// transceiver by provided mid from provided peer. Errors if transceiver
    /// lookup fails. `track` in created receiver is `None`, since receiver
    /// must be created before actual track arrives,
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
                    &TransceiverType::Audio,
                    &TransceiverDirection::Recvonly,
                )),
                MediaType::Video(_) => Some(peer.add_transceiver(
                    &TransceiverType::Video,
                    &TransceiverDirection::Recvonly,
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

    pub fn track(&self) -> Option<&Rc<MediaTrack>> {
        self.track.as_ref()
    }

    /// Returns [`Receiver`]s. Will try to fetch it from underlying transceiver
    /// if current value is `None`.
    pub fn mid(&mut self) -> Option<&str> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid()
        }

        self.mid.as_ref().map(String::as_str)
    }
}
