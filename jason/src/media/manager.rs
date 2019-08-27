//! Acquiring and storing [MediaStream]s.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    convert::TryFrom,
    rc::{Rc, Weak},
};

use futures::{
    future::{self, Either, IntoFuture as _},
    Future,
};
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{MediaDeviceKind, MediaStream as SysMediaStream};

use crate::utils::{window, Callback2, WasmErr};

use super::{
    MediaDeviceInfo, MediaStream, MediaStreamHandle, SimpleStreamRequest,
    StreamRequest,
};

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

impl InnerMediaManager {
    /// Returns the vector of [`MediaDeviceInfo`] objects.
    fn enumerate_devices(
        &self,
    ) -> impl Future<Item = Vec<MediaDeviceInfo>, Error = WasmErr> {
        window()
            .navigator()
            .media_devices()
            .into_future()
            .and_then(|devices| devices.enumerate_devices())
            .and_then(JsFuture::from)
            .and_then(|infos| {
                Ok(js_sys::Array::from(&infos)
                    .values()
                    .into_iter()
                    .filter_map(|value| {
                        let info =
                            web_sys::MediaDeviceInfo::from(value.unwrap());
                        match info.kind() {
                            MediaDeviceKind::Audioinput
                            | MediaDeviceKind::Videoinput => {
                                Some(MediaDeviceInfo::from(info))
                            }
                            _ => None,
                        }
                    })
                    .collect())
            })
            .map_err(WasmErr::from)
    }
}

/// Manager that is responsible for [`MediaStream`] acquisition and storing.
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

impl MediaManager {
    /// Obtain [`MediaStream`] basing on a provided [`StreamRequest`].
    /// Acquired streams are cached and cloning existing stream is preferable
    /// over obtaining new ones.
    ///
    /// `on_local_stream` callback will be invoked each time this function
    /// succeeds.
    // TODO: lookup stream by caps, and return its copy if found
    pub fn get_stream(
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
    pub fn set_on_local_stream(&self, f: js_sys::Function) {
        self.0.borrow_mut().on_local_stream.set_func(f);
    }

    /// Instantiates new [`MediaManagerHandle`] for use on JS side.
    #[inline]
    pub fn new_handle(&self) -> MediaManagerHandle {
        MediaManagerHandle(Rc::downgrade(&self.0))
    }
}

/// JS side handle to [`MediaManager`].
///
/// Actually, represents a [`Weak`]-based handle to [`InnerMediaManager`].
///
/// For using [`MediaManagerHandle`] on Rust side,
/// consider the [`MediaManager`].
#[wasm_bindgen]
pub struct MediaManagerHandle(Weak<RefCell<InnerMediaManager>>);

#[wasm_bindgen]
impl MediaManagerHandle {
    /// Returns the JS array of [`MediaDeviceInfo`] objects.
    pub fn enumerate_devices(&self) -> Promise {
        let fut =
            match map_weak!(self, |inner| inner.borrow().enumerate_devices()) {
                Ok(fut) => Either::A(
                    fut.and_then(|infos| {
                        Ok(infos
                            .into_iter()
                            .fold(js_sys::Array::new(), |devices_info, info| {
                                devices_info.push(&JsValue::from(info));
                                devices_info
                            })
                            .into())
                    })
                    .map_err(JsValue::from),
                ),
                Err(e) => Either::B(future::err(e)),
            };
        future_to_promise(fut)
    }

    /// Returns [`SysMediaStream`] object.
    pub fn init_local_stream(
        &self,
        constraints: web_sys::MediaStreamConstraints,
    ) -> Promise {
        let fut = window()
            .navigator()
            .media_devices()
            .into_future()
            .and_then(move |devices| {
                devices.get_user_media_with_constraints(&constraints)
            })
            .and_then(JsFuture::from);
        future_to_promise(fut)
    }
}
