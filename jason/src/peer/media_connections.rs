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
    senders: HashMap<TrackId, Sender>,
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
    pub fn get_send_mids(&self) -> Result<HashMap<u64, String>, WasmErr> {
        let mut mids = HashMap::new();
        for (track_id, sender) in &self.senders {
            mids.insert(
                *track_id,
                sender.transceiver.mid().ok_or_else(|| {
                    WasmErr::build_from_str("Peer has senders without mid")
                })?,
            );
        }

        Ok(mids)
    }

    // TODO: Doesnt really updates anything, but only generates new senders and
    //       receivers atm.
    pub fn update_track(&mut self, track: Track) {
        match track.direction {
            Direction::Send { .. } => {
                self.need_new_stream = true;
                self.senders.insert(
                    track.id,
                    Sender::new(track.id, track.media_type, &self.peer),
                );
            }
            Direction::Recv { sender, mid } => {
                self.receivers.insert(
                    track.id,
                    Receiver::new(track.id, track.media_type, sender, mid),
                );
            }
        }
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

    /// Inserts tracks from provided [`MediaStream`] into stored [`Sender`]s
    /// based on track ids. Stream must have all required tracks.
    pub fn insert_local_stream(
        &mut self,
        stream: &Rc<MediaStream>,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // validate that provided stream have all tracks that we need
        for sender in self.senders.values() {
            if !stream.has_track(sender.track_id) {
                return future::Either::A(future::err(
                    WasmErr::build_from_str(
                        "Stream does not have all necessary tracks",
                    ),
                ));
            }
        }

        let mut promises = Vec::new();
        for sender in self.senders.values() {
            let sender: &Sender = sender;

            if let Some(track) = stream.get_track_by_id(sender.track_id) {
                promises.push(
                    JsFuture::from(
                        sender
                            .transceiver
                            .sender()
                            .replace_track(Some(track.track())),
                    )
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
        // should be safe to unwrap
        let mid = transceiver.mid().unwrap();

        for receiver in &mut self.receivers.values_mut() {
            if let Some(recv_mid) = &receiver.mid {
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

        None
    }

    /// Update this peer [`Receiver`]s mids.
    pub fn set_recv_mids(&mut self, mids: HashMap<u64, String>) {
        for (track_id, mid) in mids {
            if let Some(receiver) = self.receivers.get_mut(&track_id) {
                receiver.mid.replace(mid);
            }
        }
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

/// Local track representation, that is being sent to some remote peer.
pub struct Sender {
    track_id: TrackId,
    transceiver: RtcRtpTransceiver,
    caps: MediaType,
}

impl Sender {
    fn new(
        track_id: TrackId,
        caps: MediaType,
        peer: &Rc<RtcPeerConnection>,
    ) -> Self {
        let transceiver = match caps {
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
        };

        Self {
            track_id,
            transceiver,
            caps,
        }
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
    fn new(
        track_id: TrackId,
        caps: MediaType,
        sender_id: u64,
        mid: Option<String>,
    ) -> Self {
        Self {
            track_id,
            caps,
            sender_id,
            transceiver: None,
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
}
