use dart_sys::Dart_Handle;

use crate::{utils::future_to_dart, ForeignClass};

pub struct ReconnectHandle;

impl ForeignClass for ReconnectHandle {}

impl ReconnectHandle {
    pub async fn reconnect_with_delay(&self, _delay_ms: u32) {
        // Result<(), JasonError>
    }

    pub async fn reconnect_with_backoff(
        &self,
        _starting_delay_ms: u32,
        _multiplier: f32,
        _max_delay: u32,
    ) {
        // Result<(), JasonError>
    }
}

/// Tries to reconnect after the provided delay in milliseconds.
///
/// If [`Room`] is already reconnecting then new reconnection attempt won't be
/// performed. Instead, it will wait for the first reconnection attempt result
/// and use it here.
///
/// [`Room`]: crate::room::Room
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: *mut ReconnectHandle,
    delay_ms: i64, // TODO: must check for cast_sign_loss
) -> Dart_Handle {
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.reconnect_with_delay(delay_ms as u32).await;
        Ok::<_, ()>(())
    })
}

/// Tries to reconnect [`Room`] in a loop with a growing backoff delay.
///
/// The first attempt to reconnect is guaranteed to happen no earlier than
/// `starting_delay_ms`.
///
/// Also, it guarantees that delay between reconnection attempts won't be
/// greater than `max_delay_ms`.
///
/// After each reconnection attempt, delay between reconnections will be
/// multiplied by the given `multiplier` until it reaches `max_delay_ms`.
///
/// If [`Room`] is already reconnecting then new reconnection attempt won't be
/// performed. Instead, it will wait for the first reconnection attempt result
/// and use it here.
///
/// If `multiplier` is negative number than `multiplier` will be considered
/// as `0.0`.
///
/// [`Room`]: crate::room::Room
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_backoff(
    this: *mut ReconnectHandle,
    starting_delay: i64, // TODO: must check for cast_sign_loss
    multiplier: f32,
    max_delay: i64,
) -> Dart_Handle {
    let this = this.as_ref().unwrap();

    future_to_dart(async move {
        this.reconnect_with_backoff(
            starting_delay as u32,
            multiplier,
            max_delay as u32,
        )
        .await;
        Ok::<_, ()>(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: *mut ReconnectHandle) {
    ReconnectHandle::from_ptr(this);
}
