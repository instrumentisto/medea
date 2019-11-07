//! [MediaStreamConstraints][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints

use std::{collections::HashMap, convert::TryFrom};

use medea_client_api_proto::TrackId;
use thiserror::*;
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

use crate::media::{
    AudioTrackConstraints, MediaStreamConstraints, TrackConstraints,
    VideoTrackConstraints,
};

use super::{MediaStream, MediaTrack};

/// Errors that may occur when validating [`StreamRequest`] or
/// parsing [`MediaStream`].
#[derive(Debug, Error)]
pub enum Error {
    /// [`StreamRequest`] contains multiple [`AudioTrackConstraints`].
    #[error("only one audio track is  allowed in SimpleStreamRequest")]
    TooManyAudioTracks,

    /// [`StreamRequest`] contains multiple [`VideoTrackConstraints`].
    #[error("only one video track is allowed in SimpleStreamRequest")]
    TooManyVideoTracks,

    /// [`StreamRequest`] contains no track constraints at all.
    #[error("SimpleStreamRequest should have at least one track")]
    NoTracks,

    /// Provided [`MediaStream`] has multiple audio [`MediaTrack`]s.
    #[error("provided MediaStream was expected to have single audio track")]
    ExpectedAudioTracks,

    /// Provided [`MediaStream`] has multiple video [`MediaTrack`]s.
    #[error("provided MediaStream was expected to have single video track")]
    ExpectedVideoTracks,

    /// Audio [`MediaTrack`] fails to satisfy specified constraints.
    #[error("provided audio track does not satisfy specified constraints")]
    InvalidAudioTrack,

    /// Video [`MediaTrack`] fails to satisfy specified constraints.
    #[error("provided video track does not satisfy specified constraints")]
    InvalidVideoTrack,
}

/// Representation of [MediaStreamConstraints][1] object.
///
/// It's used for invoking [getUserMedia()][2] to specify what kinds of tracks
/// should be included into returned [`MediaStream`], and, optionally,
/// to establish constraints for those [`MediaTrack`]'s settings.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
/// [2]:
/// https://www.w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [3]: https://www.w3.org/TR/mediacapture-streams/#mediastream
#[derive(Default)]
pub struct StreamRequest {
    audio: HashMap<TrackId, AudioTrackConstraints>,
    video: HashMap<TrackId, VideoTrackConstraints>,
}

impl StreamRequest {
    /// Adds track request to this [`StreamRequest`].
    pub fn add_track_request<T: Into<TrackConstraints>>(
        &mut self,
        track_id: TrackId,
        caps: T,
    ) {
        match caps.into() {
            TrackConstraints::Audio(audio) => {
                self.audio.insert(track_id, audio);
            }
            TrackConstraints::Video(video) => {
                self.video.insert(track_id, video);
            }
        }
    }
}

/// Subtype of [`StreamRequest`], which can have maximum one track of each kind
/// and must have at least one track of any kind.
#[allow(clippy::module_name_repetitions)]
pub struct SimpleStreamRequest {
    audio: Option<(TrackId, AudioTrackConstraints)>,
    video: Option<(TrackId, VideoTrackConstraints)>,
}

impl SimpleStreamRequest {
    /// Parses raw [`SysMediaStream`] and returns [`MediaStream`].
    pub fn parse_stream(
        &self,
        stream: &SysMediaStream,
    ) -> Result<MediaStream, Error> {
        use Error::*;

        let mut tracks = Vec::new();

        if let Some((id, audio)) = &self.audio {
            let audio_tracks: Vec<_> =
                js_sys::try_iter(&stream.get_audio_tracks())
                    .unwrap()
                    .unwrap()
                    .map(|tr| SysMediaStreamTrack::from(tr.unwrap()))
                    .collect();

            if audio_tracks.len() == 1 {
                let track = audio_tracks.into_iter().next().unwrap();
                if audio.satisfies(&track) {
                    tracks.push(MediaTrack::new(
                        *id,
                        track,
                        TrackConstraints::Audio(audio.clone()),
                    ))
                } else {
                    return Err(InvalidAudioTrack);
                }
            } else {
                return Err(ExpectedAudioTracks);
            }
        }

        if let Some((id, video)) = &self.video {
            let video_tracks: Vec<_> =
                js_sys::try_iter(&stream.get_video_tracks())
                    .unwrap()
                    .unwrap()
                    .map(|tr| SysMediaStreamTrack::from(tr.unwrap()))
                    .collect();

            if video_tracks.len() == 1 {
                let track = video_tracks.into_iter().next().unwrap();
                if video.satisfies(&track) {
                    tracks.push(MediaTrack::new(
                        *id,
                        track,
                        TrackConstraints::Video(video.clone()),
                    ))
                } else {
                    return Err(InvalidVideoTrack);
                }
            } else {
                return Err(ExpectedVideoTracks);
            }
        }

        Ok(MediaStream::from_tracks(tracks))
    }
}

impl TryFrom<StreamRequest> for SimpleStreamRequest {
    type Error = Error;

    fn try_from(value: StreamRequest) -> Result<Self, Self::Error> {
        use Error::*;

        if value.video.len() > 1 {
            return Err(TooManyVideoTracks);
        } else if value.audio.len() > 1 {
            return Err(TooManyAudioTracks);
        } else if value.video.is_empty() && value.audio.is_empty() {
            return Err(NoTracks);
        }

        let mut req = Self {
            audio: None,
            video: None,
        };
        for (id, audio) in value.audio {
            req.audio.replace((id, audio));
        }
        for (id, video) in value.video {
            req.video.replace((id, video));
        }
        Ok(req)
    }
}

impl From<&SimpleStreamRequest> for MediaStreamConstraints {
    fn from(request: &SimpleStreamRequest) -> Self {
        let mut constraints = Self::new();

        if let Some((_, audio)) = &request.audio {
            constraints.audio(audio.clone());
        }
        if let Some((_, video)) = &request.video {
            constraints.video(video.clone());
        }

        constraints
    }
}
