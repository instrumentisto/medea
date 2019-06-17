//! Acquires and stores [`MediaStream`]s.

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use futures::{
    future::{self, Either, IntoFuture as _},
    Future,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as SysMediaStream;

use crate::{
    media::{
        MediaStream, MediaStreamHandle, SimpleStreamRequest, StreamRequest,
    },
    utils::{window, Callback2, WasmErr},
};

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

#[derive(Default)]
struct InnerMediaManager {
    // Obtained streams.
    streams: Vec<Rc<MediaStream>>,

    // Callback to be invoked when new [`MediaStream`] was acquired.
    on_local_stream: Rc<Callback2<MediaStreamHandle, WasmErr>>,
}

impl MediaManager {
    // TODO: lookup stream by caps, and return its copy if found
    pub fn get_stream(
        &self,
        caps: StreamRequest,
    ) -> impl Future<Item = Rc<MediaStream>, Error = WasmErr> {
        let request = match SimpleStreamRequest::try_from(caps) {
            Ok(request) => request,
            Err(err) => return Either::A(future::err(err)),
        };

        let inner: Rc<RefCell<InnerMediaManager>> = Rc::clone(&self.0);
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
                    request.parse_stream(&SysMediaStream::from(stream))
                })
                .then(
                    move |result: Result<MediaStream, WasmErr>| match result {
                        Ok(stream) => {
                            let stream = Rc::new(stream);
                            inner.borrow_mut().streams.push(Rc::clone(&stream));
                            inner
                                .borrow()
                                .on_local_stream
                                .call1(stream.new_handle());
                            Ok(stream)
                        }
                        Err(err) => {
                            inner.borrow().on_local_stream.call2(err.clone());
                            Err(err)
                        }
                    },
                ),
        )
    }

    pub fn set_on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }
}
