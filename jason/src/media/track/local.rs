use std::rc::{Rc, Weak};

use derive_more::AsRef;
use medea_client_api_proto::MediaSourceKind;
use wasm_bindgen::prelude::*;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{media::MediaKind, JsMediaSourceKind};

#[derive(Clone, Debug)]
pub struct Track<S> {
    source_kind: MediaSourceKind,
    kind: MediaKind,
    track: S,
}

#[derive(Clone, Debug)]
pub struct WeakPtr(Weak<InnerTrack>);

#[derive(Clone, Debug)]
pub struct SharedPtr(Rc<InnerTrack>);

impl Track<WeakPtr> {
    /// Checks whether this weak reference can be upgraded to a strong one.
    #[inline]
    pub fn can_be_upgraded(&self) -> bool {
        self.track.0.strong_count() > 0
    }

    pub fn upgrade(&self) -> Option<Track<SharedPtr>> {
        Some(Track {
            track: SharedPtr(Weak::upgrade(&self.track.0)?),
            kind: self.kind,
            source_kind: self.source_kind,
        })
    }
}

impl AsRef<SysMediaStreamTrack> for Track<SharedPtr> {
    fn as_ref(&self) -> &SysMediaStreamTrack {
        self.track.0.as_ref().as_ref()
    }
}

impl Track<SharedPtr> {
    pub fn new(
        track: SysMediaStreamTrack,
        source_kind: MediaSourceKind,
    ) -> Self {
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };
        Self {
            track: SharedPtr(InnerTrack::new(track)),
            source_kind,
            kind,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.track.0.as_ref().as_ref().set_enabled(enabled);
    }

    pub fn id(&self) -> String {
        self.track.0.id()
    }

    pub fn source_kind(&self) -> MediaSourceKind {
        self.source_kind
    }

    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    pub fn fork(&self) -> Self {
        Self {
            track: SharedPtr(self.track.0.fork()),
            source_kind: self.source_kind,
            kind: self.kind,
        }
    }

    pub fn downgrade(&self) -> Track<WeakPtr> {
        Track {
            track: WeakPtr(Rc::downgrade(&self.track.0)),
            kind: self.kind,
            source_kind: self.source_kind,
        }
    }
}

#[derive(AsRef, Clone, Debug)]
struct InnerTrack {
    parent: Option<Rc<InnerTrack>>,
    #[as_ref]
    track: SysMediaStreamTrack,
}

impl InnerTrack {
    fn new(track: SysMediaStreamTrack) -> Rc<Self> {
        Rc::new(Self {
            parent: None,
            track,
        })
    }

    fn id(&self) -> String {
        self.track.id()
    }

    fn fork(self: &Rc<Self>) -> Rc<Self> {
        let parent = Rc::clone(self);
        let track = SysMediaStreamTrack::clone(&self.track);
        Rc::new(Self {
            parent: Some(parent),
            track,
        })
    }
}

impl Drop for InnerTrack {
    fn drop(&mut self) {
        self.track.stop();
    }
}

#[wasm_bindgen(js_name = LocalMediaTrack)]
pub struct JsTrack(Track<SharedPtr>);

impl JsTrack {
    pub fn new(track: Track<SharedPtr>) -> Self {
        JsTrack(track)
    }
}

#[wasm_bindgen(js_class = LocalMediaTrack)]
impl JsTrack {
    /// Returns underlying [`SysMediaStreamTrack`] from this
    /// [`MediaStreamTrack`].
    pub fn get_track(&self) -> SysMediaStreamTrack {
        Clone::clone(self.0.as_ref())
    }

    /// Returns a [`String`] set to `audio` if the track is an audio track
    /// and to `video`, if it is a video track.
    #[wasm_bindgen(js_name = kind)]
    pub fn js_kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns a [`String`] set to `device` if track is sourced from some
    /// device (webcam/microphone) and to `display`, if track is captured
    /// via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://tinyurl.com/y2anfntz
    #[wasm_bindgen(js_name = media_source_kind)]
    pub fn js_media_source_kind(&self) -> JsMediaSourceKind {
        self.0.source_kind().into()
    }
}
