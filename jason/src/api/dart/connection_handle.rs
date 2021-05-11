use std::ptr::NonNull;

use dart_sys::Dart_Handle;

use crate::{
    api::{dart::utils::DartResult, JasonError},
    platform,
};

use super::{utils::string_into_c_str, ForeignClass};

#[cfg(feature = "mockable")]
pub use self::mock::ConnectionHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::connection::ConnectionHandle;

impl ForeignClass for ConnectionHandle {}

/// Sets callback, invoked when this `Connection` will close.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_close(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_close(platform::Function::new(f))
        .map_err(JasonError::from)
        .into()
}

/// Sets callback, invoked when a new [`remote::Track`] is added to this
/// [`Connection`].
///
/// [`remote::Track`]: crate::media::track::remote::Track
/// [`Connection`]: crate::connection::Connection
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_remote_track_added(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_remote_track_added(platform::Function::new(f))
        .map_err(JasonError::from)
        .into()
}

/// Sets callback, invoked when a connection quality score is updated by
/// a server.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_quality_score_update(
    this: NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_quality_score_update(platform::Function::new(f))
        .map_err(JasonError::from)
        .into()
}

/// Returns remote `Member` ID.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: NonNull<ConnectionHandle>,
) -> DartResult {
    let this = this.as_ref();

    this.get_remote_member_id()
        .map_err(JasonError::from)
        .map(string_into_c_str)
        .into()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(
    this: NonNull<ConnectionHandle>,
) {
    drop(ConnectionHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::{
        api::{JasonError, RemoteMediaTrack},
        connection::{
            ConnectionError, ConnectionHandle as CoreConnectionHandle,
        },
        platform,
    };

    pub struct ConnectionHandle;

    impl From<CoreConnectionHandle> for ConnectionHandle {
        fn from(_: CoreConnectionHandle) -> Self {
            Self
        }
    }

    #[allow(clippy::missing_errors_doc)]
    impl ConnectionHandle {
        pub fn get_remote_member_id(&self) -> Result<String, JasonError> {
            Err(tracerr::new!(ConnectionError::Detached).into())
        }

        pub fn on_close(
            &self,
            f: platform::Function<()>,
        ) -> Result<(), JasonError> {
            f.call0();
            Ok(())
        }

        pub fn on_remote_track_added(
            &self,
            f: platform::Function<RemoteMediaTrack>,
        ) -> Result<(), JasonError> {
            f.call1(RemoteMediaTrack);
            Ok(())
        }

        pub fn on_quality_score_update(
            &self,
            f: platform::Function<u8>,
        ) -> Result<(), JasonError> {
            f.call1(4);
            Ok(())
        }
    }
}
