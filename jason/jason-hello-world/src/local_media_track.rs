pub struct LocalMediaTrack;

enum MediaKind {
    Foo
}

impl Into<u8> for MediaKind {
    fn into(self) -> u8 {
        0
    }
}

enum MediaSourceKind {
    Foo
}

impl Into<u8> for MediaSourceKind {
    fn into(self) -> u8 {
        0
    }
}

impl LocalMediaTrack {
    fn kind(&self) -> MediaKind {
        MediaKind::Foo
    }

    fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Foo
    }
}


pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind().into()
}
