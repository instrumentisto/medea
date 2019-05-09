use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use web_sys::{
    RtcOfferOptions, RtcPeerConnection, RtcSdpType, RtcSessionDescription,
    RtcSessionDescriptionInit,
};

use crate::utils::WasmErr;
use futures::Future;
use wasm_bindgen_futures::JsFuture;
use futures::future::ok;
use crate::media::stream::MediaStream;

type Id = u64;

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
            .map_err(|e| e.into())
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
                JsFuture::from(inner.set_local_description(&desc)).map(move |_|offer)
            })
            .map_err(|e| e.into())
    }

    pub fn create_and_set_answer(&self) -> impl Future<Item=String, Error=WasmErr> {
        let inner = Rc::clone(&self.inner);
        JsFuture::from(self.inner.create_answer())
            .map(RtcSessionDescription::from)
            .and_then(move |answer: RtcSessionDescription| {
                let answer = answer.sdp();
                let mut desc =
                    RtcSessionDescriptionInit::new(RtcSdpType::Answer);
                desc.sdp(&answer);
                JsFuture::from(inner.set_local_description(&desc)).map(move |_| answer)
            })
            .map_err(|e| e.into())
    }

    pub fn add_ice_candidate(&self) {

    }

    pub fn add_stream(&self, stream: Rc<MediaStream>) {
        self.inner.add_stream(&stream.get_media_stream());
    }

    pub fn on_remote_stream(&self) {

    }
//    pub fn set_onaddstream(&self, onaddstream: Option<&Function>)
}

impl Drop for PeerConnection {
    fn drop(&mut self) {
        self.inner.close()
    }
}

impl From<RtcPeerConnection> for PeerConnection {
    fn from(peer: RtcPeerConnection) -> Self {
        PeerConnection {
            inner: Rc::new(peer),
        }
    }
}

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct PeerRepository {
    peers: RefCell<HashMap<Id, Rc<PeerConnection>>>,
}

impl PeerRepository {
    // TODO: set ice_servers
    pub fn create(&self, id: Id) -> Result<Rc<PeerConnection>, WasmErr> {
        let peer = PeerConnection::from(RtcPeerConnection::new()?);

        let peer = Rc::new(peer);
        self.peers.borrow_mut().insert(id, Rc::clone(&peer));
        Ok(peer)
    }
}
