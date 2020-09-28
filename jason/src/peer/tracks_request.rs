//! [MediaStreamConstraints][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints

use std::{collections::HashMap, convert::TryFrom};

use derive_more::Display;
use medea_client_api_proto::TrackId;
use tracerr::Traced;

use crate::{
    media::{
        AudioTrackConstraints, MediaStreamSettings, MediaStreamTrack,
        TrackConstraints, TrackKind, VideoTrackConstraints,
    },
    utils::{JsCaused, JsError},
};

/// Errors that may occur when validating [`TracksRequest`] or
/// parsing [`MediaStreamTrack`]s.
#[derive(Debug, Display, JsCaused)]
pub enum TracksRequestError {
    /// [`TracksRequest`] contains multiple [`AudioTrackConstraints`].
    #[display(fmt = "only one audio track is allowed in SimpleTracksRequest")]
    TooManyAudioTracks,

    /// [`TracksRequest`] contains multiple [`VideoTrackConstraints`].
    #[display(fmt = "only one video track is allowed in SimpleTracksRequest")]
    TooManyVideoTracks,

    /// [`TracksRequest`] contains no track constraints at all.
    #[display(fmt = "SimpleTracksRequest should have at least one track")]
    NoTracks,

    /// Provided multiple audio [`MediaStreamTrack`]s.
    #[display(fmt = "provided multiple audio MediaStreamTracks")]
    ExpectedAudioTracks,

    /// Provided multiple video [`MediaStreamTrack`]s.
    #[display(fmt = "provided multiple video MediaStreamTracks")]
    ExpectedVideoTracks,

    /// Audio track fails to satisfy specified constraints.
    #[display(
        fmt = "provided audio track does not satisfy specified constraints"
    )]
    InvalidAudioTrack,

    /// Video track fails to satisfy specified constraints.
    #[display(
        fmt = "provided video track does not satisfy specified constraints"
    )]
    InvalidVideoTrack,
}

type Result<T> = std::result::Result<T, Traced<TracksRequestError>>;

/// Representation of [MediaStreamConstraints][1] object.
///
/// It's used for invoking [getUserMedia()][2] to specify what kinds of tracks
/// should be included into returned [`MediaStream`], and, optionally,
/// to establish constraints for those track's settings.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
/// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [3]: https://w3.org/TR/mediacapture-streams/#mediastream
#[derive(Debug, Default)]
pub struct TracksRequest {
    audio: HashMap<TrackId, AudioTrackConstraints>,
    video: HashMap<TrackId, VideoTrackConstraints>,
}

impl TracksRequest {
    /// Adds track request to this [`TracksRequest`].
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

/// Subtype of [`TracksRequest`], which can have maximum one track of each kind
/// and must have at least one track of any kind.
#[derive(Debug)]
pub struct SimpleTracksRequest {
    audio: Option<(TrackId, AudioTrackConstraints)>,
    video: Option<(TrackId, VideoTrackConstraints)>,
}

impl SimpleTracksRequest {
    /// Parses [`MediaStreamTrack`]s and returns [`HashMap`] with [`TrackId`]s
    /// and [`MediaStreamTracks`]s.
    ///
    /// # Errors
    ///
    /// Errors with [`TracksRequestError::InvalidAudioTrack`] if some audio
    /// track from provided [`MediaStream`] not satisfies
    /// contained constrains.
    ///
    /// Errors with [`TracksRequestError::ExpectedAudioTracks`] if provided
    /// [`HashMap`] doesn't have expected audio track.
    ///
    /// Errors with [`TracksRequestError::InvalidVideoTrack`] if some video
    /// track from provided [`HashMap`] not satisfies
    /// contained constrains.
    ///
    /// Errors with [`TracksRequestError::ExpectedVideoTracks`] if provided
    /// [`HashMap`] doesn't have expected video track.
    pub fn parse_tracks(
        &self,
        tracks: Vec<MediaStreamTrack>,
    ) -> Result<HashMap<TrackId, MediaStreamTrack>> {
        use TracksRequestError::{InvalidAudioTrack, InvalidVideoTrack};

        let mut parsed_tracks = HashMap::new();

        let (video_tracks, audio_tracks): (Vec<_>, Vec<_>) =
            tracks.into_iter().partition(|track| match track.kind() {
                TrackKind::Audio { .. } => false,
                TrackKind::Video { .. } => true,
            });

        if let Some((id, audio)) = &self.audio {
            if let Some(track) = audio_tracks.into_iter().next() {
                if audio.satisfies(track.as_ref()) {
                    parsed_tracks.insert(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidAudioTrack));
                }
            }
        }

        if let Some((id, video)) = &self.video {
            if let Some(track) = video_tracks.into_iter().next() {
                if video.satisfies(track.as_ref()) {
                    parsed_tracks.insert(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidVideoTrack));
                }
            }
        }

        Ok(parsed_tracks)
    }

    /// Merges [`SimpleTracksRequest`] with provided [`MediaStreamSettings`].
    ///
    /// Applies new settings if possible, meaning that if this
    /// [`SimpleTracksRequest`] does not have some constraint, then it will be
    /// applied from [`MediaStreamSettings`].
    ///
    /// # Errors
    ///
    /// Errors with [`TracksRequestError::ExpectedAudioTracks`] if
    /// [`SimpleTracksRequest`] contains [`AudioTrackConstraints`], but provided
    /// [`MediaStreamSettings`] doesn't and this [`AudioTrackConstraints`] are
    /// important.
    ///
    /// Errors with [`TracksRequestError::ExpectedVideoTracks`] if
    /// [`SimpleTracksRequest`] contains [`VideoTrackConstraints`], but provided
    /// [`MediaStreamSettings`] doesn't and this [`VideoTrackConstraints`] are
    /// important.
    pub fn merge<T: Into<MediaStreamSettings>>(
        &mut self,
        other: T,
    ) -> Result<()> {
        let other = other.into();

        if let Some((_, video_caps)) = &self.video {
            if !other.is_video_enabled() {
                if video_caps.is_required() {
                    return Err(tracerr::new!(
                        TracksRequestError::ExpectedVideoTracks
                    ));
                } else {
                    self.video.take();
                }
            }
        }
        if let Some((_, audio_caps)) = &self.audio {
            if !other.is_audio_enabled() {
                if audio_caps.is_required() {
                    return Err(tracerr::new!(
                        TracksRequestError::ExpectedAudioTracks
                    ));
                } else {
                    self.audio.take();
                }
            }
        }

        if other.is_audio_enabled() {
            if let Some((_, audio)) = self.audio.as_mut() {
                audio.merge(other.get_audio().clone());
            }
        }
        if other.is_video_enabled() {
            if let Some((_, video)) = self.video.as_mut() {
                video.merge(other.get_video().clone());
            }
        }

        Ok(())
    }
}

impl TryFrom<TracksRequest> for SimpleTracksRequest {
    type Error = TracksRequestError;

    fn try_from(
        value: TracksRequest,
    ) -> std::result::Result<Self, Self::Error> {
        use TracksRequestError::{
            NoTracks, TooManyAudioTracks, TooManyVideoTracks,
        };

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

impl From<&SimpleTracksRequest> for MediaStreamSettings {
    fn from(request: &SimpleTracksRequest) -> Self {
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
