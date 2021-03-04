//! Wrapper around [`sys::MediaStreamTrack`] received from
//! [getUserMedia()][1]/[getDisplayMedia()][2] request.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
//! [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia

use std::rc::Rc;

use medea_client_api_proto::MediaSourceKind;
use wasm_bindgen::prelude::*;
use web_sys as sys;

use crate::{media::MediaKind, JsMediaSourceKind};

/// Wrapper around [`sys::MediaStreamTrack`] received from from
/// [getUserMedia()][1]/[getDisplayMedia()][2] request.
///
/// Underlying [`sys::MediaStreamTrack`] is stopped on this [`Track`]'s
/// [`Drop`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(Debug)]
pub struct Track {
    /// Actual [`sys::MediaStreamTrack`].
    track: sys::MediaStreamTrack,

    /// Underlying [`sys::MediaStreamTrack`] source kind.
    source_kind: MediaSourceKind,

    /// Underlying [`sys::MediaStreamTrack`] kind.
    kind: MediaKind,

    /// Reference to the parent [`Track`].
    ///
    /// Parent will be [`None`] if this [`Track`] wasn't forked from another
    /// [`Track`].
    ///
    /// This field is used only for holding strong reference to the parent.
    _parent: Option<Rc<Self>>,
}

impl Track {
    /// Builds new [`Track`] from the provided [`sys::MediaStreamTrack`] and
    /// [`MediaSourceKind`].
    #[must_use]
    pub fn new(
        track: sys::MediaStreamTrack,
        source_kind: MediaSourceKind,
    ) -> Self {
        let kind = match track.kind().as_ref() {
            "audio" => MediaKind::Audio,
            "video" => MediaKind::Video,
            _ => unreachable!(),
        };
        Self {
            track,
            source_kind,
            kind,
            _parent: None,
        }
    }

    /// Changes [`enabled`][1] attribute on the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn set_enabled(&self, enabled: bool) {
        self.track.set_enabled(enabled);
    }

    /// Returns [`id`] of underlying [MediaStreamTrack][2].
    ///
    /// [`id`]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    #[must_use]
    pub fn id(&self) -> String {
        self.track.id()
    }

    /// Returns this [`Track`]'s media source kind.
    #[inline]
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.source_kind
    }

    /// Returns this [`Track`]'s kind (audio/video).
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Forks this [`Track`].
    ///
    /// Creates new [`sys::MediaStreamTrack`] from this [`Track`]'s
    /// [`sys::MediaStreamTrack`] using [`clone()`][1] method.
    ///
    /// Forked [`Track`] will hold a strong reference to this [`Track`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-clone
    #[must_use]
    pub fn fork(self: &Rc<Self>) -> Self {
        let parent = Rc::clone(self);
        let track = sys::MediaStreamTrack::clone(&self.track);
        Self {
            track,
            kind: self.kind,
            source_kind: self.source_kind,
            _parent: Some(parent),
        }
    }

    /// Returns reference to the underlying [`sys::MediaStreamTrack`] of this
    /// [`Track`].
    #[inline]
    #[must_use]
    pub fn sys_track(&self) -> &sys::MediaStreamTrack {
        &self.track
    }
}

impl Drop for Track {
    #[inline]
    fn drop(&mut self) {
        self.track.stop();
    }
}

/// Wrapper around strongly referenced [`Track`] for JS side.
#[wasm_bindgen(js_name = LocalMediaTrack)]
pub struct JsTrack(Rc<Track>);

impl JsTrack {
    /// Creates new [`JsTrack`] from the provided [`Track`].
    #[inline]
    #[must_use]
    pub fn new(track: Rc<Track>) -> Self {
        JsTrack(track)
    }
}

#[wasm_bindgen(js_class = LocalMediaTrack)]
impl JsTrack {
    /// Returns the underlying [`sys::MediaStreamTrack`] of this [`JsTrack`].
    #[must_use]
    pub fn get_track(&self) -> sys::MediaStreamTrack {
        Clone::clone(self.0.track.as_ref())
    }

    /// Returns [`MediaKind::Audio`] if this [`JsTrack`] represents an audio
    /// track, or [`MediaKind::Video`] if it represents a video track.
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns [`JsMediaSourceKind::Device`] if this [`JsTrack`] is sourced
    /// from some device (webcam/microphone), or [`JsMediaSourceKind::Display`]
    /// if ot is captured via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[must_use]
    pub fn media_source_kind(&self) -> JsMediaSourceKind {
        self.0.media_source_kind().into()
    }
}
