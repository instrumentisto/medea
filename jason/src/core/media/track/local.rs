//! Wrapper around [`platform::MediaStreamTrack`] received from
//! [getUserMedia()][1]/[getDisplayMedia()][2] request.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
//! [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia

use std::rc::Rc;

use derive_more::AsRef;
use medea_client_api_proto as proto;

use crate::{
    core::media::{MediaKind, MediaSourceKind},
    platform,
};

/// Wrapper around [`platform::MediaStreamTrack`] received from from
/// [getUserMedia()][1]/[getDisplayMedia()][2] request.
///
/// Underlying [`platform::MediaStreamTrack`] is stopped on this [`Track`]'s
/// [`Drop`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(Debug, AsRef)]
pub struct Track {
    /// Actual [`platform::MediaStreamTrack`].
    #[as_ref]
    track: platform::MediaStreamTrack,

    /// Underlying [`platform::MediaStreamTrack`] source kind.
    source_kind: proto::MediaSourceKind,

    /// Reference to the parent [`Track`].
    ///
    /// Parent will be [`None`] if this [`Track`] wasn't forked from another
    /// [`Track`].
    ///
    /// This field is used only for holding strong reference to the parent.
    _parent: Option<Rc<Self>>,
}

impl Track {
    /// Builds new [`Track`] from the provided [`platform::MediaStreamTrack`]
    /// and [`proto::MediaSourceKind`].
    #[must_use]
    pub fn new(
        track: platform::MediaStreamTrack,
        source_kind: proto::MediaSourceKind,
    ) -> Self {
        Self {
            track,
            source_kind,
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
    pub fn media_source_kind(&self) -> proto::MediaSourceKind {
        self.source_kind
    }

    /// Returns this [`Track`]'s kind (audio/video).
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.track.kind()
    }

    /// Forks this [`Track`].
    ///
    /// Creates new [`platform::MediaStreamTrack`] from this [`Track`]'s
    /// [`platform::MediaStreamTrack`] using [`clone()`][1] method.
    ///
    /// Forked [`Track`] will hold a strong reference to this [`Track`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-clone
    #[must_use]
    pub fn fork(self: &Rc<Self>) -> Self {
        let parent = Rc::clone(self);
        let track = platform::MediaStreamTrack::clone(&self.track);
        Self {
            track,
            source_kind: self.source_kind,
            _parent: Some(parent),
        }
    }
}

impl Drop for Track {
    #[inline]
    fn drop(&mut self) {
        self.track.stop();
    }
}

/// Wrapper around strongly referenced [`Track`] for JS side.
pub struct LocalMediaTrack(Rc<Track>);

impl LocalMediaTrack {
    /// Creates new [`LocalMediaTrack`] from the provided [`Track`].
    #[inline]
    #[must_use]
    pub fn new(track: Rc<Track>) -> Self {
        LocalMediaTrack(track)
    }

    /// Returns the underlying [`platform::MediaStreamTrack`] of this
    /// [`LocalMediaTrack`].
    pub fn get_track(&self) -> &platform::MediaStreamTrack {
        &self.0.track
    }

    /// Returns [`MediaKind::Audio`] if this [`LocalMediaTrack`] represents an
    /// audio track, or [`MediaKind::Video`] if it represents a video track.
    pub fn kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns [`MediaSourceKind::Device`] if this [`LocalMediaTrack`] is
    /// sourced from some device (webcam/microphone), or
    /// [`MediaSourceKind::Display`] if ot is captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind().into()
    }
}
