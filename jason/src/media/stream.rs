use std::rc::{Rc, Weak};
use wasm_bindgen::{prelude::*, JsValue};
use web_sys::{MediaStream as BackingMediaStream, MediaStreamTrack};

use crate::utils::WasmErr;

pub struct GetMediaRequest {
    pub audio: bool,
    pub video: bool,
}

impl GetMediaRequest {
    pub fn new(audio: bool, video: bool) -> Result<Self, WasmErr> {
        if !audio && !video {
            return Err(WasmErr::from_str(
                "MediaCaps should have video, audio, or both",
            ));
        }
        Ok(Self { audio, video })
    }
}

impl From<&GetMediaRequest> for web_sys::MediaStreamConstraints {
    fn from(caps: &GetMediaRequest) -> Self {
        let mut constraints = Self::new();
        if caps.audio {
            constraints.audio(&JsValue::from_bool(true));
        }
        if caps.video {
            constraints.video(&JsValue::from_bool(true));
        }
        constraints
    }
}

struct InnerStream {
    stream: BackingMediaStream,
    audio_tracks: Vec<MediaStreamTrack>,
    video_tracks: Vec<MediaStreamTrack>,
}

impl From<BackingMediaStream> for InnerStream {
    fn from(media_stream: BackingMediaStream) -> Self {
        let mut audio_tracks = Vec::new();
        let mut video_tracks = Vec::new();

        let stream_audio_tracks =
            js_sys::try_iter(&media_stream.get_audio_tracks())
                .unwrap()
                .unwrap();

        for track in stream_audio_tracks {
            audio_tracks.push(MediaStreamTrack::from(track.unwrap()));
        }

        let stream_video_tracks =
            js_sys::try_iter(&media_stream.get_video_tracks())
                .unwrap()
                .unwrap();

        for track in stream_video_tracks {
            video_tracks.push(MediaStreamTrack::from(track.unwrap()));
        }

        Self {
            stream: media_stream,
            audio_tracks,
            video_tracks,
        }
    }
}

#[allow(clippy::module_name_repetitions)]
pub struct MediaStream(Rc<InnerStream>);

impl MediaStream {
    pub fn from_stream(stream: BackingMediaStream) -> Rc<Self> {
        Rc::new(Self(Rc::new(InnerStream::from(stream))))
    }

    pub fn from_tracks(tracks: &[&MediaStreamTrack]) -> Rc<Self> {
        // should be safe to unwrap
        let stream = BackingMediaStream::new().unwrap();

        for track in tracks {
            stream.add_track(&track);
        }

        Self::from_stream(stream)
    }

    pub fn new_handle(&self) -> MediaStreamHandle {
        MediaStreamHandle(Rc::downgrade(&self.0))
    }

    pub fn get_audio_track(&self) -> Option<&MediaStreamTrack> {
        self.0.audio_tracks.get(0)
    }

    pub fn get_video_track(&self) -> Option<&MediaStreamTrack> {
        self.0.video_tracks.get(0)
    }
}

#[wasm_bindgen]
pub struct MediaStreamHandle(Weak<InnerStream>);

#[wasm_bindgen]
impl MediaStreamHandle {
    pub fn get_media_stream(&self) -> Result<BackingMediaStream, JsValue> {
        match self.0.upgrade() {
            Some(inner) => Ok(inner.stream.clone()),
            None => Err(WasmErr::from_str("Detached state").into()),
        }
    }
}
