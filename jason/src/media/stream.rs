//! [MediaStream][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#mediastream

use std::rc::{Rc, Weak};

use derive_more::{AsRef, Display};
use wasm_bindgen::prelude::*;
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

use crate::MediaStreamSettings;
use futures::Stream;
use medea_client_api_proto::{MediaType, TrackId};

/// Representation of [MediaStream][1] object. Contains strong references to
/// [`MediaStreamTrack`].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
#[wasm_bindgen(js_name = LocalMediaStream)]
#[derive(AsRef, Clone)]
pub struct MediaStream {
    #[as_ref]
    stream: SysMediaStream,
    constraints: MediaStreamSettings,
    tracks: Vec<MediaStreamTrack>,
}

impl MediaStream {
    /// Creates new [`MediaStream`] from provided tracks and stream settings.
    pub fn new(
        tracks: Vec<MediaStreamTrack>,
        constraints: MediaStreamSettings,
    ) -> Self {
        let stream = SysMediaStream::new().unwrap();
        tracks
            .iter()
            .for_each(|track| stream.add_track(track.as_ref()));

        MediaStream {
            stream,
            constraints,
            tracks,
        }
    }

    /// Consumes `self` returning all underlying [`MediaStreamTrack`]s.
    pub fn into_tracks(self) -> Vec<MediaStreamTrack> {
        self.tracks
    }
}

#[wasm_bindgen(js_class = LocalMediaStream)]
impl MediaStream {
    /// Returns underlying [MediaStream][1].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#mediastream
    pub fn get_media_stream(&self) -> SysMediaStream {
        Clone::clone(&self.stream)
    }

    /// Drops all audio tracks contained in ths stream.
    pub fn free_audio(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => false,
            TrackKind::Video => true,
        });
    }

    /// Drops all video tracks contained in ths stream.
    pub fn free_video(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => true,
            TrackKind::Video => false,
        });
    }
}

/// Weak reference to [MediaStreamTrack][1].
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
pub struct WeakMediaStreamTrack(Weak<SysMediaStreamTrack>);

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

/// Strong reference to [MediaStreamTrack][1].
///
/// Track will be automatically stopped when there are no strong references
/// left.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
// TODO: remove pub on field
#[derive(Clone)]
pub struct MediaStreamTrack(pub Rc<SysMediaStreamTrack>);

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[derive(Clone, Copy, Display, Eq, PartialEq)]
pub enum TrackKind {
    /// Audio track.
    #[display(fmt = "audio")]
    Audio,

    /// Video track.
    #[display(fmt = "video")]
    Video,
}

impl From<TrackKind> for JsValue {
    fn from(from: TrackKind) -> Self {
        JsValue::from(from.to_string())
    }
}

impl From<&MediaType> for TrackKind {
    fn from(from: &MediaType) -> Self {
        match from {
            MediaType::Audio(_) => Self::Audio,
            MediaType::Video(_) => Self::Video,
        }
    }
}

impl<T> From<T> for MediaStreamTrack
where
    SysMediaStreamTrack: From<T>,
{
    #[inline]
    fn from(track: T) -> Self {
        MediaStreamTrack(Rc::new(<SysMediaStreamTrack as From<T>>::from(track)))
    }
}

impl AsRef<SysMediaStreamTrack> for MediaStreamTrack {
    #[inline]
    fn as_ref(&self) -> &SysMediaStreamTrack {
        &self.0
    }
}

impl MediaStreamTrack {
    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn id(&self) -> String {
        self.0.id()
    }

    /// Returns track kind (audio/video).
    #[inline]
    pub fn kind(&self) -> TrackKind {
        match self.0.kind().as_ref() {
            "audio" => TrackKind::Audio,
            "video" => TrackKind::Video,
            _ => unreachable!(),
        }
    }

    /// Creates weak reference to underlying [MediaStreamTrack][2].
    ///
    /// [2]: https://w3.org/TR/mediacapture-streams/#mediastreamtrack
    #[inline]
    pub fn downgrade(&self) -> WeakMediaStreamTrack {
        WeakMediaStreamTrack(Rc::downgrade(&self.0))
    }
}

impl Drop for MediaStreamTrack {
    #[inline]
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        if Rc::strong_count(&self.0) == 1 {
            self.0.stop();
        }
    }
}
