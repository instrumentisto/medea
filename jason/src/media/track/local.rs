//! Implementation of the wrappers around [`SysMediaStreamTrack`] received from
//! the gUM/gDM request.

use std::rc::{Rc, Weak};

use derive_more::AsRef;
use medea_client_api_proto::MediaSourceKind;
use wasm_bindgen::prelude::*;
use web_sys::MediaStreamTrack as SysMediaStreamTrack;

use crate::{media::MediaKind, JsMediaSourceKind};

/// [`Weak`] reference to the [`InnerTrack`].
#[derive(Clone, Debug)]
pub struct WeakPtr(Weak<InnerTrack>);

/// [`Rc`] reference to the [`InnerTrack`].
#[derive(Clone, Debug)]
pub struct SharedPtr(Rc<InnerTrack>);

/// Wrapper around [`SysMediaStreamTrack`] received from the gUM/gDM request.
///
/// Can be forked by calling [`Track::fork`].
///
/// This [`Track`] will be stopped when all references to this [`Track`] and
/// forked [`Track`]s will be dropped.
#[derive(Clone, Debug)]
pub struct Track<S> {
    /// Underlying [`SysMediaStreamTrack`] source kind.
    source_kind: MediaSourceKind,

    /// Underlying [`SysMediaStreamTrack`] kind.
    kind: MediaKind,

    /// Actual reference to the [`SysMediaStreamTrack`].
    track: S,
}

impl Track<WeakPtr> {
    /// Checks whether this weak reference can be upgraded to a strong one.
    #[inline]
    pub fn can_be_upgraded(&self) -> bool {
        self.track.0.strong_count() > 0
    }

    /// Upgrades [`Weak`] referenced [`Track`] to the [`Rc`] referenced
    /// [`Track`].
    ///
    /// Returns [`None`] if [`Weak`] reference can't be upgraded.
    pub fn upgrade(&self) -> Option<Track<SharedPtr>> {
        Some(Track {
            track: SharedPtr(Weak::upgrade(&self.track.0)?),
            kind: self.kind,
            source_kind: self.source_kind,
        })
    }
}

impl Track<SharedPtr> {
    /// Returns new [`Rc`] referenced [`Track`].
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

    /// Sets [`SysMediaStreamTrack`] `enabled` field to the provided one.
    pub fn set_enabled(&self, enabled: bool) {
        self.track.0.as_ref().as_ref().set_enabled(enabled);
    }

    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn id(&self) -> String {
        self.track.0.id()
    }

    /// Returns this [`Track`] media source kind.
    pub fn source_kind(&self) -> MediaSourceKind {
        self.source_kind
    }

    /// Returns track kind (audio/video).
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Returns fork of this [`Track`].
    ///
    /// Returned [`Track`] is identical except for its unique id.
    ///
    /// Forked [`Track`] will hold reference to it's parent, so this [`Track`]
    /// can't be dropped until all his forked [`Track`]s will be dropped.
    ///
    /// You can change properties of the forked [`Track`] without affecting the
    /// original one.
    pub fn fork(&self) -> Self {
        Self {
            track: SharedPtr(self.track.0.fork()),
            source_kind: self.source_kind,
            kind: self.kind,
        }
    }

    /// Downgrades [`Rc`] referenced [`Track`] to the [`Weak`] referenced
    /// [`Track`].
    pub fn downgrade(&self) -> Track<WeakPtr> {
        Track {
            track: WeakPtr(Rc::downgrade(&self.track.0)),
            kind: self.kind,
            source_kind: self.source_kind,
        }
    }
}

impl AsRef<SysMediaStreamTrack> for Track<SharedPtr> {
    fn as_ref(&self) -> &SysMediaStreamTrack {
        self.track.0.as_ref().as_ref()
    }
}

/// Actual [`SysMediaStreamTrack`] with a strong reference to the it's parent
/// [`InnerTrack`].
///
/// When this [`InnerTrack`] is dropped - [`SysMediaStreamTrack::stop`] will be
/// performed.
///
/// Note that all childs of this [`InnerTrack`] will hold strong reference to
/// this [`InnerTrack`], so it can be dropped only when all it's childs was
/// dropped.
#[derive(AsRef, Clone, Debug)]
struct InnerTrack {
    /// Reference to the parent [`InnerTrack`].
    ///
    /// Parent will be [`None`] if this [`InnerTrack`] wasn't forked from
    /// another [`InnerTrack`].
    ///
    /// This field used only for holding strong reference to the parent.
    _parent: Option<Rc<InnerTrack>>,

    /// Actual [`SysMediaStreamTrack`].
    #[as_ref]
    track: SysMediaStreamTrack,
}

impl InnerTrack {
    /// Returns new root [`InnerTrack`] with a provided [`SysMediaStreamTrack`]
    /// as underlying track.
    fn new(track: SysMediaStreamTrack) -> Rc<Self> {
        Rc::new(Self {
            _parent: None,
            track,
        })
    }

    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    fn id(&self) -> String {
        self.track.id()
    }

    /// Returns fork of this [`InnerTrack`].
    ///
    /// Returned [`InnerTrack`] is identical except for its unique id.
    ///
    /// Forked [`InnerTrack`] will hold reference to it's parent, so this
    /// [`InnerTrack`] can't be dropped until all his forked [`InnerTrack`]s
    /// will be dropped.
    ///
    /// You can change properties of the forked [`InnerTrack`] without affecting
    /// the original one.
    fn fork(self: &Rc<Self>) -> Rc<Self> {
        let parent = Rc::clone(self);
        let track = SysMediaStreamTrack::clone(&self.track);
        Rc::new(Self {
            _parent: Some(parent),
            track,
        })
    }
}

impl Drop for InnerTrack {
    fn drop(&mut self) {
        self.track.stop();
    }
}

/// Wrapper around strongly referenced [`Track`] for the JS side.
#[wasm_bindgen(js_name = LocalMediaTrack)]
pub struct JsTrack(Track<SharedPtr>);

impl JsTrack {
    /// Wraps provided [`Track`] to the [`JsTrack`].
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
    pub fn kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns a [`String`] set to `device` if track is sourced from some
    /// device (webcam/microphone) and to `display`, if track is captured
    /// via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://tinyurl.com/y2anfntz
    pub fn media_source_kind(&self) -> JsMediaSourceKind {
        self.0.source_kind().into()
    }
}
