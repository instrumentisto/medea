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

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind().into()
}
