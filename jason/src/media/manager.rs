//! Acquiring and storing [MediaStream][1]s.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::{
    cell::RefCell,
    convert::TryFrom,
    fmt,
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
    MediaStreamConstraints as SysMediaStreamConstraints,
};

use crate::{
    media::{
        stream::{MediaStream, MediaStreamTrack, WeakMediaStreamTrack},
        MediaStreamSettings, MultiSourceMediaStreamConstraints,
    },
    utils::{window, HandlerDetachedError, JasonError, JsCaused, JsError},
};

use super::InputDeviceInfo;

// TODO: Screen capture API (https://w3.org/TR/screen-capture/) is in draft
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

    /// Occurs if the [MediaDevices.getDisplayMedia()][1] request failed.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[display(fmt = "MediaDevices.getDisplayMedia() failed: {}", _0)]
    GetDisplayMediaFailed(JsError),

    /// Occurs when cannot get info about connected [MediaDevices][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediadevices
    #[display(fmt = "MediaDevices.enumerateDevices() failed: {}", _0)]
    EnumerateDevicesFailed(JsError),
}

#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct GetUserMediaError {
    pub media_type: GetUserMediaType,
}

#[wasm_bindgen]
impl GetUserMediaError {
    pub fn media_type(&self) -> String {
        self.media_type.to_string()
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum GetUserMediaType {
    Audio,
    Video,
}

impl fmt::Display for GetUserMediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_write = match self {
            Self::Audio => "audio",
            Self::Video => "video",
        };
        write!(f, "{}", to_write)
    }
}

impl TryFrom<&MediaManagerError> for GetUserMediaError {
    type Error = ();

    fn try_from(
        value: &MediaManagerError,
    ) -> std::result::Result<Self, Self::Error> {
        match value {
            MediaManagerError::GetUserMediaFailed(e) => {
                let media_type = if e.message.contains("audio") {
                    GetUserMediaType::Audio
                } else if e.message.contains("video") {
                    GetUserMediaType::Video
                } else {
                    return Err(());
                };

                Ok(Self { media_type })
            }
            _ => Err(()),
        }
    }
}

type Result<T> = std::result::Result<T, Traced<MediaManagerError>>;

/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to
/// [`MediaStreamTrack`]s, so if there are no strong references to some track,
/// then this track is stopped and deleted from [`MediaManager`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(Default)]
pub struct MediaManager(Rc<InnerMediaManager>);

/// Actual data of [`MediaManager`].
#[derive(Default)]
struct InnerMediaManager {
    /// Obtained tracks storage
    tracks: Rc<RefCell<Vec<WeakMediaStreamTrack>>>,
}

impl InnerMediaManager {
    /// Returns the vector of [`MediaDeviceInfo`] objects.
    fn enumerate_devices() -> impl Future<Output = Result<Vec<InputDeviceInfo>>>
    {
        use MediaManagerError::{
            CouldNotGetMediaDevices, EnumerateDevicesFailed,
        };

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

    /// Obtains [`MediaStream`] based on a provided [`MediaStreamSettings`].
    /// This can be a stream built from tracks that were acquired earlier, or
    /// from new tracks, acquired via [getUserMedia()][1] or/and
    /// [getDisplayMedia()][2] requests.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] IF [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    fn get_stream(
        &self,
        mut caps: MediaStreamSettings,
    ) -> impl Future<Output = Result<(MediaStream, bool)>> {
        let original_caps = caps.clone();

        let mut result = self.get_from_storage(&mut caps);
        let caps: Option<MultiSourceMediaStreamConstraints> = caps.into();
        match caps {
            None => {
                future::ok((MediaStream::new(result, original_caps), false))
                    .left_future()
                    .left_future()
            }
            Some(MultiSourceMediaStreamConstraints::Display(caps)) => self
                .get_display_media(caps)
                .map_ok(|mut tracks| {
                    result.append(&mut tracks);
                    result
                })
                .map_ok(|result| {
                    (MediaStream::new(result, original_caps), true)
                })
                .left_future()
                .right_future(),
            Some(MultiSourceMediaStreamConstraints::Device(caps)) => self
                .get_user_media(caps)
                .map_ok(|mut tracks| {
                    result.append(&mut tracks);
                    result
                })
                .map_ok(|result| {
                    (MediaStream::new(result, original_caps), true)
                })
                .right_future()
                .left_future(),
            Some(MultiSourceMediaStreamConstraints::DeviceAndDisplay(
                device_caps,
                display_caps,
            )) => {
                let get_user_media = self.get_user_media(device_caps);
                let get_display_media = self.get_display_media(display_caps);

                async move {
                    let mut get_user_media = get_user_media.await?;
                    let mut get_display_media = get_display_media.await?;
                    result.append(&mut get_user_media);
                    result.append(&mut get_display_media);

                    Ok((MediaStream::new(result, original_caps), true))
                }
                .right_future()
                .right_future()
            }
        }
    }

    /// Tries to find [`MediaStreamTrack`]s that satisfies
    /// [`MediaStreamSettings`], from tracks that were acquired earlier to avoid
    /// redundant [getUserMedia()][1]/[getDisplayMedia()][2] calls.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    fn get_from_storage(
        &self,
        caps: &mut MediaStreamSettings,
    ) -> Vec<MediaStreamTrack> {
        // cleanup weak links
        self.tracks
            .borrow_mut()
            .retain(WeakMediaStreamTrack::can_be_upgraded);

        let mut tracks = Vec::new();
        let storage: Vec<_> = self
            .tracks
            .borrow()
            .iter()
            .map(|track| track.upgrade().unwrap())
            .collect();

        if let Some(audio) = caps.get_audio() {
            let track = storage
                .iter()
                .find(|track| audio.satisfies(track.as_ref()))
                .cloned();

            if let Some(track) = track {
                caps.take_audio();
                tracks.push(track);
            }
        }

        if let Some(video) = caps.get_video() {
            let track = storage
                .iter()
                .find(|track| video.satisfies(track.as_ref()))
                .cloned();

            if let Some(track) = track {
                caps.take_video();
                tracks.push(track);
            }
        }

        tracks
    }

    /// Obtains new [MediaStream][1] making [getUserMedia()][2] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://tinyurl.com/rnxcavf
    fn get_user_media(
        &self,
        caps: SysMediaStreamConstraints,
    ) -> impl Future<Output = Result<Vec<MediaStreamTrack>>> {
        use MediaManagerError::{CouldNotGetMediaDevices, GetUserMediaFailed};

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
            .map(SysMediaStream::from)
            .map_err(JsError::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?;

            let mut storage_mut = storage.borrow_mut();
            let tracks: Vec<_> = js_sys::try_iter(&stream.get_tracks())
                .unwrap()
                .unwrap()
                .map(|tr| MediaStreamTrack::from(tr.unwrap()))
                .inspect(|track| storage_mut.push(track.downgrade()))
                .collect();

            Ok(tracks)
        }
    }

    /// Obtains new [MediaStream][1] making [getDisplayMedia()][2] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    fn get_display_media(
        &self,
        caps: SysMediaStreamConstraints,
    ) -> impl Future<Output = Result<Vec<MediaStreamTrack>>> {
        use MediaManagerError::{
            CouldNotGetMediaDevices, GetDisplayMediaFailed, GetUserMediaFailed,
        };

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
            .map(SysMediaStream::from)
            .map_err(JsError::from)
            .map_err(GetUserMediaFailed)
            .map_err(tracerr::from_and_wrap!())?;

            let mut storage_mut = storage.borrow_mut();
            let tracks: Vec<_> = js_sys::try_iter(&stream.get_tracks())
                .unwrap()
                .unwrap()
                .map(|tr| MediaStreamTrack::from(tr.unwrap()))
                .inspect(|track| storage_mut.push(track.downgrade()))
                .collect();

            Ok(tracks)
        }
    }
}

impl MediaManager {
    /// Obtains [`MediaStream`] based on a provided [`MediaStreamSettings`].
    /// This can be a stream built from tracks that were acquired earlier, or
    /// from new tracks, acquired via [getUserMedia()][1] or/and
    /// [getDisplayMedia()][2] requests.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] if [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    /// [1]: https://tinyurl.com/rnxcavf
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub async fn get_stream<I: Into<MediaStreamSettings>>(
        &self,
        caps: I,
    ) -> Result<(MediaStream, bool)> {
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
/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to [`MediaStreamTrack`]s, so if
/// there are no strong references to some track, then this track is stopped
/// and deleted from [`MediaManager`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[wasm_bindgen]
pub struct MediaManagerHandle(Weak<InnerMediaManager>);

#[wasm_bindgen]
#[allow(clippy::unused_self)]
impl MediaManagerHandle {
    /// Returns array of [`InputDeviceInfo`] objects, which represent available
    /// media input and output devices, such as microphones, cameras, and so
    /// forth.
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

    /// Returns [`MediaStream`](LocalMediaStream) object, built from provided
    /// [`MediaStreamSettings`].
    pub fn init_local_stream(&self, caps: &MediaStreamSettings) -> Promise {
        match upgrade_or_detached!(self.0)
            .map(|inner| inner.get_stream(caps.clone()))
        {
            Ok(stream) => future_to_promise(async {
                stream
                    .await
                    .map(|(stream, _)| stream.into())
                    .map_err(tracerr::wrap!(=> MediaManagerError))
                    .map_err(|e| {
                        if let Ok(err) = GetUserMediaError::try_from(e.as_ref())
                        {
                            err.into()
                        } else {
                            JasonError::from(e).into()
                        }
                    })
            }),
            Err(err) => future_to_promise(future::err(err)),
        }
    }
}
