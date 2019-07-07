use std::{collections::HashMap, rc::Rc};

use futures::{
    future::{self, join_all},
    Future,
};
use medea_client_api_proto::{Direction, MediaType, Track};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    RtcPeerConnection, RtcRtpTransceiver, RtcRtpTransceiverDirection,
    RtcRtpTransceiverInit,
};

use crate::{
    media::{MediaStream, MediaTrack, StreamRequest, TrackId},
    utils::WasmErr,
};

/// Stores [`Peer`]s [`Sender`]s and [`Receiver`]s.
pub struct MediaConnections {
    peer: Rc<RtcPeerConnection>,
    need_new_stream: bool,
    senders: HashMap<TrackId, Rc<Sender>>,
    receivers: HashMap<TrackId, Receiver>,
}

impl MediaConnections {
    pub fn new(peer: Rc<RtcPeerConnection>) -> Self {
        Self {
            peer,
            need_new_stream: false,
            senders: HashMap::new(),
            receivers: HashMap::new(),
        }
    }

    /// Returns map of track id to corresponding transceiver mid.
    pub fn get_mids(&mut self) -> Result<HashMap<u64, String>, WasmErr> {
        let mut mids = HashMap::new();
        for (track_id, sender) in &self.senders {
            mids.insert(
                *track_id,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::from("Peer has senders without mid")
                })?,
            );
        }

        for (track_id, receiver) in self.receivers.iter_mut() {
            mids.insert(
                *track_id,
                receiver.mid().map(|mid| mid.to_owned()).ok_or_else(|| {
                    WasmErr::from("Peer has receivers without mid")
                })?,
            );
        }

        Ok(mids)
    }

    // TODO: Doesnt really updates anything, but only generates new senders and
    //       receivers atm.
    pub fn update_tracks(&mut self, tracks: Vec<Track>) -> Result<(), WasmErr> {
        for track in tracks {
            match track.direction {
                Direction::Send { mid, .. } => {
                    self.need_new_stream = true;

                    self.senders.insert(
                        track.id,
                        Sender::new(
                            track.id,
                            track.media_type,
                            &self.peer,
                            mid,
                        )?,
                    );
                }
                Direction::Recv { sender, mid } => {
                    self.receivers.insert(
                        track.id,
                        Receiver::new(
                            track.id,
                            track.media_type,
                            sender,
                            &self.peer,
                            mid,
                        ),
                    );
                }
            }
        }
        Ok(())
    }

    /// Check if [`Sender`]s require new [`MediaStream`].
    pub fn get_request(&self) -> Option<StreamRequest> {
        if self.need_new_stream {
            let mut media_request = StreamRequest::default();
            for (track_id, sender) in &self.senders {
                media_request.add_track_request(*track_id, sender.caps.clone());
            }
            Some(media_request)
        } else {
            None
        }
    }

    /// Inserts tracks from provided [`MediaStream`] into [`Sender`]s
    /// based on track ids. Provided [`MediaStream`] must have all required
    /// [`MediaTrack`]s. [`MediaTrack`]s are inserted in [`Senders`]
    /// [`RTCRtpTransceiver`][1]s with [replaceTrack][2]
    /// changing transceivers direction to sendonly.
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    /// [2]: https://www.w3.org/TR/webrtc/#dom-rtcrtpsender-replacetrack
    pub fn insert_local_stream(
        &mut self,
        stream: &MediaStream,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // validate that provided stream have all tracks that we need
        for sender in self.senders.values() {
            if !stream.has_track(sender.track_id) {
                return future::Either::A(future::err(WasmErr::from(
                    "Stream does not have all necessary tracks",
                )));
            }
        }

        let mut promises = Vec::new();
        for sender in self.senders.values() {
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
    /// [`StreamTrack`] and [`RtcRtpTransceiver`][1] and return found
    /// [`Receiver`].
    ///
    /// [1]: https://www.w3.org/TR/webrtc/#rtcrtptransceiver-interface
    pub fn add_remote_track(
        &mut self,
        transceiver: RtcRtpTransceiver,
        track: web_sys::MediaStreamTrack,
    ) -> Option<&Receiver> {
        if let Some(mid) = transceiver.mid() {
            for receiver in &mut self.receivers.values_mut() {
                if let Some(recv_mid) = &receiver.mid() {
                    if recv_mid == &mid {
                        let track = MediaTrack::new(
                            receiver.track_id,
                            track,
                            receiver.caps.clone(),
                        );

                        receiver.transceiver.replace(transceiver);
                        receiver.track.replace(track);
                        return Some(receiver);
                    }
                }
            }
        }

        None
    }

    /// Returns [`Receiver`]s that share provided sender id.
    pub fn get_by_sender(
        &mut self,
        sender_id: u64,
    ) -> impl Iterator<Item = &mut Receiver> {
        self.receivers.iter_mut().filter_map(move |(_, receiver)| {
            if receiver.sender_id == sender_id {
                Some(receiver)
            } else {
                None
            }
        })
    }
}

/// Find transceiver in peer transceivers by provided mid.
// TODO: create wrapper for RtcPeerConnection
fn get_transceiver_by_mid(
    peer: &RtcPeerConnection,
    mid: &str,
) -> Option<RtcRtpTransceiver> {
    let mut transceiver = None;

    let transceivers =
        js_sys::try_iter(&peer.get_transceivers()).unwrap().unwrap();
    for tr in transceivers {
        let tr: RtcRtpTransceiver = RtcRtpTransceiver::from(tr.unwrap());
        if let Some(tr_mid) = tr.mid() {
            if mid.eq(&tr_mid) {
                transceiver = Some(tr);
                break;
            }
        }
    }

    transceiver
}

/// Local track representation, that is being sent to some remote peer.
pub struct Sender {
    track_id: TrackId,
    transceiver: RtcRtpTransceiver,
    caps: MediaType,
}

impl Sender {
    /// Creates new transceiver if mid is None, or retrieves existing
    /// transceiver by provided mid. Errors if transceiver lookup fails.
    fn new(
        track_id: TrackId,
        caps: MediaType,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Result<Rc<Self>, WasmErr> {
        let transceiver = match mid {
            None => match caps {
                MediaType::Audio(_) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Sendonly);
                    peer.add_transceiver_with_str_and_init("audio", &init)
                }
                MediaType::Video(_) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Sendonly);
                    peer.add_transceiver_with_str_and_init("video", &init)
                }
            },
            Some(mid) => {
                get_transceiver_by_mid(&peer, &mid).ok_or_else(|| {
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
            caps,
        }))
    }
}

/// Remote track representation that is being received from some remote peer.
/// Basically, it can have two states: new and established. When track arrives
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
    /// Creates new transceiver if mid is None, or retrieves existing
    /// transceiver by provided mid. Errors if transceiver lookup fails. Track
    /// in created receiver is None, since receiver must be created before
    /// actual track arrives.
    fn new(
        track_id: TrackId,
        caps: MediaType,
        sender_id: u64,
        peer: &RtcPeerConnection,
        mid: Option<String>,
    ) -> Self {
        let transceiver = match mid {
            None => match caps {
                MediaType::Audio(_) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Recvonly);
                    Some(peer.add_transceiver_with_str_and_init("audio", &init))
                }
                MediaType::Video(_) => {
                    let mut init = RtcRtpTransceiverInit::new();
                    init.direction(RtcRtpTransceiverDirection::Recvonly);
                    Some(peer.add_transceiver_with_str_and_init("video", &init))
                }
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

    pub fn sender_id(&self) -> u64 {
        self.sender_id
    }

    pub fn track(&self) -> Option<&Rc<MediaTrack>> {
        self.track.as_ref()
    }

    pub fn mid(&mut self) -> Option<&str> {
        if self.mid.is_none() && self.transceiver.is_some() {
            self.mid = self.transceiver.as_ref().unwrap().mid()
        }

        self.mid.as_ref().map(|mid| mid.as_str())
    }
}
