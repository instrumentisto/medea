use std::ptr::NonNull;

use dart_sys::Dart_Handle;

use crate::api::dart::utils::IntoDartFuture;

use super::ForeignClass;

#[cfg(feature = "mockable")]
pub use self::mock::ReconnectHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::rpc::ReconnectHandle;

impl ForeignClass for ReconnectHandle {}

/// Tries to reconnect a [`Room`] after the provided delay in milliseconds.
///
/// If the [`Room`] is already reconnecting then new reconnection attempt won't
/// be performed. Instead, it will wait for the first reconnection attempt
/// result and use it here..
///
/// [`Room`]: crate::room::Room
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: NonNull<ReconnectHandle>,
    delay_ms: i64, // TODO: must check for cast_sign_loss
) -> Dart_Handle {
    let this = this.as_ref().clone();

    async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.reconnect_with_delay(delay_ms as u32).await.unwrap();
        Ok::<_, ()>(())
    }
    .into_dart_future()
}

/// Tries to reconnect a [`Room`] in a loop with a growing backoff delay.
///
/// The first attempt to reconnect is guaranteed to happen not earlier than
/// `starting_delay_ms`.
///
/// Also, it guarantees that delay between reconnection attempts won't be
/// greater than `max_delay_ms`.
///
/// After each reconnection attempt, delay between reconnections will be
/// multiplied by the given `multiplier` until it reaches `max_delay_ms`.
///
/// If the [`Room`] is already reconnecting then new reconnection attempt won't
/// be performed. Instead, it will wait for the first reconnection attempt
/// result and use it here.
///
/// If `multiplier` is negative number then `multiplier` will be considered as
/// `0.0`.
///
/// [`Room`]: crate::room::Room
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_backoff(
    this: NonNull<ReconnectHandle>,
    starting_delay: i64, // TODO: must check for cast_sign_loss
    multiplier: f32,
    max_delay: i64,
) -> Dart_Handle {
    let this = this.as_ref().clone();

    async move {
        // TODO: Remove unwrap when propagating errors from Rust to Dart is
        //       implemented.
        this.reconnect_with_backoff(
            starting_delay as u32,
            multiplier,
            max_delay as u32,
        )
        .await
        .unwrap();
        Ok::<_, ()>(())
    }
    .into_dart_future()
}

/// Frees the data behind the provided pointer.
///
/// # Safety
///
/// Should be called when object is no longer needed. Calling this more than
/// once for the same pointer is equivalent to double free.
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: NonNull<ReconnectHandle>) {
    drop(ReconnectHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use crate::{
        api::dart::JasonError, rpc::ReconnectHandle as CoreReconnectHandle,
    };

    #[derive(Clone)]
    pub struct ReconnectHandle;

    impl From<CoreReconnectHandle> for ReconnectHandle {
        fn from(_: CoreReconnectHandle) -> Self {
            Self
        }
    }

    impl ReconnectHandle {
        pub async fn reconnect_with_delay(
            &self,
            _delay_ms: u32,
        ) -> Result<(), JasonError> {
            Ok(())
        }

        pub async fn reconnect_with_backoff(
            &self,
            _starting_delay_ms: u32,
            _multiplier: f32,
            _max_delay: u32,
        ) -> Result<(), JasonError> {
            Ok(())
        }
    }
}
