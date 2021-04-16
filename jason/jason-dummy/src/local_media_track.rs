use crate::{utils::ptr_from_dart_as_mut, MediaKind, MediaSourceKind};

pub struct LocalMediaTrack;

impl LocalMediaTrack {
    pub fn kind(&self) -> MediaKind {
        MediaKind::Video
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Display
    }

    // pub fn get_track(&self) -> sys::MediaStreamTrack
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    ptr_from_dart_as_mut(this).kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *mut LocalMediaTrack,
) -> u8 {
    ptr_from_dart_as_mut(this).media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__free(this: *mut LocalMediaTrack) {
    Box::from_raw(this);
}
