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

#[wasm_bindgen]
pub struct LocalMediaStreamTrack(LocalMediaTrack<Strong>);

impl LocalMediaStreamTrack {
    pub fn new(track: LocalMediaTrack<Strong>) -> Self {
        LocalMediaStreamTrack(track)
    }
}

#[wasm_bindgen]
impl LocalMediaStreamTrack {
    /// Returns underlying [`SysMediaStreamTrack`] from this
    /// [`MediaStreamTrack`].
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(self.0.as_ref())
    }

    /// Returns a [`String`] set to `audio` if the track is an audio track and
    /// to `video`, if it is a video track.
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns a [`String`] set to `device` if track is sourced from some
    /// device (webcam/microphone) and to `display`, if track is captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[wasm_bindgen(js_name = media_source_kind)]
    pub fn js_media_source_kind(&self) -> JsMediaSourceKind {
        self.0.source_kind().into()
    }
}

#[derive(Clone, Debug)]
pub struct Soft(Weak<DeepTrack>);

#[derive(Clone, Debug)]
pub struct Strong(Rc<DeepTrack>);

#[derive(Clone, Debug)]
pub struct LocalMediaTrack<S> {
    source_kind: MediaSourceKind,
    kind: MediaKind,
    deep_track: S,
}

impl LocalMediaTrack<Soft> {
    /// Checks whether this weak reference can be upgraded to a strong one.
    #[inline]
    pub fn can_be_upgraded(&self) -> bool {
        self.deep_track.0.strong_count() > 0
    }

    pub fn upgrade(&self) -> Option<LocalMediaTrack<Strong>> {
        Some(LocalMediaTrack {
            deep_track: Strong(Weak::upgrade(&self.deep_track.0)?),
            kind: self.kind,
            source_kind: self.source_kind,
        })
    }
}

impl AsRef<SysMediaStreamTrack> for LocalMediaTrack<Strong> {
    fn as_ref(&self) -> &SysMediaStreamTrack {
        self.deep_track.0.track()
    }
}

impl LocalMediaTrack<Strong> {
    pub fn new(track: SysMediaStreamTrack, source_kind: MediaSourceKind) -> Self {
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };
        Self {
            deep_track: Strong(DeepTrack::new(track)),
            source_kind,
            kind,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.deep_track.0.track().set_enabled(enabled);
    }

    pub fn id(&self) -> String {
        self.deep_track.0.id()
    }

    pub fn source_kind(&self) -> MediaSourceKind {
        self.source_kind
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn deep_clone(&self) -> Self {
        Self {
            deep_track: Strong(self.deep_track.0.deep_clone_track()),
            source_kind: self.source_kind,
            kind: self.kind
        }
    }

    pub fn downgrade(&self) -> LocalMediaTrack<Soft> {
        LocalMediaTrack {
            deep_track: Soft(Rc::downgrade(&self.deep_track.0)),
            kind: self.kind,
            source_kind: self.source_kind
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeepTrack {
    parent: Option<Rc<DeepTrack>>,
    track: SysMediaStreamTrack,
}

impl DeepTrack {
    pub fn new(track: SysMediaStreamTrack) -> Rc<Self> {
        Rc::new(Self {
            parent: None,
            track,
        })
    }

    pub fn id(&self) -> String {
        self.track.id()
    }

    pub fn track(&self) -> &SysMediaStreamTrack {
        &self.track
    }

    pub fn deep_clone_track(self: &Rc<Self>) -> Rc<Self> {
        let parent = Rc::clone(self);
        let cloned_track = SysMediaStreamTrack::clone(&self.track);
        Rc::new(Self {
            parent: Some(parent),
            track: cloned_track,
        })
    }
}

impl Drop for DeepTrack {
    fn drop(&mut self) {
        self.track.stop();
    }
}

/// Wrapper around [`SysMediaStreamTrack`] to track when it's enabled or
/// disabled.
struct InnerMediaStreamTrack {
    track: SysMediaStreamTrack,

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
    /// Creates new [`MediaStreamTrack`] with a provided
    /// [`DeeplyCloneableTrack`] and provided [`SysMediaStreamTrack`], spawns
    /// listener for [`InnerMediaStreamTrack::enabled`] state changes.
    pub fn new(
        track: SysMediaStreamTrack,
        media_source_kind: MediaSourceKind,
    ) -> Self {
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };

        let track = MediaStreamTrack(Rc::new(InnerMediaStreamTrack {
            enabled: ObservableCell::new(track.enabled()),
            track,
            on_enabled: Callback0::default(),
            on_disabled: Callback0::default(),
            media_source_kind,
            kind,
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
    }

    /// Returns root [`id`][1] of underlying [`DeeplyCloneableTrack`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn id(&self) -> String {
        self.0.track.id()
    }

    /// Returns track kind (audio/video).
    #[inline]
    pub fn kind(&self) -> MediaKind {
        self.0.kind
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
