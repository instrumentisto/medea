use crate::{utils::ptr_from_dart_as_ref, MediaKind, MediaSourceKind};

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
    this: *const LocalMediaTrack,
) -> u8 {
    let this = ptr_from_dart_as_ref(this);

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: *const LocalMediaTrack,
) -> u8 {
    let this = ptr_from_dart_as_ref(this);

    this.media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__free(this: *mut LocalMediaTrack) {
    if !this.is_null() {
        Box::from_raw(this);
    }
}
