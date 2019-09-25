//! [MediaStreamConstraints][1] related objects.
//!
//! [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints

use std::{collections::HashMap, convert::TryFrom};

use js_sys::Reflect;
use medea_client_api_proto::{
    AudioSettings, MediaType, TrackId, VideoSettings,
};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::{
    ConstrainDomStringParameters, MediaStream as SysMediaStream,
    MediaStreamConstraints as SysMediaStreamConstraints,
    MediaStreamTrack as SysMediaStreamTrack,
    MediaTrackConstraints as SysMediaTrackConstraints,
};

use crate::utils::WasmErr;

use super::{MediaStream, MediaTrack};

/// Representation of [MediaStreamConstraints][1] object.
///
/// It's used for invoking [`getUserMedia()`][2] to specify what kinds of tracks
/// should be included into returned [`MediaStream`], and, optionally,
/// to establish constraints for those [`MediaTrack`]'s settings.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
/// [2]:
/// https://www.w3.org/TR/mediacapture-streams/#dom-mediadevices-getusermedia
/// [3]: https://www.w3.org/TR/mediacapture-streams/#mediastream
#[derive(Default)]
pub struct StreamRequest {
    audio: HashMap<TrackId, AudioSettings>,
    video: HashMap<TrackId, VideoSettings>,
}

impl StreamRequest {
    /// Adds track request to this [`StreamRequest`].
    pub fn add_track_request(&mut self, track_id: TrackId, caps: MediaType) {
        match caps {
            MediaType::Audio(audio) => {
                self.audio.insert(track_id, audio);
            }
            MediaType::Video(video) => {
                self.video.insert(track_id, video);
            }
        }
    }
}

/// Subtype of [`StreamRequest`], which can have maximum one track of each kind
/// and must have at least one track of any kind.
#[allow(clippy::module_name_repetitions)]
pub struct SimpleStreamRequest {
    audio: Option<(TrackId, AudioSettings)>,
    video: Option<(TrackId, VideoSettings)>,
}

impl SimpleStreamRequest {
    /// Parses raw [`SysMediaStream`] and returns [`MediaStream`].
    pub fn parse_stream(
        &self,
        stream: &SysMediaStream,
    ) -> Result<MediaStream, WasmErr> {
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
                tracks.push(MediaTrack::new(
                    *id,
                    track,
                    MediaType::Audio(audio.clone()),
                ))
            } else {
                return Err(WasmErr::from(
                    "Provided MediaStream was expected to have single audio \
                     track",
                ));
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
                tracks.push(MediaTrack::new(
                    *id,
                    track,
                    MediaType::Video(video.clone()),
                ))
            } else {
                return Err(WasmErr::from(
                    "Provided MediaStream was expected to have single video \
                     track",
                ));
            }
        }

        Ok(MediaStream::from_tracks(tracks))
    }
}

impl TryFrom<StreamRequest> for SimpleStreamRequest {
    type Error = WasmErr;

    fn try_from(value: StreamRequest) -> Result<Self, Self::Error> {
        if value.video.len() > 1 {
            Err(WasmErr::from(
                "Only one video track allowed in SimpleStreamRequest",
            ))
        } else if value.audio.len() > 1 {
            Err(WasmErr::from(
                "Only one audio track allowed in SimpleStreamRequest",
            ))
        } else if value.video.len() + value.audio.len() < 1 {
            Err(WasmErr::from(
                "SimpleStreamRequest should have at least on track",
            ))
        } else {
            let mut request = Self {
                audio: None,
                video: None,
            };
            for (id, audio) in value.audio {
                request.audio.replace((id, audio));
            }
            for (id, video) in value.video {
                request.video.replace((id, video));
            }

            Ok(request)
        }
    }
}

/// [MediaStreamConstraints][1] wrapper.
///
/// [1]: https://www.w3.org/TR/mediacapture-streams/#dom-mediastreamconstraints
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct MediaStreamConstraints {
    audio: Option<AudioTrackConstraints>,
    video: Option<VideoTrackConstraints>,
}

impl MediaStreamConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn audio(&self) -> &Option<AudioTrackConstraints> {
        &self.audio
    }

    pub fn video(&self) -> &Option<VideoTrackConstraints> {
        &self.video
    }

    pub fn audio_mut(&mut self) -> &mut Option<AudioTrackConstraints> {
        &mut self.audio
    }

    pub fn video_mut(&mut self) -> &mut Option<VideoTrackConstraints> {
        &mut self.video
    }
}

impl From<&SimpleStreamRequest> for MediaStreamConstraints {
    fn from(request: &SimpleStreamRequest) -> Self {
        let mut constraints = Self {
            audio: None,
            video: None,
        };

        if let Some((_, _)) = request.video {
            constraints.video = Some(VideoTrackConstraints { device_id: None });
        }
        if let Some((_, _)) = request.audio {
            constraints.audio = Some(AudioTrackConstraints { device_id: None });
        }

        constraints
    }
}

impl From<MediaStreamConstraints> for SysMediaStreamConstraints {
    fn from(constraints: MediaStreamConstraints) -> Self {
        let mut sys_constraints = Self::new();

        if let Some(video) = constraints.video {
            let video: SysMediaTrackConstraints = video.into();
            sys_constraints.video(&video.into());
        }

        if let Some(audio) = constraints.audio {
            let audio: SysMediaTrackConstraints = audio.into();
            sys_constraints.audio(&audio.into());
        }

        sys_constraints
    }
}

// TODO: its gonna be a nightmare if we will add all possible constraints,
//       especially if we will support all that `exact`/`min`/`max`/`ideal`
//       stuff, will need major refactoring then
// TODO: using reflection to get fields values is pure evil, but there are no
//       getters, should be wrapped or improved in wasm-bindgen

/// Represents constraints applicable to audio tracks.
#[derive(Clone, Default)]
pub struct AudioTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

impl AudioTrackConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "audio" {
            return false;
        }

        if let Some(device_id) = &self.device_id {
            let track_device_id = if let Ok(val) = Reflect::get(
                track.get_settings().as_ref(),
                &JsValue::from_str("deviceId"),
            ) {
                if let Some(val) = val.as_string() {
                    val
                } else {
                    return false;
                }
            } else {
                return false;
            };

            if track_device_id.as_str() != device_id {
                return false;
            }
        }

        true
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }
}

impl Into<SysMediaTrackConstraints> for AudioTrackConstraints {
    fn into(self) -> SysMediaTrackConstraints {
        let mut constraints = SysMediaTrackConstraints::new();

        if let Some(device_id) = self.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}

/// Represents constraints applicable to video tracks.
#[derive(Clone, Default)]
pub struct VideoTrackConstraints {
    /// The identifier of the device generating the content of the media track.
    device_id: Option<String>,
}

impl VideoTrackConstraints {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if provided [`MediaStreamTrack`][1] satisfies constraints
    /// contained.
    ///
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#mediastreamtrack
    pub fn satisfies(&self, track: &SysMediaStreamTrack) -> bool {
        if track.kind() != "video" {
            return false;
        }

        if let Some(device_id) = &self.device_id {
            let track_device_id = if let Ok(val) = Reflect::get(
                track.get_settings().as_ref(),
                &JsValue::from_str("deviceId"),
            ) {
                if let Some(val) = val.as_string() {
                    val
                } else {
                    return false;
                }
            } else {
                return false;
            };

            if track_device_id.as_str() != device_id {
                return false;
            }
        }

        true
    }

    /// Sets [deviceId][1] constraint.
    /// [1]: https://www.w3.org/TR/mediacapture-streams/#def-constraint-deviceId
    pub fn device_id(&mut self, device_id: String) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }
}

impl Into<SysMediaTrackConstraints> for VideoTrackConstraints {
    fn into(self) -> SysMediaTrackConstraints {
        let mut constraints = SysMediaTrackConstraints::new();

        if let Some(device_id) = self.device_id {
            let mut val = ConstrainDomStringParameters::new();
            val.exact(&(device_id.into()));
            constraints.device_id(&(val.into()));
        }

        constraints
    }
}
