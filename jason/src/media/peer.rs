use std::collections::HashMap;
use std::rc::Rc;
use web_sys::{
    RtcIceCandidateInit, RtcOfferOptions, RtcPeerConnection, RtcSdpType,
    RtcSessionDescription, RtcSessionDescriptionInit,
};

use crate::media::stream::MediaStream;
use crate::utils::WasmErr;
use futures::Future;
use protocol::IceCandidate;
use wasm_bindgen_futures::JsFuture;

pub type Id = u64;

#[allow(clippy::module_name_repetitions)]
pub struct PeerConnection {
    inner: Rc<RtcPeerConnection>,
}

impl PeerConnection {
    pub fn set_remote_answer(
        &self,
        answer: &str,
    ) -> impl Future<Item = (), Error = WasmErr> {
        let mut desc = RtcSessionDescriptionInit::new(RtcSdpType::Answer);
        desc.sdp(&answer);

        JsFuture::from(self.inner.set_remote_description(&desc))
            .map(|_| ())
            .map_err(Into::into)
    }

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

    pub fn add_ice_candidate(
        &self,
        candidate: &IceCandidate,
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

    //    pub fn set_onaddstream(&self, onaddstream: Option<&Function>)
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
