//! Acquiring and storing [MediaStream]s.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::{cell::RefCell, convert::TryFrom, rc::Rc};

use futures::{
    future::{self, Either, IntoFuture as _},
    Future,
};
use wasm_bindgen_futures::JsFuture;
use web_sys::MediaStream as SysMediaStream;

use crate::utils::{window, Callback2, WasmErr};

use super::{
    MediaStream, MediaStreamHandle, SimpleStreamRequest, StreamRequest,
};

/// Manager that is responsible for [`MediaStream`] acquisition and storing.
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained streams.
    streams: Vec<Rc<MediaStream>>,

    /// Callback to be invoked when new [`MediaStream`] is acquired providing
    /// its handle.
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_local_stream: Callback2<MediaStreamHandle, WasmErr>,
}

impl MediaManager {
    /// Obtain [`MediaStream`] basing on a provided [`StreamRequest`].
    /// Acquired streams are cached and cloning existing stream is preferable
    /// over obtaining new ones.
    ///
    /// `on_local_stream` callback will be invoked each time this function
    /// succeeds.
    // TODO: lookup stream by caps, and return its copy if found
    pub(crate) fn get_stream(
        &self,
        caps: StreamRequest,
    ) -> impl Future<Item = Rc<MediaStream>, Error = WasmErr> {
        let request = match SimpleStreamRequest::try_from(caps) {
            Ok(request) => request,
            Err(err) => return Either::A(future::err(err)),
        };

        let mngr: Rc<RefCell<InnerMediaManager>> = Rc::clone(&self.0);
        let constraints = web_sys::MediaStreamConstraints::from(&request);
        let fut = window()
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
            .then(move |result: Result<MediaStream, WasmErr>| match result {
                Ok(stream) => {
                    let stream = Rc::new(stream);
                    mngr.borrow_mut().streams.push(Rc::clone(&stream));
                    mngr.borrow().on_local_stream.call1(stream.new_handle());
                    Ok(stream)
                }
                Err(err) => {
                    mngr.borrow().on_local_stream.call2(err.clone());
                    Err(err)
                }
            });
        Either::B(fut)
    }

    /// Sets `on_local_stream` callback that will be invoked when
    /// [`MediaManager`] obtains [`MediaStream`].
    #[inline]
    pub(crate) fn set_on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }
}
