use std::ptr;

use dart_sys::Dart_Handle;
use tracerr::Traced;

use crate::{
    api::dart::utils::{new_handler_detached_error, DartError, DartResult},
    connection::ConnectionError,
    platform,
};

use super::ForeignClass;

#[cfg(feature = "mockable")]
pub use self::mock::ConnectionHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::connection::ConnectionHandle;

impl ForeignClass for ConnectionHandle {}

impl From<Traced<ConnectionError>> for DartError {
    fn from(err: Traced<ConnectionError>) -> Self {
        let (err, stacktrace) = err.into_parts();
        let stacktrace = stacktrace.to_string();
        match err {
            ConnectionError::Detached => unsafe {
                new_handler_detached_error(stacktrace)
            },
        }
    }
}

/// Sets callback, invoked when this `Connection` will close.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_close(
    this: ptr::NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_close(platform::Function::new(f))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked when a new [`remote::Track`] is added to this
/// [`Connection`].
///
/// [`remote::Track`]: crate::media::track::remote::Track
/// [`Connection`]: crate::connection::Connection
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_remote_track_added(
    this: ptr::NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_remote_track_added(platform::Function::new(f))
        .map_err(DartError::from)
        .into()
}

/// Sets callback, invoked when a connection quality score is updated by
/// a server.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__on_quality_score_update(
    this: ptr::NonNull<ConnectionHandle>,
    f: Dart_Handle,
) -> DartResult {
    let this = this.as_ref();

    this.on_quality_score_update(platform::Function::new(f))
        .map_err(DartError::from)
        .into()
}

/// Returns remote `Member` ID.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__get_remote_member_id(
    this: ptr::NonNull<ConnectionHandle>,
) -> DartResult {
    let this = this.as_ref();

    this.get_remote_member_id().map_err(DartError::from).into()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn ConnectionHandle__free(
    this: ptr::NonNull<ConnectionHandle>,
) {
    drop(ConnectionHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use tracerr::Traced;

    use crate::{
        api::RemoteMediaTrack,
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

    impl ConnectionHandle {
        pub fn get_remote_member_id(
            &self,
        ) -> Result<String, Traced<ConnectionError>> {
            Err(tracerr::new!(ConnectionError::Detached).into())
        }

        pub fn on_close(
            &self,
            f: platform::Function<()>,
        ) -> Result<(), Traced<ConnectionError>> {
            f.call0();
            Ok(())
        }

        pub fn on_remote_track_added(
            &self,
            f: platform::Function<RemoteMediaTrack>,
        ) -> Result<(), Traced<ConnectionError>> {
            f.call1(RemoteMediaTrack);
            Ok(())
        }

        pub fn on_quality_score_update(
            &self,
            f: platform::Function<u8>,
        ) -> Result<(), Traced<ConnectionError>> {
            f.call1(4);
            Ok(())
        }
    }
}
