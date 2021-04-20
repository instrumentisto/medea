use crate::{MediaKind, MediaSourceKind};

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
    this: *const RemoteMediaTrack,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.enabled() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__muted(
    this: *const RemoteMediaTrack,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.muted() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: *const RemoteMediaTrack,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: *const RemoteMediaTrack,
) -> u8 {
    let this = this.as_ref().unwrap();

    this.media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__free(this: *mut RemoteMediaTrack) {
    Box::from_raw(this);
}
