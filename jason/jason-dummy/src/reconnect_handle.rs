use crate::{
    room_handle::RoomHandle,
    utils::{spawn, Completer},
};
use dart_sys::Dart_Handle;

pub struct ReconnectHandle;

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

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_delay(
    this: *mut ReconnectHandle,
    delay_ms: i64,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.reconnect_with_delay(delay_ms as u32).await;
        completer.complete(())
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__reconnect_with_backoff(
    this: *mut ReconnectHandle,
    starting_delay: i64,
    multiplier: f32,
    max_delay: i64,
) -> Dart_Handle {
    let this = Box::from_raw(this);
    let completer: Completer<(), ()> = Completer::new();
    let fut = completer.future();
    spawn(async move {
        this.reconnect_with_backoff(
            starting_delay as u32,
            multiplier,
            max_delay as u32,
        )
        .await;
        completer.complete(())
    });
    fut
}

#[no_mangle]
pub unsafe extern "C" fn ReconnectHandle__free(this: *mut ReconnectHandle) {
    Box::from_raw(this);
}
