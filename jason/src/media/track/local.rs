//! Wrapper around a [`platform::MediaStreamTrack`] received from a
//! [getUserMedia()][1]/[getDisplayMedia()][2] request.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
//! [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia

use std::rc::Rc;

use derive_more::AsRef;
use medea_client_api_proto as proto;

use crate::{
    media::{MediaKind, MediaSourceKind},
    platform,
};

/// Wrapper around a [`platform::MediaStreamTrack`] received from a
/// [getUserMedia()][1]/[getDisplayMedia()][2] request.
///
/// Underlying [`platform::MediaStreamTrack`] is stopped on this [`Track`]'s
/// [`Drop`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
#[derive(AsRef, Debug)]
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
    /// Builds a new [`Track`] from the provided [`sys::MediaStreamTrack`] and
    /// [`proto::MediaSourceKind`].
    #[inline]
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
    /// Creates a new [`Track`] from this [`Track`]'s
    /// [`platform::MediaStreamTrack`] using a [`clone()`][1] method.
    ///
    /// Forked [`Track`] will hold a strong reference to this [`Track`].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-clone
    #[must_use]
    pub fn fork(self: &Rc<Self>) -> Self {
        let parent = Rc::clone(self);
        let track = self.track.fork();
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

/// Strongly referenced [`Track`] received from a
/// [getUserMedia()][1]/[getDisplayMedia()][2] request.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [2]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
pub struct LocalMediaTrack(Rc<Track>);

impl LocalMediaTrack {
    /// Createsa  new [`LocalMediaTrack`] from the provided [`Track`].
    #[inline]
    #[must_use]
    pub fn new(track: Rc<Track>) -> Self {
        LocalMediaTrack(track)
    }

    /// Returns the underlying [`platform::MediaStreamTrack`] of this
    /// [`LocalMediaTrack`].
    #[inline]
    #[must_use]
    pub fn get_track(&self) -> &platform::MediaStreamTrack {
        &self.0.track
    }

    /// Returns a [`MediaKind::Audio`] if this [`LocalMediaTrack`] represents an
    /// audio track, or a [`MediaKind::Video`] if it represents a video track.
    #[inline]
    #[must_use]
    pub fn kind(&self) -> MediaKind {
        self.0.kind()
    }

    /// Returns a [`MediaSourceKind::Device`] if this [`LocalMediaTrack`] is
    /// sourced from some device (webcam/microphone), or
    /// a [`MediaSourceKind::Display`] if it's captured via
    /// [MediaDevices.getDisplayMedia()][1].
    ///
    /// [1]: https://w3.org/TR/screen-capture/#dom-mediadevices-getdisplaymedia
    #[inline]
    #[must_use]
    pub fn media_source_kind(&self) -> MediaSourceKind {
        self.0.media_source_kind().into()
    }
}
