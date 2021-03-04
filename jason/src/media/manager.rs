//! Acquiring and storing [`local::Track`]s.

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::TryFrom,
    rc::{Rc, Weak},
};

use derive_more::Display;
use js_sys::Promise;
use medea_client_api_proto::MediaSourceKind;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsValue};
use wasm_bindgen_futures::{future_to_promise, JsFuture};
use web_sys as sys;

use crate::{
    media::{MediaStreamSettings, MultiSourceTracksConstraints},
    utils::{window, HandlerDetachedError, JasonError, JsCaused, JsError},
    MediaKind,
};

use super::{track::local, InputDeviceInfo};

/// Errors that may occur in a [`MediaManager`].
#[derive(Clone, Debug, Display, JsCaused)]
pub enum MediaManagerError {
    /// Occurs when cannot get access to [MediaDevices][1] object.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediadevices
    #[display(fmt = "Navigator.mediaDevices() failed: {}", _0)]
    CouldNotGetMediaDevices(JsError),

    /// Occurs if the [getUserMedia][1] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    #[display(fmt = "MediaDevices.getUserMedia() failed: {}", _0)]
    GetUserMediaFailed(JsError),

    /// Occurs if the [getDisplayMedia()][1] request failed.
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[display(fmt = "MediaDevices.getDisplayMedia() failed: {}", _0)]
    GetDisplayMediaFailed(JsError),

    /// Occurs when cannot get info about connected [MediaDevices][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediadevices
    #[display(fmt = "MediaDevices.enumerateDevices() failed: {}", _0)]
    EnumerateDevicesFailed(JsError),

    /// Occurs when local track is [`ended`][1] right after [getUserMedia()][2]
    /// or [getDisplayMedia()][3] request.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    /// [2]: https://tinyurl.com/rnxcavf
    /// [3]: https://w3.org/TR/screen-capture#dom-mediadevices-getdisplaymedia
    #[display(fmt = "{} track is ended", _0)]
    LocalTrackIsEnded(MediaKind),
}

type Result<T> = std::result::Result<T, Traced<MediaManagerError>>;

/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to
/// [`local::Track`]s, so if there are no strong references to some track,
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
    tracks: Rc<RefCell<HashMap<String, Weak<local::Track>>>>,
}

impl InnerMediaManager {
    /// Returns the vector of [`InputDeviceInfo`] objects.
    async fn enumerate_devices() -> Result<Vec<InputDeviceInfo>> {
        use MediaManagerError::{
            CouldNotGetMediaDevices, EnumerateDevicesFailed,
        };

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

    /// Obtains [`local::Track`]s based on a provided
    /// [`MediaStreamSettings`]. This can be the tracks that were acquired
    /// earlier, or new tracks, acquired via [getUserMedia()][1] or/and
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
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    async fn get_tracks(
        &self,
        mut caps: MediaStreamSettings,
    ) -> Result<Vec<(Rc<local::Track>, bool)>> {
        let tracks_from_storage = self
            .get_from_storage(&mut caps)
            .into_iter()
            .map(|t| (t, false));
        match caps.into() {
            None => Ok(tracks_from_storage.collect()),
            Some(MultiSourceTracksConstraints::Display(caps)) => {
                Ok(tracks_from_storage
                    .chain(
                        self.get_display_media(caps)
                            .await?
                            .into_iter()
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
            Some(MultiSourceTracksConstraints::Device(caps)) => {
                Ok(tracks_from_storage
                    .chain(
                        self.get_user_media(caps)
                            .await?
                            .into_iter()
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
            Some(MultiSourceTracksConstraints::DeviceAndDisplay(
                device_caps,
                display_caps,
            )) => {
                let device_tracks = self.get_user_media(device_caps).await?;
                let display_tracks =
                    self.get_display_media(display_caps).await?;
                Ok(tracks_from_storage
                    .chain(
                        device_tracks
                            .into_iter()
                            .chain(display_tracks.into_iter())
                            .map(|t| (t, true)),
                    )
                    .collect())
            }
        }
    }

    /// Tries to find [`local::Track`]s that satisfies [`MediaStreamSettings`],
    /// from tracks that were acquired earlier to avoid redundant
    /// [getUserMedia()][1]/[getDisplayMedia()][2] calls.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    fn get_from_storage(
        &self,
        caps: &mut MediaStreamSettings,
    ) -> Vec<Rc<local::Track>> {
        // cleanup weak links
        self.tracks
            .borrow_mut()
            .retain(|_, track| Weak::strong_count(track) > 0);

        let mut tracks = Vec::new();
        let storage: Vec<_> = self
            .tracks
            .borrow()
            .iter()
            .map(|(_, track)| Weak::upgrade(track).unwrap())
            .collect();

        if caps.is_audio_enabled() {
            let track = storage
                .iter()
                .find(|track| caps.get_audio().satisfies(track.sys_track()))
                .cloned();

            if let Some(track) = track {
                caps.set_audio_publish(false);
                tracks.push(track);
            }
        }

        tracks.extend(
            storage
                .iter()
                .filter(|track| {
                    caps.unconstrain_if_satisfies_video(track.sys_track())
                })
                .cloned(),
        );

        tracks
    }

    /// Obtains new [MediaStream][1] making [getUserMedia()][2] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    async fn get_user_media(
        &self,
        caps: sys::MediaStreamConstraints,
    ) -> Result<Vec<Rc<local::Track>>> {
        use MediaManagerError::{CouldNotGetMediaDevices, GetUserMediaFailed};

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
        .map(sys::MediaStream::from)
        .map_err(JsError::from)
        .map_err(GetUserMediaFailed)
        .map_err(tracerr::from_and_wrap!())?;

        Ok(self.parse_and_save_tracks(stream, MediaSourceKind::Device)?)
    }

    /// Obtains new [MediaStream][1] making [getDisplayMedia()][2] call, saves
    /// received tracks weak refs to storage, returns list of tracks strong
    /// refs.
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    async fn get_display_media(
        &self,
        caps: sys::DisplayMediaStreamConstraints,
    ) -> Result<Vec<Rc<local::Track>>> {
        use MediaManagerError::{
            CouldNotGetMediaDevices, GetDisplayMediaFailed, GetUserMediaFailed,
        };

        let media_devices = window()
            .navigator()
            .media_devices()
            .map_err(JsError::from)
            .map_err(CouldNotGetMediaDevices)
            .map_err(tracerr::from_and_wrap!())?;

        let stream = JsFuture::from(
            media_devices
                .get_display_media_with_constraints(&caps)
                .map_err(JsError::from)
                .map_err(GetDisplayMediaFailed)
                .map_err(tracerr::from_and_wrap!())?,
        )
        .await
        .map(sys::MediaStream::from)
        .map_err(JsError::from)
        .map_err(GetUserMediaFailed)
        .map_err(tracerr::from_and_wrap!())?;

        Ok(self.parse_and_save_tracks(stream, MediaSourceKind::Display)?)
    }

    /// Retrieves tracks from provided [`sys::MediaStream`], saves tracks weak
    /// references in [`MediaManager`] tracks storage.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::LocalTrackIsEnded`] if at least one track from
    /// the provided [`sys::MediaStream`] is in [`ended`][1] state.
    ///
    /// In case of error all tracks are stopped and are not saved in
    /// [`MediaManager`]'s tracks storage.
    ///
    /// [1]: https://tinyurl.com/w3-streams#idl-def-MediaStreamTrackState.ended
    #[allow(clippy::needless_pass_by_value)]
    fn parse_and_save_tracks(
        &self,
        stream: sys::MediaStream,
        kind: MediaSourceKind,
    ) -> Result<Vec<Rc<local::Track>>> {
        use MediaManagerError::LocalTrackIsEnded;

        let mut storage = self.tracks.borrow_mut();
        let tracks: Vec<_> = js_sys::try_iter(&stream.get_tracks())
            .unwrap()
            .unwrap()
            .map(|tr| Rc::new(local::Track::new(tr.unwrap().into(), kind)))
            .collect();

        // Tracks returned by getDisplayMedia()/getUserMedia() request should be
        // `live`. Otherwise, we should err without caching tracks in
        // `MediaManager`. Tracks will be stopped on `Drop`.
        for track in &tracks {
            if track.sys_track().ready_state()
                != sys::MediaStreamTrackState::Live
            {
                return Err(tracerr::new!(LocalTrackIsEnded(track.kind())));
            }
        }

        for track in &tracks {
            storage.insert(track.id(), Rc::downgrade(&track));
        }

        Ok(tracks)
    }
}

impl MediaManager {
    /// Obtains [`local::Track`]s based on a provided [`MediaStreamSettings`].
    /// This can be the tracks that were acquired earlier, or new tracks,
    /// acquired via [getUserMedia()][1] or/and [getDisplayMedia()][2] requests.
    ///
    /// # Errors
    ///
    /// With [`MediaManagerError::GetUserMediaFailed`] if [getUserMedia()][1]
    /// request failed.
    ///
    /// With [`MediaManagerError::GetDisplayMediaFailed`] if
    /// [getDisplayMedia()][2] request failed.
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediadevices-getusermedia
    /// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub async fn get_tracks<I: Into<MediaStreamSettings>>(
        &self,
        caps: I,
    ) -> Result<Vec<(Rc<local::Track>, bool)>> {
        self.0.get_tracks(caps.into()).await
    }

    /// Instantiates new [`MediaManagerHandle`] for use on JS side.
    #[inline]
    #[must_use]
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
/// [`MediaManager`] stores weak references to [`local::Track`]s, so if there
/// are no strong references to some track, then this track is stopped and
/// deleted from [`MediaManager`].
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

    /// Returns [`local::JsTrack`]s objects, built from provided
    /// [`MediaStreamSettings`].
    pub fn init_local_tracks(&self, caps: &MediaStreamSettings) -> Promise {
        let inner = upgrade_or_detached!(self.0, JasonError);
        let caps = caps.clone();
        future_to_promise(async move {
            inner?
                .get_tracks(caps)
                .await
                .map(|tracks| {
                    tracks
                        .into_iter()
                        .map(|(t, _)| local::JsTrack::new(t))
                        .map(JsValue::from)
                        .collect::<js_sys::Array>()
                        .into()
                })
                .map_err(tracerr::wrap!(=> MediaManagerError))
                .map_err(|e| JasonError::from(e).into())
        })
    }
}
