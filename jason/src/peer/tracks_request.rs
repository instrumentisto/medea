//! [MediaStreamConstraints][1] related objects.
//!
//! [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamconstraints

use std::{collections::HashMap, convert::TryFrom, rc::Rc};

use derive_more::Display;
use medea_client_api_proto::{MediaSourceKind, TrackId};
use tracerr::Traced;

use crate::{
    media::{
        track::local, AudioTrackConstraints, DeviceVideoTrackConstraints,
        DisplayVideoTrackConstraints, MediaKind, MediaStreamSettings,
        TrackConstraints, VideoSource,
    },
    platform,
    utils::Caused,
};

/// Errors that may occur when validating [`TracksRequest`] or
/// parsing [`local::Track`]s.
#[derive(Clone, Debug, Display, Eq, Caused, PartialEq)]
#[cause(error = "platform::Error")]
pub enum TracksRequestError {
    /// [`TracksRequest`] contains multiple [`AudioTrackConstraints`].
    #[display(fmt = "only one audio track is allowed in SimpleTracksRequest")]
    TooManyAudioTracks,

    /// [`TracksRequest`] contains multiple [`DeviceVideoTrackConstraints`].
    #[display(
        fmt = "only one device video track is allowed in SimpleTracksRequest"
    )]
    TooManyDeviceVideoTracks,

    /// [`TracksRequest`] contains multiple [`DisplayVideoTrackConstraints`].
    #[display(
        fmt = "only one display video track is allowed in SimpleTracksRequest"
    )]
    TooManyDisplayVideoTracks,

    /// [`TracksRequest`] contains no track constraints at all.
    #[display(fmt = "SimpleTracksRequest should have at least one track")]
    NoTracks,

    /// Provided multiple audio [`local::Track`]s.
    #[display(fmt = "provided multiple audio MediaStreamTracks")]
    ExpectedAudioTracks,

    /// Provided multiple device video [`local::Track`]s.
    #[display(fmt = "provided multiple device video MediaStreamTracks")]
    ExpectedDeviceVideoTracks,

    /// Provided multiple display video [`local::Track`]s.
    #[display(fmt = "provided multiple display video MediaStreamTracks")]
    ExpectedDisplayVideoTracks,

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

/// Representation of [MediaStreamConstraints][1] object.
///
/// It's used for invoking [getUserMedia()][2] to specify what kinds of tracks
/// should be included into returned `MediaStream`, and, optionally,
/// to establish constraints for those track's settings.
///
/// [1]: https://w3.org/TR/mediacapture-streams#dom-mediastreamconstraints
/// [2]: https://w3.org/TR/mediacapture-streams#dom-mediadevices-getusermedia
/// [3]: https://w3.org/TR/mediacapture-streams#mediastream
#[derive(Debug, Default)]
pub struct TracksRequest {
    audio: HashMap<TrackId, AudioTrackConstraints>,
    device_video: HashMap<TrackId, DeviceVideoTrackConstraints>,
    display_video: HashMap<TrackId, DisplayVideoTrackConstraints>,
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
            TrackConstraints::Video(video) => match video {
                VideoSource::Device(device) => {
                    self.device_video.insert(track_id, device);
                }
                VideoSource::Display(display) => {
                    self.display_video.insert(track_id, display);
                }
            },
        }
    }
}

/// Subtype of [`TracksRequest`], which can have maximum one track of each kind
/// and must have at least one track of any kind.
#[derive(Debug)]
pub struct SimpleTracksRequest {
    audio: Option<(TrackId, AudioTrackConstraints)>,
    display_video: Option<(TrackId, DisplayVideoTrackConstraints)>,
    device_video: Option<(TrackId, DeviceVideoTrackConstraints)>,
}

impl SimpleTracksRequest {
    /// Parses [`local::Track`]s and returns [`HashMap`] with [`TrackId`]s
    /// and [`local::Track`]s.
    ///
    /// # Errors
    ///
    /// - [`TracksRequestError::InvalidAudioTrack`] when some audio track from
    ///   the provided [`local::Track`]s not satisfies contained constrains.
    /// - [`TracksRequestError::ExpectedAudioTracks`] when the provided
    ///   [`HashMap`] doesn't have the expected audio track.
    /// - [`TracksRequestError::InvalidVideoTrack`] when some device video track
    ///   from the provided [`HashMap`] doesn't satisfy contained constrains.
    /// - [`TracksRequestError::ExpectedDeviceVideoTracks`] when the provided
    ///   [`HashMap`] doesn't have the expected device video track.
    /// - [`TracksRequestError::InvalidVideoTrack`] when some display video
    ///   track from the provided [`HashMap`] doesn't satisfy contained
    ///   constrains.
    /// - [`TracksRequestError::ExpectedDisplayVideoTracks`] when the provided
    ///   [`HashMap`] doesn't have the expected display video track.
    pub fn parse_tracks(
        &self,
        tracks: Vec<Rc<local::Track>>,
    ) -> Result<HashMap<TrackId, Rc<local::Track>>, Traced<TracksRequestError>>
    {
        use TracksRequestError::{InvalidAudioTrack, InvalidVideoTrack};

        let mut parsed_tracks = HashMap::new();

        let mut display_video_tracks = Vec::new();
        let mut device_video_tracks = Vec::new();
        let mut audio_tracks = Vec::new();
        for track in tracks {
            match track.kind() {
                MediaKind::Audio => {
                    audio_tracks.push(track);
                }
                MediaKind::Video => match track.media_source_kind() {
                    MediaSourceKind::Device => {
                        device_video_tracks.push(track);
                    }
                    MediaSourceKind::Display => {
                        display_video_tracks.push(track);
                    }
                },
            }
        }

        if let Some((id, audio)) = &self.audio {
            if let Some(track) = audio_tracks.into_iter().next() {
                if audio.satisfies(track.as_ref()) {
                    parsed_tracks.insert(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidAudioTrack));
                }
            }
        }
        if let Some((id, device_video)) = &self.device_video {
            if let Some(track) = device_video_tracks.into_iter().next() {
                if device_video.satisfies(track.as_ref()) {
                    parsed_tracks.insert(*id, track);
                } else {
                    return Err(tracerr::new!(InvalidVideoTrack));
                }
            }
        }
        if let Some((id, display_video)) = &self.display_video {
            if let Some(track) = display_video_tracks.into_iter().next() {
                if display_video.satisfies(track.as_ref()) {
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
    /// - [`TracksRequestError::ExpectedAudioTracks`] when
    ///   [`SimpleTracksRequest`] contains [`AudioTrackConstraints`], but the
    ///   provided [`MediaStreamSettings`] doesn't and these
    ///   [`AudioTrackConstraints`] are important.
    /// - [`TracksRequestError::ExpectedDeviceVideoTracks`] when
    ///   [`SimpleTracksRequest`] contains [`DeviceVideoTrackConstraints`], but
    ///   the provided [`MediaStreamSettings`] doesn't and these
    ///   [`DeviceVideoTrackConstraints`] are important.
    /// - [`TracksRequestError::ExpectedDisplayVideoTracks`] when
    ///   [`SimpleTracksRequest`] contains [`DisplayVideoTrackConstraints`], but
    ///   the provided [`MediaStreamSettings`] doesn't and these
    ///   [`DisplayVideoTrackConstraints`] are important.
    pub fn merge<T: Into<MediaStreamSettings>>(
        &mut self,
        other: T,
    ) -> Result<(), Traced<TracksRequestError>> {
        let other = other.into();

        if let Some((_, audio_caps)) = &self.audio {
            if !other.is_audio_enabled() {
                if audio_caps.required() {
                    return Err(tracerr::new!(
                        TracksRequestError::ExpectedAudioTracks
                    ));
                }
                self.audio.take();
            }
        }
        if let Some((_, device_video_caps)) = &self.device_video {
            if !other.is_device_video_enabled() {
                if device_video_caps.required() {
                    return Err(tracerr::new!(
                        TracksRequestError::ExpectedDeviceVideoTracks
                    ));
                }
                self.device_video.take();
            }
        }
        if let Some((_, display_video_caps)) = &self.display_video {
            if !other.is_display_video_enabled() {
                if display_video_caps.required() {
                    return Err(tracerr::new!(
                        TracksRequestError::ExpectedDisplayVideoTracks
                    ));
                }
                self.display_video.take();
            }
        }

        if other.is_audio_enabled() {
            if let Some((_, audio)) = self.audio.as_mut() {
                audio.merge(other.get_audio().clone());
            }
        }
        if other.is_display_video_enabled() {
            if let Some((_, display_video)) = self.display_video.as_mut() {
                if let Some(other_display_video) = other.get_display_video() {
                    display_video.merge(other_display_video);
                }
            }
        }
        if other.is_device_video_enabled() {
            if let Some((_, device_video)) = self.device_video.as_mut() {
                if let Some(other_device_video) = other.get_device_video() {
                    device_video.merge(other_device_video.clone());
                }
            }
        }

        Ok(())
    }
}

impl TryFrom<TracksRequest> for SimpleTracksRequest {
    type Error = TracksRequestError;

    fn try_from(value: TracksRequest) -> Result<Self, Self::Error> {
        use TracksRequestError::{
            NoTracks, TooManyAudioTracks, TooManyDeviceVideoTracks,
            TooManyDisplayVideoTracks,
        };

        if value.device_video.len() > 1 {
            return Err(TooManyDeviceVideoTracks);
        } else if value.display_video.len() > 1 {
            return Err(TooManyDisplayVideoTracks);
        } else if value.audio.len() > 1 {
            return Err(TooManyAudioTracks);
        } else if value.device_video.is_empty()
            && value.display_video.is_empty()
            && value.audio.is_empty()
        {
            return Err(NoTracks);
        }

        let mut req = Self {
            audio: None,
            device_video: None,
            display_video: None,
        };
        for (id, audio) in value.audio {
            req.audio.replace((id, audio));
        }
        for (id, device) in value.device_video {
            req.device_video.replace((id, device));
        }
        for (id, display) in value.display_video {
            req.display_video.replace((id, display));
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
        if let Some((_, device_video)) = &request.device_video {
            constraints.device_video(device_video.clone());
        }
        if let Some((_, display_video)) = &request.display_video {
            constraints.display_video(display_video.clone());
        }

        constraints
    }
}
