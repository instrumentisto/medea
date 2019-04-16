use serde::{Deserialize, Serialize};

use crate::media::peer::Id as PeerID;

/// ID of [`Track`].
pub type Id = u64;

/// [`MediaStreamTrack`] representation.
#[derive(Debug)]
pub struct Track {
    pub id: Id,
    pub media_type: MediaType,
}

impl Track {
    /// Creates new [`Track`] of the specified type.
    pub fn new(id: Id, media_type: MediaType) -> Self {
        Self { id, media_type }
    }
}

/// [`Track] with specified direction.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Directional {
    pub id: Id,
    pub direction: Direction,
    pub media_type: MediaType,
}

/// Direction of [`Track`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Direction {
    Send { receivers: Vec<PeerID> },
    Recv { sender: PeerID },
}

/// Type of [`Track`].
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MediaType {
    Audio(AudioSettings),
    Video(VideoSettings),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioSettings {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VideoSettings {}
