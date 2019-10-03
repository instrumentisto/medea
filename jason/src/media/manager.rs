//! Acquiring and storing [MediaStream][1]s.
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
use web_sys::{
    MediaStream as SysMediaStream,
    MediaStreamConstraints as SysMediaStreamConstraints, MediaStreamTrack,
};

use crate::{
    media::MediaStreamConstraints,
    utils::{window, Callback2, WasmErr},
};

use super::{
    InputDeviceInfo, MediaStream, MediaStreamHandle, SimpleStreamRequest,
    StreamRequest,
};
use wasm_bindgen::JsValue;

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained tracks storage
    tracks: Rc<RefCell<Vec<MediaStreamTrack>>>,

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
    ) -> impl Future<Item = Vec<InputDeviceInfo>, Error = WasmErr> {
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
                    .filter_map(|info| {
                        let info =
                            web_sys::MediaDeviceInfo::from(info.unwrap());
                        InputDeviceInfo::try_from(info).ok()
                    })
                    .collect())
            })
            .map_err(WasmErr::from)
    }

    /// Returns [MediaStream][1] and is this stream new, meaning that it was
    /// obtained via new [`getUserMediaCall`] call or was build from already
    /// owned tracks.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    fn get_stream(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Item = (SysMediaStream, bool), Error = WasmErr> {
        if let Some(stream) = self.get_from_storage(&caps) {
            future::Either::A(future::ok((stream, false)))
        } else {
            future::Either::B(
                self.get_user_media(caps).map(|stream| (stream, true)),
            )
        }
    }

    /// Tries to build new [MediaStream][1] from already owned tracks to avoid
    /// useless getUserMedia requests.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    fn get_from_storage(
        &self,
        caps: &MediaStreamConstraints,
    ) -> Option<SysMediaStream> {
        let storage = self.tracks.borrow();

        caps.satisfies_tracks(&storage).map(|tracks| {
            let stream = SysMediaStream::new().unwrap();
            for track in tracks {
                stream.add_track(track);
            }
            stream
        })
    }

    /// Obtain new [MediaStream][1] and save its tracks to storage.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    fn get_user_media(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Item = SysMediaStream, Error = WasmErr> {
        let storage = Rc::clone(&self.tracks);
        window()
            .navigator()
            .media_devices()
            .map_err(WasmErr::from)
            .into_future()
            .and_then(move |devices| {
                let caps: SysMediaStreamConstraints = caps.into();

                devices
                    .get_user_media_with_constraints(&caps)
                    .map_err(WasmErr::from)
            })
            .and_then(|promise: js_sys::Promise| {
                JsFuture::from(promise).map_err(WasmErr::from)
            })
            .map(SysMediaStream::from)
            .and_then(move |stream| {
                let mut storage = storage.borrow_mut();

                js_sys::try_iter(&stream.get_tracks())
                    .unwrap()
                    .unwrap()
                    .map(|tr| web_sys::MediaStreamTrack::from(tr.unwrap()))
                    .for_each(|track| storage.push(track));

                Ok(stream)
            })
    }
}

/// Manager that is responsible for [`MediaStream`] acquisition and storing.
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MediaManager(Rc<RefCell<InnerMediaManager>>);

// TODO: add tests
impl MediaManager {
    /// Obtain [MediaStream][1] basing on a provided [`StreamRequest`].
    /// Acquired streams are cached and cloning existing stream is preferable
    /// over obtaining new ones.
    ///
    /// `on_local_stream` callback will be invoked each time new stream was
    /// obtained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub fn get_stream_by_request(
        &self,
        caps: StreamRequest,
    ) -> impl Future<Item = Rc<MediaStream>, Error = WasmErr> {
        let caps = match SimpleStreamRequest::try_from(caps) {
            Ok(request) => request,
            Err(err) => return Either::A(future::err(err)),
        };

        let inner: Rc<RefCell<InnerMediaManager>> = Rc::clone(&self.0);
        let fut = self
            .0
            .borrow()
            .get_stream((&caps).into())
            .and_then(move |(stream, is_new_stream)| {
                caps.parse_stream(&stream)
                    .map(|stream| (stream, is_new_stream))
            })
            .then(move |result: Result<(MediaStream, bool), WasmErr>| {
                match result {
                    Ok((stream, is_new_stream)) => {
                        let stream = Rc::new(stream);
                        if is_new_stream {
                            inner
                                .borrow()
                                .on_local_stream
                                .call1(stream.new_handle());
                        }
                        Ok(stream)
                    }
                    Err(err) => {
                        inner.borrow().on_local_stream.call2(err.clone());
                        Err(err)
                    }
                }
            });

        Either::B(fut)
    }

    /// Obtain [MediaStream][1] basing on a provided [`MediaStreamConstraints`].
    /// Either builds new stream from already known tracks or initiates new user
    /// media request saving returned tracks.
    pub fn get_stream_by_constraints(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Item = SysMediaStream, Error = WasmErr> {
        self.0.borrow().get_stream(caps).map(|(stream, _)| stream)
    }

    /// Sets `on_local_stream` callback that will be invoked when
    /// [`MediaManager`] obtains new [`MediaStream`].
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

    /// Returns [`MediaStream`] object.
    pub fn init_local_stream(&self, caps: MediaStreamConstraints) -> Promise {
        match map_weak!(self, |inner| { inner.borrow().get_stream(caps) }) {
            Ok(ok) => future_to_promise(
                ok.map(|(stream, _)| stream.into())
                    .map_err(|err| err.into()),
            ),
            Err(err) => future_to_promise(future::err(err)),
        }
    }
}
