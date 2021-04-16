use crate::{utils::ptr_from_dart_as_mut, MediaKind, MediaSourceKind};

pub struct RemoteMediaTrack;

impl RemoteMediaTrack {
    pub fn enabled(&self) -> bool {
        true
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::Video
    }

    pub fn media_source_kind(&self) -> MediaSourceKind {
        MediaSourceKind::Device
    }

    pub fn muted(&self) -> bool {
        false
    }
    // pub fn on_enabled(&self, callback: Callback<()>)
    // pub fn on_disabled(&self, callback: Callback<()>)
    // pub fn get_track(&self) -> sys::MediaStreamTrack
    // pub fn on_muted(&self, cb: Callback<()>)
    // pub fn on_unmuted(&self, cb: Callback<()>)
    // pub fn on_stopped(&self, cb: Callback<()>)
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enabled(
    this: *mut RemoteMediaTrack,
) -> u8 {
    ptr_from_dart_as_mut(this).enabled() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    ptr_from_dart_as_mut(this).kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    ptr_from_dart_as_mut(this).media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__free(this: *mut RemoteMediaTrack) {
    Box::from_raw(this);
}
