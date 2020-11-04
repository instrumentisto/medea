//! [MediaStreamTrack][1] related objects.
//!
//! [1]: https://developer.mozilla.org/en-US/docs/Web/API/MediaStreamTrack

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use futures::StreamExt;
use medea_client_api_proto::MediaSourceKind;
use medea_reactive::ObservableCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{media::MediaKind, utils::Callback0};

/// Media source type.
#[wasm_bindgen(js_name = MediaSourceKind)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JsMediaSourceKind {
    /// Media is sourced from some media device (webcam or microphone).
    Device,

    /// Media is obtained with screen-capture.
    Display,
}

impl From<JsMediaSourceKind> for MediaSourceKind {
    fn from(val: JsMediaSourceKind) -> Self {
        match val {
            JsMediaSourceKind::Device => Self::Device,
            JsMediaSourceKind::Display => Self::Display,
        }
    }
}

impl From<MediaSourceKind> for JsMediaSourceKind {
    fn from(val: MediaSourceKind) -> Self {
        match val {
            MediaSourceKind::Device => Self::Device,
            MediaSourceKind::Display => Self::Display,
        }
    }
}

struct TracksRepo {
    root_track: SysMediaStreamTrack,
    tracks: Vec<SysMediaStreamTrack>,
}

impl TracksRepo {
    fn root_id(&self) -> String {
        self.root_track.id()
    }

    fn new(track: SysMediaStreamTrack) -> Self {
        Self {
            root_track: track,
            tracks: Vec::new(),
        }
    }

    fn new_child(&mut self) -> SysMediaStreamTrack {
        let new_track = SysMediaStreamTrack::clone(&self.root_track);
        let cloned_track = Clone::clone(&new_track);
        self.tracks.push(cloned_track);

        new_track
    }

    fn update_root_enabled(&self) {
        self.root_track
            .set_enabled(self.tracks.iter().any(SysMediaStreamTrack::enabled));
    }

    fn get_root(&self) -> SysMediaStreamTrack {
        Clone::clone(&self.root_track)
    }
}

impl Drop for TracksRepo {
    fn drop(&mut self) {
        self.tracks.drain(..).for_each(|track| {
            track.stop();
        });
        self.root_track.stop();
    }
}

/// Wrapper around [`SysMediaStreamTrack`] to track when it's enabled or
/// disabled.
struct InnerMediaStreamTrack {
    /// Underlying JS-side [`SysMediaStreamTrack`].
    track: SysMediaStreamTrack,

    tracks: Rc<RefCell<TracksRepo>>,

    /// Underlying [`SysMediaStreamTrack`] kind.
    kind: MediaKind,

    /// Underlying [`SysMediaStreamTrack`] source kind.
    media_source_kind: MediaSourceKind,

    /// Callback to be invoked when this [`MediaStreamTrack`] is enabled.
    on_enabled: Callback0,

    /// Callback to be invoked when this [`MediaStreamTrack`] is disabled.
    on_disabled: Callback0,

    /// [enabled] property of [MediaStreamTrack][1].
    ///
    /// [enabled]: https://tinyurl.com/y5byqdea
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
    enabled: ObservableCell<bool>,
}

/// Strong reference to [MediaStreamTrack][1].
///
/// Track will be automatically stopped when there are no strong references
/// left.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[wasm_bindgen(js_name = MediaTrack)]
#[derive(Clone)]
pub struct MediaStreamTrack(Rc<InnerMediaStreamTrack>);

impl MediaStreamTrack {
    fn inner_new(
        tracks: Rc<RefCell<TracksRepo>>,
        track: SysMediaStreamTrack,
        media_source_kind: MediaSourceKind,
    ) -> Self {
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };

        let track = MediaStreamTrack(Rc::new(InnerMediaStreamTrack {
            tracks,
            enabled: ObservableCell::new(track.enabled()),
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
            media_source_kind,
            kind,
            track,
        }));

        let mut track_enabled_state_changes =
            track.enabled().subscribe().skip(1);
        spawn_local({
            let weak_inner = Rc::downgrade(&track.0);
            async move {
                while let Some(enabled) =
                    track_enabled_state_changes.next().await
                {
                    if let Some(track) = weak_inner.upgrade() {
                        if enabled {
                            track.on_enabled.call();
                        } else {
                            track.on_disabled.call();
                        }
                    } else {
                        break;
                    }
                }
            }
        });

        track
    }

    /// Creates new [`MediaStreamTrack`], spawns listener for
    /// [`InnerMediaStreamTrack::enabled`] state changes.
    pub fn new<T>(track: T, media_source_kind: MediaSourceKind) -> Self
    where
        SysMediaStreamTrack: From<T>,
    {
        let track = SysMediaStreamTrack::from(track);
        let tracks =
            Rc::new(RefCell::new(TracksRepo::new(Clone::clone(&track))));
        let track = tracks.borrow_mut().new_child();
        Self::inner_new(tracks, track, media_source_kind)
    }

    #[inline]
    pub fn new_root(&self) -> Self {
        Self::inner_new(
            self.0.tracks.clone(),
            self.0.tracks.borrow().get_root(),
            self.0.media_source_kind,
        )
    }

    pub fn deep_clone(&self) -> Self {
        let new_track = self.0.tracks.borrow_mut().new_child();

        Self::inner_new(
            self.0.tracks.clone(),
            new_track,
            self.0.media_source_kind,
        )
    }

    /// Returns `true` if this [`MediaStreamTrack`] is enabled.
    #[inline]
    pub fn enabled(&self) -> &ObservableCell<bool> {
        &self.0.enabled
    }

    /// Sets [`MediaStreamTrack::enabled`] to the provided value.
    ///
    /// Updates `enabled` in the underlying [`SysMediaStreamTrack`].
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.0.enabled.set(enabled);
        self.0.track.set_enabled(enabled);
        self.0.tracks.borrow().update_root_enabled();
    }

    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn id(&self) -> String {
        self.0.tracks.borrow().root_id()
    }

    /// Returns track kind (audio/video).
    #[inline]
    pub fn kind(&self) -> MediaKind {
        self.0.kind
    }

    /// Creates weak reference to underlying [MediaStreamTrack][2].
    ///
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn downgrade(&self) -> WeakMediaStreamTrack {
        WeakMediaStreamTrack(Rc::downgrade(&self.0))
    }

    /// Returns this [`MediaStreamTrack`] media source kind.
    #[inline]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind
    }
}

#[wasm_bindgen(js_class = MediaTrack)]
impl MediaStreamTrack {
    /// Returns underlying [`SysMediaStreamTrack`] from this
    /// [`MediaStreamTrack`].
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(&self.0.track)
    }

    /// Returns is this [`MediaStreamTrack`] enabled.
    #[wasm_bindgen(js_name = enabled)]
    pub fn js_enabled(&self) -> bool {
        self.0.enabled.get()
    }

    /// Sets callback, which will be invoked when this [`MediaStreamTrack`] is
    /// enabled.
    pub fn on_enabled(&self, callback: js_sys::Function) {
        self.0.on_enabled.set_func(callback);
    }

    /// Sets callback, which will be invoked when this [`MediaStreamTrack`] is
    /// enabled.
    pub fn on_disabled(&self, callback: js_sys::Function) {
        self.0.on_disabled.set_func(callback);
    }

    /// Returns a [`String`] set to `audio` if the track is an audio track and
    /// to `video`, if it is a video track.
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> MediaKind {
        self.kind()
    }

    /// Returns a [`String`] set to `device` if track is sourced from some
    /// device (webcam/microphone) and to `display`, if track is captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[wasm_bindgen(js_name = media_source_kind)]
    pub fn js_media_source_kind(&self) -> JsMediaSourceKind {
        self.0.media_source_kind.into()
    }
}

impl AsRef<SysMediaStreamTrack> for MediaStreamTrack {
    #[inline]
    fn as_ref(&self) -> &SysMediaStreamTrack {
        &self.0.track
    }
}

impl Drop for MediaStreamTrack {
    #[inline]
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        if Rc::strong_count(&self.0) == 1 {
            self.0.track.stop();
        }
    }
}

/// Weak reference to [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
pub struct WeakMediaStreamTrack(Weak<InnerMediaStreamTrack>);

impl WeakMediaStreamTrack {
    /// Tries to upgrade this weak reference to a strong one.
    #[inline]
    pub fn upgrade(&self) -> Option<MediaStreamTrack> {
        self.0.upgrade().map(MediaStreamTrack)
    }

    /// Checks whether this weak reference can be upgraded to a strong one.
    #[inline]
    pub fn can_be_upgraded(&self) -> bool {
        self.0.strong_count() > 0
    }
}
