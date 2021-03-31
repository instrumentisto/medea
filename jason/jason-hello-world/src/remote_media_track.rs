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

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enable(this: *mut RemoteMediaTrack) {
    let this = Box::from_raw(this);
    this.enable();
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.kind().into()
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: *mut RemoteMediaTrack,
) -> u8 {
    let this = Box::from_raw(this);
    this.media_source_kind().into()
}
