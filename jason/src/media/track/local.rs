//! Implementation of the wrapper around [`sys::MediaStreamTrack`] received from
//! the gUM/gDM request.

use std::rc::Rc;

use medea_client_api_proto::MediaSourceKind;
use wasm_bindgen::prelude::*;
use web_sys as sys;

use crate::{media::MediaKind, JsMediaSourceKind};

/// Wrapper around [`sys::MediaStreamTrack`] received from the gUM/gDM request.
///
/// Underlying [`sys::MediaStreamTrack`] is stopped on [`Track`] [`Drop`].
#[derive(Debug)]
pub struct Track {
    /// Reference to the parent [`Track`].
    ///
    /// Parent will be [`None`] if this [`Track`] wasn't forked from another
    /// [`Track`].
    ///
    /// This field used only for holding strong reference to the parent.
    _parent: Option<Rc<Self>>,

    /// Actual [`sys::MediaStreamTrack`].
    track: sys::MediaStreamTrack,

    /// Underlying [`sys::MediaStreamTrack`] source kind.
    source_kind: MediaSourceKind,

    /// Underlying [`sys::MediaStreamTrack`] kind.
    kind: MediaKind,
}

impl Track {
    /// Builds [`Track`] from provided [`sys::MediaStreamTrack`] and
    /// [`MediaSourceKind`].
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
            _parent: None,
            track,
            source_kind,
            kind,
        }
    }

    /// Changes [`enabled`][1] attribute on the underlying
    /// [MediaStreamTrack][2].
    ///
    /// [1]: https://tinyurl.com/w3-streams#dom-mediastreamtrack-enabled
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn set_enabled(&self, enabled: bool) {
        self.track.set_enabled(enabled);
    }

    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn id(&self) -> String {
        self.track.id()
    }

    /// Returns this [`Track`] media source kind.
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.source_kind
    }

    /// Returns [`Track`] kind (audio/video).
    pub fn kind(&self) -> MediaKind {
        self.kind
    }

    /// Forks this [`Track`].
    ///
    /// Creates new [`sys::MediaStreamTrack`] from this [`Track`]
    /// [`sys::MediaStreamTrack`] using [`clone`][1] method.
    ///
    /// Forked [`Track`] holds strong reference to this [`Track`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-clone
    pub fn fork(self: &Rc<Self>) -> Self {
        let parent = Rc::clone(self);
        let track = sys::MediaStreamTrack::clone(&self.track);
        Self {
            _parent: Some(parent),
            track,
            kind: self.kind,
            source_kind: self.source_kind,
        }
    }

    /// Returns reference to the [`sys::MediaStreamTrack`].
    pub fn sys_track(&self) -> &sys::MediaStreamTrack {
        &self.track
    }
}

impl Drop for Track {
    fn drop(&mut self) {
        self.track.stop();
    }
}

/// Wrapper around strongly referenced [`Track`] for the JS side.
#[wasm_bindgen(js_name = LocalMediaTrack)]
pub struct JsTrack(Rc<Track>);

impl JsTrack {
    /// Creates new [`JsTrack`] from provided [`Track`].
    pub fn new(track: Rc<Track>) -> Self {
        JsTrack(track)
    }
}

#[wasm_bindgen(js_class = LocalMediaTrack)]
impl JsTrack {
    /// Returns underlying [`sys::MediaStreamTrack`] from this [`JsTrack`].
    pub fn get_track(&self) -> sys::MediaStreamTrack {
        Clone::clone(self.0.track.as_ref())
    }

    /// Returns a [`MediaKind::Audio`] if the track is an audio track and to
    /// [`MediaKind::Video`], if it is a video track.
    pub fn kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns [`JsMediaSourceKind::Device`] if track is sourced from some
    /// device (webcam/microphone) and [`JsMediaSourceKind::Display`], if track
    /// is captured via [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://tinyurl.com/y2anfntz
    pub fn media_source_kind(&self) -> JsMediaSourceKind {
        self.0.media_source_kind().into()
    }
}
