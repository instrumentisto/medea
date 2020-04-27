//! [MediaStream][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::rc::{Rc, Weak};

use crate::MediaStreamSettings;

use wasm_bindgen::prelude::*;
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

/// Representation of [MediaStream][1] object. Contains strong references to
/// [`MediaStreamTrack`].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
#[wasm_bindgen(js_name = LocalMediaStream)]
#[derive(Clone)]
pub struct MediaStream {
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
        for track in &self.tracks {
            self.stream.remove_track(track.as_ref());
        }
        self.tracks
    }
}

#[wasm_bindgen(js_class = LocalMediaStream)]
impl MediaStream {
    /// Returns underlying [MediaStream][1].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub fn get_media_stream(&self) -> SysMediaStream {
        Clone::clone(&self.stream)
    }

    /// Drops all audio tracks contained in ths stream.
    pub fn free_audio(&mut self) {
        let stream = Clone::clone(&self.stream);
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => {
                stream.remove_track(track.as_ref());
                false
            }
            TrackKind::Video => true,
        });
    }

    /// Drops all video tracks contained in ths stream.
    pub fn free_video(&mut self) {
        let stream = Clone::clone(&self.stream);
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => true,
            TrackKind::Video => {
                stream.remove_track(track.as_ref());
                false
            }
        });
    }
}

impl AsRef<SysMediaStream> for MediaStream {
    fn as_ref(&self) -> &SysMediaStream {
        &self.stream
    }
}

/// Weak reference to [MediaStreamTrack][1].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
pub struct WeakMediaStreamTrack(Weak<SysMediaStreamTrack>);

impl WeakMediaStreamTrack {
    pub fn upgrade(&self) -> Option<MediaStreamTrack> {
        self.0.upgrade().map(MediaStreamTrack)
    }

    pub fn can_be_upgraded(&self) -> bool {
        self.0.strong_count() > 0
    }
}

/// Strong reference to [MediaStreamTrack][1].
///
/// Track will be automatically stopped when there are no strong references
/// left.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamtrack
#[derive(Clone)]
pub struct MediaStreamTrack(Rc<SysMediaStreamTrack>);

/// [MediaStreamTrack.kind][1] representation.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-kind
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum TrackKind {
    /// Audio track.
    Audio,

    /// Video track.
    Video,
}

impl<T> From<T> for MediaStreamTrack
where
    SysMediaStreamTrack: From<T>,
{
    fn from(track: T) -> Self {
        MediaStreamTrack(Rc::new(<SysMediaStreamTrack as From<T>>::from(track)))
    }
}

impl AsRef<SysMediaStreamTrack> for MediaStreamTrack {
    fn as_ref(&self) -> &SysMediaStreamTrack {
        &self.0
    }
}

impl MediaStreamTrack {
    /// Returns [`id`][1] of underlying [MediaStreamTrack][2].
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamtrack-id
    /// [2]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn id(&self) -> String {
        self.0.id()
    }

    /// Returns track kind (audio/video).
    pub fn kind(&self) -> TrackKind {
        match self.0.kind().as_ref() {
            "audio" => TrackKind::Audio,
            "video" => TrackKind::Video,
            _ => unreachable!(),
        }
    }

    /// Creates weak reference to underlying track.
    pub fn downgrade(&self) -> WeakMediaStreamTrack {
        WeakMediaStreamTrack(Rc::downgrade(&self.0))
    }
}

impl Drop for MediaStreamTrack {
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        if Rc::strong_count(&self.0) == 1 {
            self.0.stop();
        }
    }
}
