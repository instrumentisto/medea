use std::ptr::NonNull;

use crate::{ForeignClass, MediaKind, MediaSourceKind};

pub struct LocalMediaTrack;

impl ForeignClass for LocalMediaTrack {}

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
    this: NonNull<LocalMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__media_source_kind(
    this: NonNull<LocalMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn LocalMediaTrack__free(this: NonNull<LocalMediaTrack>) {
    LocalMediaTrack::from_ptr(this);
}
