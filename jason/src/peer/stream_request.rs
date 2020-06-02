//! [MediaStreamConstraints][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints

use std::{collections::HashMap, convert::TryFrom};

use derive_more::Display;
use medea_client_api_proto::TrackId;
use tracerr::Traced;

use crate::{
    media::{
        AudioTrackConstraints, MediaStream, MediaStreamSettings,
        TrackConstraints, TrackKind, VideoTrackConstraints,
    },
    utils::{console_error, JsCaused, JsError},
};

use super::PeerMediaStream;

/// Errors that may occur when validating [`StreamRequest`] or
/// parsing [`MediaStream`].
#[derive(Debug, Display, JsCaused)]
pub enum StreamRequestError {
    /// [`StreamRequest`] contains multiple [`AudioTrackConstraints`].
    #[display(fmt = "only one audio track is allowed in SimpleStreamRequest")]
    TooManyAudioTracks,

    /// [`StreamRequest`] contains multiple [`VideoTrackConstraints`].
    #[display(fmt = "only one video track is allowed in SimpleStreamRequest")]
    TooManyVideoTracks,

    /// [`StreamRequest`] contains no track constraints at all.
    #[display(fmt = "SimpleStreamRequest should have at least one track")]
    NoTracks,

    /// Provided [`MediaStream`] has multiple audio tracks.
    #[display(
        fmt = "provided MediaStream was expected to have single audio track"
    )]
    ExpectedAudioTracks,

    /// Provided [`MediaStream`] has multiple video tracks.
    #[display(
        fmt = "provided MediaStream was expected to have single video track"
    )]
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

type Result<T> = std::result::Result<T, Traced<StreamRequestError>>;

/// Representation of [MediaStreamConstraints][1] object.
///
/// It's used for invoking [getUserMedia()][2] to specify what kinds of tracks
/// should be included into returned [`MediaStream`], and, optionally,
/// to establish constraints for those track's settings.
///
/// [1]: https://w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
/// [2]: https://w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [3]: https://w3.org/TR/mediacapture-streams/#mediastream
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
pub struct SimpleStreamRequest {
    audio: Option<(TrackId, AudioTrackConstraints)>,
    video: Option<(TrackId, VideoTrackConstraints)>,
}

impl SimpleStreamRequest {
    /// Parses [`MediaStream`] and returns [`PeerMediaStream`] wrapper.
    ///
    /// # Errors
    ///
    /// Errors with [`StreamRequestError::InvalidAudioTrack`] if some audio
    /// track from provided [`MediaStream`] not satisfies
    /// contained constrains.
    ///
    /// Errors with [`StreamRequestError::ExpectedAudioTracks`] if provided
    /// [`MediaStream`] doesn't have expected audio track.
    ///
    /// Errors with [`StreamRequestError::InvalidVideoTrack`] if some video
    /// track from provided [`MediaStream`] not satisfies
    /// contained constrains.
    ///
    /// Errors with [`StreamRequestError::ExpectedVideoTracks`] if provided
    /// [`MediaStream`] doesn't have expected video track.
    pub fn parse_stream(&self, stream: MediaStream) -> Result<PeerMediaStream> {
        use StreamRequestError::{InvalidAudioTrack, InvalidVideoTrack};

        let result_stream = PeerMediaStream::new();

        let (video_tracks, audio_tracks): (Vec<_>, Vec<_>) = stream
            .into_tracks()
            .into_iter()
            .partition(|track| match track.kind() {
                TrackKind::Audio { .. } => false,
                TrackKind::Video { .. } => true,
            });

        if let Some((id, audio)) = &self.audio {
            if audio_tracks.len() == 1 {
                let track = audio_tracks.into_iter().next().unwrap();
                if audio.satisfies(track.as_ref()) {
                    result_stream.add_track(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidAudioTrack));
                }
            } else {
                // TODO: Maybe we should do something??
                // return Err(tracerr::new!(ExpectedAudioTracks));
            }
        }

        if let Some((id, video)) = &self.video {
            if video_tracks.len() == 1 {
                let track = video_tracks.into_iter().next().unwrap();
                if video.satisfies(track.as_ref()) {
                    result_stream.add_track(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidVideoTrack));
                }
            } else {
                // TODO: Maybe we should do something??
                // return Err(tracerr::new!(ExpectedVideoTracks));
            }
        }

        Ok(result_stream)
    }

    /// Merges [`SimpleStreamRequest`] with provided [`MediaStreamSettings`].
    ///
    /// Applies new settings if possible, meaning that if this
    /// [`SimpleStreamRequest`] does not have some constraint, then it will be
    /// applied from [`MediaStreamSettings`].
    ///
    /// # Errors
    ///
    /// Errors with [`StreamRequestError::ExpectedAudioTracks`] if
    /// [`SimpleStreamRequest`] contains [`AudioTrackConstraints`], but provided
    /// [`MediaStreamSettings`] doesn't.
    ///
    /// Errors with [`StreamRequestError::ExpectedVideoTracks`] if
    /// [`SimpleStreamRequest`] contains [`VideoTrackConstraints`], but provided
    /// [`MediaStreamSettings`] doesn't.
    // TODO: correct docs, because this is not evident merge.
    //       Also, may be rename this function.
    pub fn merge<T: Into<MediaStreamSettings>>(
        &mut self,
        other: T,
    ) -> Result<()> {
        let mut other = other.into();

        if let Some((_, video_caps)) = &self.video {
            if other.get_video().is_none() {
                if video_caps.is_important() {
                    return Err(tracerr::new!(
                        StreamRequestError::ExpectedAudioTracks
                    ));
                } else {
                    self.video.take();
                }
            }
        }
        if let Some((_, audio_caps)) = &self.audio {
            if other.get_audio().is_none() {
                if audio_caps.is_important() {
                    return Err(tracerr::new!(
                        StreamRequestError::ExpectedVideoTracks
                    ));
                } else {
                    self.audio.take();
                }
            }
        }

        if let Some(other_audio) = other.take_audio() {
            if let Some((_, audio)) = self.audio.as_mut() {
                audio.merge(other_audio);
            }
        }
        if let Some(other_video) = other.take_video() {
            if let Some((_, video)) = self.video.as_mut() {
                video.merge(other_video);
            }
        }

        Ok(())
    }
}

impl TryFrom<StreamRequest> for SimpleStreamRequest {
    type Error = StreamRequestError;

    fn try_from(
        value: StreamRequest,
    ) -> std::result::Result<Self, Self::Error> {
        use StreamRequestError::{
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

impl From<&SimpleStreamRequest> for MediaStreamSettings {
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
