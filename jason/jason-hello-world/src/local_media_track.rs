use crate::{MediaKind, MediaSourceKind};

pub struct LocalMediaTrack;

impl LocalMediaTrack {
    pub fn kind(&self) -> MediaKind {
        MediaKind::Foo
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Foo
    }
}
