use crate::{MediaKind, MediaSourceKind};

pub struct LocalMediaTrack;

impl LocalMediaTrack {
    pub fn kind(&self) -> MediaKind {
        MediaKind::Audio
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Device
    }
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__free(
    this: *mut LocalMediaTrack,
) {
    Box::from_raw(this);
}
