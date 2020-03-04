//! Acquiring and storing [MediaStream][1]s.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    convert::TryFrom,
    future::Future,
    rc::{Rc, Weak},
};

use derive_more::Display;
use futures::{future, FutureExt as _, TryFutureExt as _};
use js_sys::Promise;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys::{
    MediaDevices, MediaStream as SysMediaStream,
    MediaStreamConstraints as SysMediaStreamConstraints, MediaStreamTrack,
};

use crate::{
    media::{MediaStreamConstraints, MultiSourceMediaStreamConstraints},
    utils::{window, HandlerDetachedError, JasonError, JsCaused, JsError},
};

use super::InputDeviceInfo;

// TODO: Screen capture API (https://www.w3.org/TR/screen-capture/) is in draft
//       stage atm, so there is no web-sys bindings for it.
//       Discussion https://github.com/rustwasm/wasm-bindgen/issues/1950
#[wasm_bindgen(inline_js = "export function get_display_media(media_devices, \
                            constraints) { return \
                            media_devices.getDisplayMedia(constraints) }")]
extern "C" {
    #[allow(clippy::needless_pass_by_value)]
    #[wasm_bindgen(catch)]
    fn get_display_media(
        media_devices: &MediaDevices,
        constraints: &SysMediaStreamConstraints,
    ) -> std::result::Result<Promise, JsValue>;
}

/// Errors that may occur in a [`MediaManager`].
#[derive(Debug, Display, JsCaused)]
pub enum MediaManagerError {
    /// Occurs when cannot get access to [MediaDevices][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediadevices
    #[display(fmt = "Navigator.mediaDevices() failed: {}", _0)]
    CouldNotGetMediaDevices(JsError),

    /// Occurs if the [getUserMedia][1] request failed.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    #[display(fmt = "MediaDevices.getUserMedia() failed: {}", _0)]
    GetUserMediaFailed(JsError),

    /// Occurs if the [getDisplayMedia][1] request failed.
    ///
    /// [1]: https://tinyurl.com/wotjrns
    #[display(fmt = "MediaDevices.getDisplayMedia() failed: {}", _0)]
    GetDisplayMediaFailed(JsError),

    /// Occurs when cannot get info about connected [MediaDevices][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediadevices
    #[display(fmt = "MediaDevices.enumerateDevices() failed: {}", _0)]
    EnumerateDevicesFailed(JsError),
}

type Result<T> = std::result::Result<T, Traced<MediaManagerError>>;

/// Manager that is responsible for [MediaStream][1] acquisition and storing.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
#[derive(Default)]
pub struct MediaManager(Rc<InnerMediaManager>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained tracks storage
    tracks: Rc<RefCell<Vec<MediaStreamTrack>>>,
}

impl InnerMediaManager {
    /// Returns the vector of [`MediaDeviceInfo`] objects.
    fn enumerate_devices() -> impl Future<Output = Result<Vec<InputDeviceInfo>>>
    {
        use MediaManagerError::*;
        async {
            let devices = window()
                .navigator()
                .media_devices()
                .map_err(JsError::from)
                .map_err(CouldNotGetMediaDevices)
                .map_err(tracerr::from_and_wrap!())?;
            let devices = JsFuture::from(
                devices
                    .enumerate_devices()
                    .map_err(JsError::from)
                    .map_err(EnumerateDevicesFailed)
                    .map_err(tracerr::from_and_wrap!())?,
            )
            .await
            .map_err(JsError::from)
            .map_err(EnumerateDevicesFailed)
            .map_err(tracerr::from_and_wrap!())?;

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
    /// [2]: https://tinyurl.com/rnxcavf
    fn get_stream(
        &self,
        caps: MediaStreamConstraints,
    ) -> impl Future<Output = Result<(SysMediaStream, bool)>> {
        if let Some(stream) = self.get_from_storage(&caps) {
            future::ok((stream, false)).left_future().left_future()
        } else {
            let caps: MultiSourceMediaStreamConstraints = caps.into();
            match caps {
                MultiSourceMediaStreamConstraints::Display(caps) => self
                    .get_display_media(caps)
                    .and_then(|stream| future::ok((stream, true)))
                    .left_future()
                    .right_future(),
                MultiSourceMediaStreamConstraints::Device(caps) => self
                    .get_user_media(caps)
                    .and_then(|stream| future::ok((stream, true)))
                    .right_future()
                    .left_future(),
                MultiSourceMediaStreamConstraints::DeviceAndDisplay(
                    device_caps,
                    display_caps,
                ) => {
                    let get_user_media = self.get_user_media(device_caps);
                    let get_display_media =
                        self.get_display_media(display_caps);

                    async move {
                        let get_user_media = get_user_media.await?;
                        let get_display_media = get_display_media.await?;

                        let merged =
                            merge_streams(&get_user_media, &get_display_media);

                        Ok((merged, true))
                    }
                    .right_future()
                    .right_future()
                }
            }
        }
    }

    /// Tries to build new [MediaStream][1] from already owned tracks to avoid
    /// redundant [getUserMedia()][2]/[getDisplayMedia()][3] calls.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://tinyurl.com/rnxcavf
    /// [3]: https://tinyurl.com/wotjrns
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

    /// Obtains new [MediaStream][1] making [getUserMedia()][2] call and saves
    /// its tracks to storage.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://tinyurl.com/rnxcavf
    fn get_user_media(
        &self,
        caps: SysMediaStreamConstraints,
    ) -> impl Future<Output = Result<SysMediaStream>> {
        use MediaManagerError::*;
        let storage = Rc::clone(&self.tracks);

        async move {
            let media_devices = window()
                .navigator()
                .media_devices()
                .map_err(JsError::from)
                .map_err(CouldNotGetMediaDevices)
                .map_err(tracerr::from_and_wrap!())?;

            let stream = JsFuture::from(
                media_devices
                    .get_user_media_with_constraints(&caps)
                    .map_err(JsError::from)
                    .map_err(GetUserMediaFailed)
                    .map_err(tracerr::from_and_wrap!())?,
            )
            .await
            .map_err(JsError::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?;

            let stream = SysMediaStream::from(stream);

            js_sys::try_iter(&stream.get_tracks())
                .unwrap()
                .unwrap()
                .map(|tr| MediaStreamTrack::from(tr.unwrap()))
                .for_each(|track| storage.borrow_mut().push(track));

            Ok(stream)
        }
    }

    /// Obtains new [MediaStream][1] making [getDisplayMedia()][2] call and
    /// saves its tracks to storage.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://tinyurl.com/wotjrns
    fn get_display_media(
        &self,
        caps: SysMediaStreamConstraints,
    ) -> impl Future<Output = Result<SysMediaStream>> {
        use MediaManagerError::*;
        let storage = Rc::clone(&self.tracks);

        async move {
            let media_devices = window()
                .navigator()
                .media_devices()
                .map_err(JsError::from)
                .map_err(CouldNotGetMediaDevices)
                .map_err(tracerr::from_and_wrap!())?;

            let stream = JsFuture::from(
                get_display_media(&media_devices, &caps)
                    .map_err(JsError::from)
                    .map_err(GetDisplayMediaFailed)
                    .map_err(tracerr::from_and_wrap!())?,
            )
            .await
            .map_err(JsError::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?;

            let stream = SysMediaStream::from(stream);

            js_sys::try_iter(&stream.get_tracks())
                .unwrap()
                .unwrap()
                .map(|tr| MediaStreamTrack::from(tr.unwrap()))
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
    ) -> Result<(SysMediaStream, bool)> {
        self.0.get_stream(caps.into()).await
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
#[allow(clippy::unused_self)]
impl MediaManagerHandle {
    /// Returns the JS array of [`InputDeviceInfo`] objects.
    pub fn enumerate_devices(&self) -> Promise {
        future_to_promise(async {
            InnerMediaManager::enumerate_devices()
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
                .map_err(tracerr::wrap!(=> MediaManagerError))
                .map_err(|e| JasonError::from(e).into())
        })
    }

    /// Returns [MediaStream][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    pub fn init_local_stream(&self, caps: MediaStreamConstraints) -> Promise {
        match upgrade_or_detached!(self.0).map(|inner| inner.get_stream(caps)) {
            Ok(stream) => future_to_promise(async {
                stream
                    .await
                    .map(|(stream, _)| stream.into())
                    .map_err(tracerr::wrap!(=> MediaManagerError))
                    .map_err(|e| JasonError::from(e).into())
            }),
            Err(err) => future_to_promise(future::err(err)),
        }
    }
}

/// Builds new [`SysMediaStream`] from tracks in provided [`SysMediaStream`]s.
fn merge_streams(
    left: &SysMediaStream,
    right: &SysMediaStream,
) -> SysMediaStream {
    let stream = SysMediaStream::new().unwrap();

    js_sys::try_iter(&left.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .for_each(|track| stream.add_track(&track));

    js_sys::try_iter(&right.get_tracks())
        .unwrap()
        .unwrap()
        .map(|tr| MediaStreamTrack::from(tr.unwrap()))
        .for_each(|track| stream.add_track(&track));

    stream
}
