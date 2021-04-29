use std::ptr::NonNull;

use dart_sys::Dart_Handle;

use crate::{utils::DartClosure, ForeignClass, MediaKind, MediaSourceKind};

pub struct RemoteMediaTrack;

impl ForeignClass for RemoteMediaTrack {}

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

    // pub fn get_track(&self) -> sys::MediaStreamTrack

    pub fn on_enabled(&self, cb: DartClosure<()>) {
        cb.call0();
    }

    pub fn on_disabled(&self, cb: DartClosure<()>) {
        cb.call0();
    }

    pub fn on_muted(&self, cb: DartClosure<()>) {
        cb.call0();
    }

    pub fn on_unmuted(&self, cb: DartClosure<()>) {
        cb.call0();
    }

    pub fn on_stopped(&self, cb: DartClosure<()>) {
        cb.call0();
    }
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_enabled(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_enabled(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_disabled(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_disabled(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_muted(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_muted(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_unmuted(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_unmuted(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__on_stopped(
    this: NonNull<RemoteMediaTrack>,
    f: Dart_Handle,
) {
    let this = this.as_ref();
    this.on_stopped(DartClosure::new(f));
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__enabled(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.enabled() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__muted(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.muted() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__kind(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__media_source_kind(
    this: NonNull<RemoteMediaTrack>,
) -> u8 {
    let this = this.as_ref();

    this.media_source_kind() as u8
}

#[no_mangle]
pub unsafe extern "C" fn RemoteMediaTrack__free(
    this: NonNull<RemoteMediaTrack>,
) {
    RemoteMediaTrack::from_ptr(this);
}
