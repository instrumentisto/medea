use std::collections::HashMap;
use std::rc::Rc;
use web_sys::{
    RtcIceCandidate, RtcIceCandidateInit, RtcOfferOptions, RtcPeerConnection,
    RtcPeerConnectionIceEvent, RtcSdpType, RtcSessionDescription,
    RtcSessionDescriptionInit,
};

use crate::media::stream::MediaStream;
use crate::utils::{EventListener, WasmErr};
use futures::Future;
use protocol::{
    Direction, IceCandidate as IceDTO, MediaType, Track as TrackDTO,
};
use std::cell::RefCell;
use wasm_bindgen_futures::JsFuture;

pub type Id = u64;

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    inner: Rc<RtcPeerConnection>,
    on_ice_candidate: RefCell<
        Option<EventListener<RtcPeerConnection, RtcPeerConnectionIceEvent>>,
    >,
    recv_audio: RefCell<bool>,
    recv_video: RefCell<bool>,
}

pub enum Sdp {
    Offer(String),
    Answer(String),
}

impl PeerConnection {
    pub fn create_and_set_offer(
        &self,
        receive_audio: bool,
        receive_video: bool,
        ice_restart: bool,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let mut opts = RtcOfferOptions::new();
        opts.offer_to_receive_audio(receive_audio)
            .offer_to_receive_video(receive_video)
            .ice_restart(ice_restart);

        let inner = Rc::clone(&self.inner);
        JsFuture::from(self.inner.create_offer_with_rtc_offer_options(&opts))
            .map(RtcSessionDescription::from)
            .and_then(move |offer: RtcSessionDescription| {
                let offer = offer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);
                JsFuture::from(inner.set_local_description(&desc))
                    .map(move |_| offer)
            })
            .map_err(Into::into)
    }

    pub fn create_and_set_answer(
        &self,
    ) -> impl Future<Item = String, Error = WasmErr> {
        let inner = Rc::clone(&self.inner);
        JsFuture::from(self.inner.create_answer())
            .map(RtcSessionDescription::from)
            .and_then(move |answer: RtcSessionDescription| {
                let answer = answer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                JsFuture::from(inner.set_local_description(&desc))
                    .map(move |_| answer)
            })
            .map_err(Into::into)
    }

    pub fn set_remote_description(
        &self,
        sdp: Sdp,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let desc = match sdp {
            Sdp::Offer(offer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Offer);
                desc.sdp(&offer);
                desc
            }
            Sdp::Answer(answer) => {
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                desc
            }
        };

        JsFuture::from(self.inner.set_remote_description(&desc))
            .map(|_| ())
            .map_err(Into::into)
    }

    pub fn add_ice_candidate(
        &self,
        candidate: &IceDTO,
    ) -> impl Future<Item = (), Error = WasmErr> {
        // TODO: According to Web IDL, return value is void.
        //       It may be worth to propose PR to wasm-bindgen, that would
        //       transform JsValue::Void to ().
        let mut cand_init = RtcIceCandidateInit::new(&candidate.candidate);
        cand_init
            .sdp_m_line_index(candidate.sdp_m_line_index)
            .sdp_mid(candidate.sdp_mid.as_ref().map(String::as_ref));
        JsFuture::from(
            self.inner
                .add_ice_candidate_with_opt_rtc_ice_candidate_init(
                    Some(cand_init).as_ref(),
                ),
        )
        .map(|_| ())
        .map_err(Into::into)
    }

    pub fn add_stream(&self, stream: &Rc<MediaStream>) {
        self.inner.add_stream(&stream.get_media_stream());
    }

    pub fn on_remote_stream(&self) {}

    pub fn on_ice_candidate<F>(&self, mut f: F) -> Result<(), WasmErr>
    where
        F: (FnMut(IceDTO)) + 'static,
    {
        self.on_ice_candidate
            .borrow_mut()
            .replace(EventListener::new_mut(
                Rc::clone(&self.inner),
                "icecandidate",
                move |msg: RtcPeerConnectionIceEvent| {
                    let candidate = msg.candidate().unwrap();

                    let candidate = IceDTO {
                        candidate: candidate.candidate(),
                        sdp_m_line_index: candidate.sdp_m_line_index(),
                        sdp_mid: candidate.sdp_mid(),
                    };

                    f(candidate);
                },
            )?);

        Ok(())
    }

//    // TODO: properly cast and store all tracks/streams in PeerConnection with
//    // states (pending, active)
//    pub fn apply_tracks(&self, tracks: Vec<TrackDTO>) -> Result<(), WasmErr> {
//        let mut send_audio = false;
//        let mut send_video = false;
//        let mut recv_audio = false;
//        let mut recv_video = false;
//
//        for track in tracks.into_iter() {
//            let track = (track.direction, track.media_type);
//            match track {
//                (Direction::Recv { .. }, MediaType::Audio { .. }) => {
//                    recv_audio = true;
//                }
//                (Direction::Recv { .. }, MediaType::Video { .. }) => {
//                    recv_video = true;
//                }
//                (Direction::Send { .. }, MediaType::Audio { .. }) => {
//                    send_audio = true;
//                }
//                (Direction::Send { .. }, MediaType::Video { .. }) => {
//                    send_video = true;
//                }
//                _ => Err(WasmErr::from_str(""))?,
//            };
//        }
//
//        *self.recv_audio.borrow_mut() = recv_audio;
//        *self.recv_video.borrow_mut() = send_video;
//
//        Ok(())
//    }
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        self.inner.close()
    }
}

impl From<RtcPeerConnection> for PeerConnection {
    fn from(peer: RtcPeerConnection) -> Self {
        Self {
            inner: Rc::new(peer),
            on_ice_candidate: RefCell::new(None),
            recv_audio: RefCell::new(false),
            recv_video: RefCell::new(false),
        }
    }
}

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    peers: HashMap<Id, PeerConnection>,
}

impl PeerRepository {
    // TODO: set ice_servers
    pub fn create(&mut self, id: Id) -> Result<&PeerConnection, WasmErr> {
        let peer = PeerConnection::from(RtcPeerConnection::new()?);
        self.peers.insert(id, peer);
        Ok(self.peers.get(&id).unwrap())
    }

    pub fn get_peer(&self, id: Id) -> Option<&PeerConnection> {
        self.peers.get(&id)
    }

    pub fn remove(&mut self, id: Id) {
        self.peers.remove(&id);
    }
}
