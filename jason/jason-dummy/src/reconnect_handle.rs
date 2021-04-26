use dart_sys::Dart_Handle;

use crate::{utils::into_dart_future, ForeignClass};

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

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: *mut ReconnectHandle,
    delay_ms: i64,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    into_dart_future(async move {
        this.reconnect_with_delay(delay_ms as u32).await;
        Ok::<(), ()>(())
    })
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_backoff(
    this: *mut ReconnectHandle,
    starting_delay: i64,
    multiplier: f32,
    max_delay: i64,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    into_dart_future(async move {
        this.reconnect_with_backoff(
            starting_delay as u32,
            multiplier,
            max_delay as u32,
        )
        .await;
        Ok::<(), ()>(())
    })
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: *mut ReconnectHandle) {
    ReconnectHandle::from_ptr(this);
}
