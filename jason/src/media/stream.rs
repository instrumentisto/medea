use std::{
    rc::{Rc, Weak},
};

use crate::MediaStreamSettings;

use wasm_bindgen::prelude::*;
use web_sys::{
    MediaStream as SysMediaStream, MediaStreamTrack as SysMediaStreamTrack,
};

#[wasm_bindgen(js_name = LocalMediaStream)]
#[derive(Clone)]
pub struct MediaStream {
    stream: SysMediaStream,
    constraints: MediaStreamSettings,
    tracks: Vec<MediaStreamTrack>,
}

impl MediaStream {
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

    pub fn into_tracks(self) -> Vec<MediaStreamTrack> {
        for track in &self.tracks {
            self.stream.remove_track(track.as_ref());
        }
        self.tracks
    }
}

#[wasm_bindgen(js_class = LocalMediaStream)]
impl MediaStream {
    pub fn get_media_stream(&self) -> SysMediaStream {
        Clone::clone(&self.stream)
    }

    pub fn free_audio(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => false,
            TrackKind::Video => true,
        });
    }

    pub fn free_video(&mut self) {
        self.tracks.retain(|track| match track.kind() {
            TrackKind::Audio => true,
            TrackKind::Video => false,
        });
    }
}

impl AsRef<SysMediaStream> for MediaStream {
    fn as_ref(&self) -> &SysMediaStream {
        &self.stream
    }
}

pub struct WeakMediaStreamTrack(Weak<SysMediaStreamTrack>);

impl WeakMediaStreamTrack {
    pub fn upgrade(&self) -> Option<MediaStreamTrack> {
        self.0.upgrade().map(MediaStreamTrack)
    }

    pub fn can_be_upgraded(&self) -> bool {
        self.0.strong_count() > 0
    }
}

#[derive(Clone)]
pub struct MediaStreamTrack(Rc<SysMediaStreamTrack>);

pub enum TrackKind {
    Audio,
    Video,
}

impl<T> From<T> for MediaStreamTrack
where
    SysMediaStreamTrack: From<T>,
{
    fn from(track: T) -> Self {
        let track = MediaStreamTrack(Rc::new(<SysMediaStreamTrack as From<
            T,
        >>::from(track)));
        crate::utils::console_error(format!("Creating {}", track.0.label()));
        track
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

    pub fn kind(&self) -> TrackKind {
        match self.0.kind().as_ref() {
            "audio" => TrackKind::Audio,
            "video" => TrackKind::Video,
            _ => unreachable!(),
        }
    }

    pub fn downgrade(&self) -> WeakMediaStreamTrack {
        WeakMediaStreamTrack(Rc::downgrade(&self.0))
    }
}

impl Drop for MediaStreamTrack {
    fn drop(&mut self) {
        // Last strong ref being dropped, so stop underlying MediaTrack
        if Rc::strong_count(&self.0) == 1 {
            crate::utils::console_error(format!(
                "Stopping {}, {}",
                self.0.label(),
                self.0.id()
            ));
            self.0.stop();
        }
    }
}
