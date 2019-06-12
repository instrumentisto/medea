mod peer;
mod stream;
mod stream_request;
mod track;

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use futures::{
    future::{self, Either},
    Future,
};

use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as BackingMediaStream;

use crate::media::stream_request::{SimpleStreamRequest, StreamRequest};
use crate::utils::{window, Callback2, WasmErr};

pub use self::{
    peer::{
        Id as PeerId, PeerConnection, PeerEvent, PeerEventHandler,
        PeerRepository, Sdp,
    },
    stream::{MediaStream, MediaStreamHandle},
};
use futures::future::IntoFuture;
use wasm_bindgen::JsValue;

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

#[derive(Default)]
struct InnerMediaManager {
    streams: Vec<Rc<MediaStream>>,
    on_local_stream: Rc<Callback2<MediaStreamHandle, WasmErr>>,
}

impl MediaManager {
    pub fn get_stream(
        &self,
        request: StreamRequest,
    ) -> impl Future<Item = Rc<MediaStream>, Error = ()> {
        // TODO: lookup stream by caps, and return its copy if found

        let inner: Rc<RefCell<InnerMediaManager>> = Rc::clone(&self.0);
        self.inner_get_stream(request)
            .then(move |result| match result {
                Ok(stream) => {
                    let stream = Rc::new(stream);
                    inner.borrow_mut().streams.push(Rc::clone(&stream));
                    inner.borrow().on_local_stream.call1(stream.new_handle());
                    Ok(stream)
                }
                Err(err) => {
                    inner.borrow().on_local_stream.call2(err);
                    Err(())
                }
            })
    }

    pub fn on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }

    fn inner_get_stream(
        &self,
        caps: StreamRequest,
    ) -> impl Future<Item = MediaStream, Error = WasmErr> {
        let request = match SimpleStreamRequest::try_from(caps) {
            Ok(request) => request,
            Err(err) => return Either::A(future::err(err)),
        };

        let constraints = web_sys::MediaStreamConstraints::from(&request);
        Either::B(
            window()
                .navigator()
                .media_devices()
                .map_err(WasmErr::from)
                .into_future()
                .and_then(move |devices| {
                    devices
                        .get_user_media_with_constraints(&constraints)
                        .map_err(WasmErr::from)
                })
                .and_then(|promise: js_sys::Promise| {
                    JsFuture::from(promise).map_err(WasmErr::from)
                })
                .and_then(move |stream| {
                    request.parse_stream(BackingMediaStream::from(stream))
                }),
        )
    }
}
