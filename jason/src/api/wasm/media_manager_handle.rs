use derive_more::From;
use js_sys::Promise;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::{
    api::{InputDeviceInfo, LocalMediaTrack, MediaStreamSettings},
    media,
};

use super::jason_error::JasonError;

/// [`MediaManagerHandle`] is a weak ref to [`MediaManager`].
///
/// [`MediaManager`] performs all media acquisition requests
/// ([getUserMedia()][1]/[getDisplayMedia()][2]) and stores all received tracks
/// for further reusage.
///
/// [`MediaManager`] stores weak references to [`LocalMediaTrack`]s, so if there
/// are no strong references to some track, then this track is stopped and
/// deleted from [`MediaManager`].
///
/// Like all handlers it contains weak reference to object that is managed by
/// Rust, so its methods will fail if weak reference could not be upgraded.
///
/// [`MediaManager`]: media::MediaManager
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[wasm_bindgen]
#[derive(From)]
pub struct MediaManagerHandle(media::MediaManagerHandle);

#[wasm_bindgen]
#[allow(clippy::unused_self)]
impl MediaManagerHandle {
    /// Returns array of [`InputDeviceInfo`] objects, which represent available
    /// media input and output devices, such as microphones, cameras, and so
    /// forth.
    pub fn enumerate_devices(&self) -> Promise {
        let this = self.0.clone();

        future_to_promise(async move {
            this.enumerate_devices()
                .await
                .map(|devices| {
                    devices
                        .into_iter()
                        .fold(js_sys::Array::new(), |devices_info, info| {
                            devices_info.push(&JsValue::from(
                                InputDeviceInfo::from(info),
                            ));
                            devices_info
                        })
                        .into()
                })
                .map_err(JasonError::from)
                .map_err(JsValue::from)
        })
    }

    /// Returns [`LocalMediaTrack`]s objects, built from provided
    /// [`MediaStreamSettings`].
    pub fn init_local_tracks(&self, caps: &MediaStreamSettings) -> Promise {
        let this = self.0.clone();
        let caps = caps.clone();

        future_to_promise(async move {
            this.init_local_tracks(caps.into())
                .await
                .map(|tracks| {
                    tracks
                        .into_iter()
                        .fold(js_sys::Array::new(), |tracks, track| {
                            tracks.push(&JsValue::from(LocalMediaTrack::from(
                                track,
                            )));
                            tracks
                        })
                        .into()
                })
                .map_err(JasonError::from)
                .map_err(JsValue::from)
        })
    }
}
