use dart_sys::Dart_Handle;

use crate::{api::dart_ffi::ForeignClass, platform};

#[cfg(feature = "mockable")]
pub use self::mock::RoomHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::room::RoomHandle;

impl ForeignClass for RoomHandle {}

/// Sets callback, invoked when a new [`Connection`] with some remote `Peer`
/// is established.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_new_connection(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_new_connection(platform::Function::new(cb)).unwrap();
}

/// Sets `on_close` callback, invoked on this [`Room`] close, providing a
/// [`RoomCloseReason`].
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_close(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_close(platform::Function::new(cb)).unwrap();
}

/// Sets callback, invoked when a new [`local::Track`] is added to this
/// [`Room`].
///
/// This might happen in such cases:
/// 1. Media server initiates a media request.
/// 2. `enable_audio`/`enable_video` is called.
/// 3. [`MediaStreamSettings`] updated via `set_local_media_settings`.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_local_track(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_local_track(platform::Function::new(cb)).unwrap();
}

/// Sets `on_connection_loss` callback, invoked when a connection with
/// server is lost.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__on_connection_loss(
    this: *mut RoomHandle,
    cb: Dart_Handle,
) {
    let this = this.as_ref().unwrap();

    // TODO: Remove unwrap when propagating errors from Rust to Dart is
    //       implemented.
    this.on_connection_loss(platform::Function::new(cb))
        .unwrap();
}

/// Frees the data behind the provided pointer. Should be called when object is
/// no longer needed. Calling this more than once for the same pointer is
/// equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn RoomHandle__free(this: *mut RoomHandle) {
    RoomHandle::from_ptr(this);
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::{
        api::{ConnectionHandle, JasonError, LocalMediaTrack, ReconnectHandle},
        platform,
        room::RoomCloseReason,
        rpc::{ClientDisconnect, CloseReason},
    };

    pub struct RoomHandle;

    #[allow(clippy::missing_errors_doc)]
    impl RoomHandle {
        pub fn on_new_connection(
            &self,
            cb: platform::Function<ConnectionHandle>,
        ) -> Result<(), JasonError> {
            cb.call1(ConnectionHandle);
            Ok(())
        }

        pub fn on_close(
            &self,
            cb: platform::Function<RoomCloseReason>,
        ) -> Result<(), JasonError> {
            cb.call1(RoomCloseReason::new(CloseReason::ByClient {
                is_err: true,
                reason: ClientDisconnect::RpcClientUnexpectedlyDropped,
            }));
            Ok(())
        }

        pub fn on_local_track(
            &self,
            cb: platform::Function<LocalMediaTrack>,
        ) -> Result<(), JasonError> {
            cb.call1(LocalMediaTrack);
            Ok(())
        }

        pub fn on_connection_loss(
            &self,
            cb: platform::Function<ReconnectHandle>,
        ) -> Result<(), JasonError> {
            cb.call1(ReconnectHandle);
            Ok(())
        }

        // pub async fn join(&self, token: String) -> Result<(), JasonError>
        // pub fn on_failed_local_media(
        //     &self,
        //     f: Callback<JasonError>,
        // ) -> Result<(), JasonError> {
        // }
        // pub async fn set_local_media_settings(&self,
        // settings: &MediaStreamSettings, stop_first: bool, rollback_on_fail:
        // bool) -> Result<(), ConstraintsUpdateException> pub async fn
        // mute_audio(&self) -> Result<(), JasonError> pub async fn
        // unmute_audio(&self) -> Result<(), JasonError> pub async fn
        // mute_video(&self, source_kind: Option<MediaSourceKind>) -> Result<(),
        // JasonError> pub async fn unmute_video(&self, source_kind:
        // Option<MediaSourceKind>) -> Result<(), JasonError> pub async fn
        // disable_audio(&self) -> Result<(), JasonError> pub async fn
        // enable_audio(&self) -> Result<(), JasonError> pub async fn
        // disable_video(&self, source_kind: Option<MediaSourceKind>) ->
        // Result<(), JasonError> pub async fn
        // enable_video(&self,source_kind: Option<MediaSourceKind>) ->
        // Result<(), JasonError> pub async fn disable_remote_audio(&
        // self) -> Result<(), JasonError> pub async fn
        // disable_remote_video(&self) -> Result<(), JasonError> pub async fn
        // enable_remote_audio(&self) -> Result<(), JasonError> pub async fn
        // enable_remote_video(&self) -> Result<(), JasonError>
    }
}
