use crate::{MediaKind, MediaSourceKind};

pub struct RemoteMediaTrack;

impl RemoteMediaTrack {
    pub fn enable(&self) {}

    pub fn kind(&self) -> MediaKind {
        MediaKind::Foo
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Foo
    }
}

// TODO: on_enable, on_disabled
