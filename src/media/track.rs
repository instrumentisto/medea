use serde::{Deserialize, Serialize};

use crate::media::peer::Id as PeerID;

/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[derive(Debug)]
pub struct Track {
    pub id: Id,
    pub media_type: TrackMediaType,
}

impl Track {
    /// Creates new [`Track`] of the specified type.
    pub fn new(id: Id, media_type: TrackMediaType) -> Track {
        Track { id, media_type }
    }
}

/// [`Track] with specified direction.
#[derive(Debug, Deserialize, Serialize)]
pub struct DirectionalTrack {
    pub id: Id,
    pub media_type: TrackMediaType,
    pub direction: TrackDirection,
}

/// Direction of [`Track`].
#[derive(Debug, Deserialize, Serialize)]
pub enum TrackDirection {
    Send { receivers: Vec<PeerID> },
    Recv { sender: PeerID },
}

/// Type of [`Track`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TrackMediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioSettings {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VideoSettings {}
