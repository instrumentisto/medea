//! Acquiring and storing [MediaStream][1]s.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    convert::TryFrom,
    future::Future,
    rc::{Rc, Weak},
};

use futures::{future, FutureExt as _, TryFutureExt as _};
use js_sys::Promise;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    MediaStream as SysMediaStream,
    MediaStreamConstraints as SysMediaStreamConstraints, MediaStreamTrack,
};

use crate::{
    media::MediaStreamConstraints,
    utils::{copy_js_ref, window, Callback2, WasmErr},
};

use super::InputDeviceInfo;

/// Manager that is responsible for [MediaStream][1] acquisition and storing.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
#[allow(clippy::module_name_repetitions)]
#[derive(Default)]
pub struct MediaManager(Rc<InnerMediaManager>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained tracks storage
    tracks: Rc<RefCell<Vec<MediaStreamTrack>>>,

    /// Callback to be invoked when new [`MediaStream`] is acquired providing
    /// its handle.
    // TODO: will be extended with some metadata that would allow client to
    //       understand purpose of obtaining this stream.
    on_local_stream: Callback2<SysMediaStream, WasmErr>,
}

impl InnerMediaManager {
    /// Returns the vector of [`MediaDeviceInfo`] objects.
    fn enumerate_devices(
        &self,
    ) -> impl Future<Output = Result<Vec<InputDeviceInfo>, WasmErr>> {
        async {
            let devices = window().navigator().media_devices()?;
            let devices = JsFuture::from(devices.enumerate_devices()?).await?;

            Ok(js_sys::Array::from(&devices)
                .values()
                .into_iter()
                .filter_map(|info| {
                    let info = web_sys::MediaDeviceInfo::from(info.unwrap());
                    InputDeviceInfo::try_from(info).ok()
                })
                .collect())
        }
    }

    /// Returns [MediaStream][1] and information if this stream is new one,
    /// meaning that it was obtained via new [getUserMedia()][2] call or was
    /// build from already owned tracks.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://bit.ly/2MHnEj5
    fn get_stream(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Output = Result<(SysMediaStream, bool), WasmErr>> {
        if let Some(stream) = self.get_from_storage(&caps) {
            future::ok((stream, false)).left_future()
        } else {
            self.get_user_media(caps)
                .and_then(|stream| future::ok((stream, true)))
                .right_future()
        }
    }

    /// Tries to build new [MediaStream][1] from already owned tracks to avoid
    /// redudant [getUserMedia()][2] requests.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://bit.ly/2MHnEj5
    fn get_from_storage(
        &self,
        caps: &MediaStreamConstraints,
    ) -> Option<SysMediaStream> {
        let mut tracks = Vec::new();
        let storage = self.tracks.borrow();

        if let Some(audio) = caps.get_audio() {
            let track = storage.iter().find(|track| audio.satisfies(track));

            if let Some(track) = track {
                tracks.push(track);
            } else {
                return None;
            }
        }

        if let Some(video) = caps.get_video() {
            let track = storage.iter().find(|track| video.satisfies(track));

            if let Some(track) = track {
                tracks.push(track);
            } else {
                return None;
            }
        }

        let stream = SysMediaStream::new().unwrap();
        for track in tracks {
            stream.add_track(track);
        }

        Some(stream)
    }

    /// Obtains new [MediaStream][1] and save its tracks to storage.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    fn get_user_media(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Output = Result<SysMediaStream, WasmErr>> {
        let storage = Rc::clone(&self.tracks);

        async move {
            let media_devices = window().navigator().media_devices()?;

            let caps: SysMediaStreamConstraints = caps.into();
            let stream = JsFuture::from(
                media_devices.get_user_media_with_constraints(&caps)?,
            )
            .await?;

            let stream = SysMediaStream::from(stream);

            js_sys::try_iter(&stream.get_tracks())
                .unwrap()
                .unwrap()
                .map(|tr| web_sys::MediaStreamTrack::from(tr.unwrap()))
                .for_each(|track| storage.borrow_mut().push(track));

            Ok(stream)
        }
    }
}

impl Drop for InnerMediaManager {
    fn drop(&mut self) {
        for track in self.tracks.borrow_mut().drain(..) {
            track.stop();
        }
    }
}

impl MediaManager {
    /// Obtains [MediaStream][1] basing on a provided
    /// [MediaStreamConstraints][2].
    /// Acquired streams are cached and cloning existing stream is preferable
    /// over obtaining new ones.
    ///
    /// `on_local_stream` callback will be invoked each time new stream was
    /// obtained.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
    pub async fn get_stream<I: Into<MediaStreamConstraints>>(
        &self,
        caps: I,
    ) -> Result<SysMediaStream, WasmErr> {
        let inner = Rc::clone(&self.0);

        async {
            let (stream, is_new_stream) =
                self.0.get_stream(caps.into()).await?;

            if is_new_stream {
                inner.on_local_stream.call1(copy_js_ref(&stream));
            }

            Ok(stream)
        }
            .await
            .map_err(move |err: WasmErr| {
                inner.on_local_stream.call2(err.clone());
                err
            })
    }

    /// Sets `on_local_stream` callback that will be invoked when
    /// [`MediaManager`] obtains new [MediaStream].
    #[inline]
    pub fn set_on_local_stream(&self, f: js_sys::Function) {
        self.0.on_local_stream.set_func(f);
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
pub struct MediaManagerHandle(Weak<InnerMediaManager>);

#[wasm_bindgen]
impl MediaManagerHandle {
    /// Returns the JS array of [`MediaDeviceInfo`] objects.
    pub fn enumerate_devices(&self) -> Promise {
        match map_weak!(self, |inner| inner.enumerate_devices()) {
            Ok(devices) => future_to_promise(async {
                devices
                    .await
                    .map(|devices| {
                        devices
                            .into_iter()
                            .fold(js_sys::Array::new(), |devices_info, info| {
                                devices_info.push(&JsValue::from(info));
                                devices_info
                            })
                            .into()
                    })
                    .map_err(Into::into)
            }),
            Err(e) => future_to_promise(future::err(e)),
        }
    }

    /// Returns [`MediaStream`] object.
    pub fn init_local_stream(&self, caps: MediaStreamConstraints) -> Promise {
        match map_weak!(self, |inner| { inner.get_stream(caps) }) {
            Ok(stream) => future_to_promise(async {
                stream
                    .await
                    .map(|(stream, _)| stream.into())
                    .map_err(|err| err.into())
            }),
            Err(err) => future_to_promise(future::err(err)),
        }
    }
}
