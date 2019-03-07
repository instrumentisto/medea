/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[derive(Debug)]
pub struct Track {
    pub id: Id,
    media_type: TrackMediaType,
}

impl Track {
    pub fn new(id: Id, media_type: TrackMediaType) -> Track {
        Track { id, media_type }
    }
}

#[derive(Debug)]
pub enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Debug)]
pub struct AudioSettings {}

#[derive(Debug)]
pub struct VideoSettings {}
