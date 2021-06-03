use std::{convert::TryFrom as _, ptr};

use tracerr::Traced;

use crate::{
    api::dart::utils::{ArgumentError, DartFuture, IntoDartFuture},
    rpc::ReconnectError,
};

use super::{
    utils::{DartError, StateError},
    ForeignClass,
};

#[cfg(feature = "mockable")]
pub use self::mock::ReconnectHandle;
#[cfg(not(feature = "mockable"))]
pub use crate::rpc::ReconnectHandle;

impl ForeignClass for ReconnectHandle {}

impl From<Traced<ReconnectError>> for DartError {
    #[inline]
    fn from(err: Traced<ReconnectError>) -> Self {
        match err.into_inner() {
            ReconnectError::Session(_) => {
                todo!()
            }
            ReconnectError::Detached => {
                StateError::new("ReconnectHandle is in detached state.").into()
            }
        }
    }
}

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
    this: ptr::NonNull<ReconnectHandle>,
    delay_ms: i64,
) -> DartFuture<Result<(), DartError>> {
    let this = this.as_ref().clone();

    async move {
        let delay_ms = u32::try_from(delay_ms).map_err(|_| {
            ArgumentError::new(delay_ms, "delayMs", "Expected u32")
        })?;

        this.reconnect_with_delay(delay_ms).await?;
        Ok(())
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
    this: ptr::NonNull<ReconnectHandle>,
    starting_delay: i64,
    multiplier: f64,
    max_delay: i64,
) -> DartFuture<Result<(), DartError>> {
    let this = this.as_ref().clone();

    async move {
        let starting_delay = u32::try_from(starting_delay).map_err(|_| {
            ArgumentError::new(
                starting_delay,
                "startingDelayMs",
                "Expected u32",
            )
        })?;
        let max_delay = u32::try_from(max_delay).map_err(|_| {
            ArgumentError::new(max_delay, "maxDelay", "Expected u32")
        })?;

        this.reconnect_with_backoff(starting_delay, multiplier, max_delay)
            .await?;
        Ok(())
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
pub unsafe extern "C" fn ReconnectHandle__free(
    this: ptr::NonNull<ReconnectHandle>,
) {
    drop(ReconnectHandle::from_ptr(this));
}

#[cfg(feature = "mockable")]
mod mock {
    use tracerr::Traced;

    use crate::rpc::{ReconnectError, ReconnectHandle as CoreReconnectHandle};

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
        ) -> Result<(), Traced<ReconnectError>> {
            Ok(())
        }

        pub async fn reconnect_with_backoff(
            &self,
            _starting_delay_ms: u32,
            _multiplier: f64,
            _max_delay: u32,
        ) -> Result<(), Traced<ReconnectError>> {
            Ok(())
        }
    }
}
