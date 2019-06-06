//! External Jason API.
mod peer;
mod stream;

use std::{cell::RefCell, rc::Rc};

use futures::{
    future::{self, Either},
    Future,
};

use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as BackingMediaStream;

use crate::utils::{window, Callback, WasmErr};

pub use self::{
    peer::{
        Id as PeerId, PeerConnection, PeerEvent, PeerEventHandler,
        PeerRepository, Sdp,
    },
    stream::{GetMediaRequest, MediaStream, MediaStreamHandle},
};

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

#[derive(Default)]
struct InnerMediaManager {
    streams: Vec<Rc<MediaStream>>,
    on_local_stream: Rc<Callback<MediaStreamHandle, WasmErr>>,
}

impl MediaManager {
    pub fn get_stream(
        &self,
        request: &GetMediaRequest,
    ) -> impl Future<Item = Rc<MediaStream>, Error = ()> {
        // TODO: lookup stream by caps, return its copy if found

        let stream = match self.inner_get_stream(request) {
            Ok(promise) => JsFuture::from(promise).map_err(WasmErr::from),
            Err(err) => {
                self.0.borrow().on_local_stream.call_err(err);
                return Either::A(future::err(()));
            }
        };

        let inner: Rc<RefCell<InnerMediaManager>> = Rc::clone(&self.0);
        let fut = stream.then(move |res| match res {
            Ok(stream) => {
                let stream =
                    MediaStream::from_stream(BackingMediaStream::from(stream));
                inner.borrow_mut().streams.push(Rc::clone(&stream));
                inner.borrow().on_local_stream.call_ok(stream.new_handle());
                Ok(stream)
            }
            Err(err) => {
                inner.borrow().on_local_stream.call_err(err);
                Err(())
            }
        });

        Either::B(fut)
    }

    pub fn on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }

    fn inner_get_stream(
        &self,
        caps: &GetMediaRequest,
    ) -> Result<js_sys::Promise, WasmErr> {
        window()
            .navigator()
            .media_devices()?
            .get_user_media_with_constraints(&caps.into())
            .map_err(WasmErr::from)
    }
}
