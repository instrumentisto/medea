//! [MediaStream][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream

use std::{collections::HashMap, rc::Rc};

use medea_client_api_proto::TrackId;
use web_sys::MediaStream as SysMediaStream;

use crate::media::TrackConstraints;

use super::MediaTrack;

/// Actual data of a [`MediaStream`].
///
/// Shared between JS side ([`MediaStreamHandle`]) and Rust side
/// ([`MediaStream`]).
struct InnerStream {
    /// Actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    stream: SysMediaStream,

    /// List of audio tracks.
    audio_tracks: HashMap<TrackId, Rc<MediaTrack>>,

    /// List of video tracks.
    video_tracks: HashMap<TrackId, Rc<MediaTrack>>,
}

impl InnerStream {
    /// Instantiates new [`InnerStream`].
    fn new() -> Self {
        Self {
            stream: SysMediaStream::new().unwrap(),
            audio_tracks: HashMap::new(),
            video_tracks: HashMap::new(),
        }
    }

    /// Adds provided [`MediaTrack`] to a stream.
    fn add_track(&mut self, track: Rc<MediaTrack>) {
        self.stream.add_track(track.track());
        let caps = track.caps();
        match caps {
            TrackConstraints::Audio(_) => {
                self.audio_tracks.insert(track.id(), track);
            }
            TrackConstraints::Video(_) => {
                self.video_tracks.insert(track.id(), track);
            }
        }
    }
}

/// Representation of [MediaStream][1] object.
///
/// It's used on Rust side and represents a handle to [`InnerStream`] data.
///
/// For using [`MediaStream`] on JS side, consider the [`MediaStreamHandle`].
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
#[allow(clippy::module_name_repetitions)]
pub struct MediaStream(Rc<InnerStream>);

impl MediaStream {
    /// Creates new [`MediaStream`] from a given collection of [`MediaTrack`]s.
    pub fn from_tracks<I>(tracks: I) -> Self
    where
        I: IntoIterator<Item = Rc<MediaTrack>>,
    {
        let mut stream = InnerStream::new();
        for track in tracks {
            stream.add_track(track);
        }
        Self(Rc::new(stream))
    }

    /// Checks if [`MediaStream`] contains a [`MediaTrack`] with specified ID.
    pub fn has_track(&self, id: TrackId) -> bool {
        self.0.video_tracks.contains_key(&id)
            || self.0.audio_tracks.contains_key(&id)
    }

    /// Returns a [`MediaTrack`] of [`MediaStream`] by its ID, if any.
    pub fn get_track_by_id(&self, track_id: TrackId) -> Option<Rc<MediaTrack>> {
        match self.0.video_tracks.get(&track_id) {
            Some(track) => Some(Rc::clone(track)),
            None => match self.0.audio_tracks.get(&track_id) {
                Some(track) => Some(Rc::clone(track)),
                None => None,
            },
        }
    }

    /// Enables or disables all audio [`MediaTrack`]s in this stream.
    pub fn toggle_audio_tracks(&self, enabled: bool) {
        for track in self.0.audio_tracks.values() {
            track.set_enabled(enabled);
        }
    }

    /// Enables or disables all video [`MediaTrack`]s in this stream.
    pub fn toggle_video_tracks(&self, enabled: bool) {
        for track in self.0.video_tracks.values() {
            track.set_enabled(enabled);
        }
    }

    /// Returns actual underlying [MediaStream][1] object.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastream
    pub fn stream(&self) -> SysMediaStream {
        Clone::clone(&self.0.stream)
    }
}
