#![cfg(target_arch = "wasm32")]

/// Analog for [`assert_eq`] but for [`js_callback`] macro.
/// Simply use it as [`assert_eq`]. For use cases and reasons
/// just read [`js_callback`]'s docs.
///
/// __Use it only in [`js_callback`]'s closures.__
macro_rules! cb_assert_eq {
    ($a:expr, $b:expr) => {
        if $a != $b {
            return Err(format!("{} != {}", $a, $b));
        }
    };
}

/// Macro which generates [`wasm_bindgen::closure::Closure`] with provided
/// [`FnOnce`] and [`futures::channel::oneshot::Receiver`] which will receive
/// result of assertions from provided [`FnOnce`]. In provided [`FnOnce`] you
/// may use [`cb_assert_eq`] macro in same way as a vanilla [`assert_eq`].
/// Result of this assertions will be returned with
/// [`futures::channel::oneshot::Receiver`]. You may simply use
/// [`wait_and_check_test_result`] which will `panic` if some assertion failed.
///
/// # Use cases
///
/// This macro is useful in tests which should check that JS callback provided
/// in some function was called and some assertions was passed. We can't use
/// habitual `assert_eq` because panics from [`wasm_bindgen::closure::Closure`]
/// will not fails `wasm_bindgen_test`'s tests.
///
/// # Example
///
/// ```ignore
/// let room: Room = get_room();
/// let mut room_handle: RoomHandle = room.new_handle();
///
/// // Create 'Closure' which we can provide as JS closure and
/// // 'Future' which will be resolved with assertions result.
/// let (cb, test_result) = js_callback!(|closed: JsValue| {
///     // You can write here any Rust code which you need.
///     // The only difference is that within this macro
///     // you can use 'cb_assert_eq!'.
///     let closed_reason = get_reason(&closed);
///     cb_assert_eq!(closed_reason, "RoomUnexpectedlyDropped");
///
///     cb_assert_eq!(get_is_err(&closed), false);
///     cb_assert_eq!(get_is_closed_by_server(&closed), false);
/// });
/// room_handle.on_close(cb.into()).unwrap();
///
/// room.close(CloseReason::ByClient {
///     reason: ClientDisconnect::RoomUnexpectedlyDropped,
///     is_err: false,
/// });
///
/// // Wait for assertions results and check it ('wait_and_check_test_result'
/// // will panic if assertion errored).
/// wait_and_check_test_result(test_result).await;
/// ```
macro_rules! js_callback {
    (|$($arg_name:ident: $arg_type:ty),*| $body:block) => {{
        let (test_tx, test_rx) = futures::channel::oneshot::channel();
        let closure = wasm_bindgen::closure::Closure::once_into_js(
            move |$($arg_name: $arg_type),*| {
                let test_fn = || {
                    $body;
                    Ok(())
                };
                test_tx.send((test_fn)()).unwrap();
            }
        );

        (closure, test_rx)
    }}
}

mod api;
mod media;
mod peer;
mod rpc;

use anyhow::Result;
use futures::{channel::oneshot, future::Either};
use js_sys::Promise;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, PeerId, Track, TrackId, VideoSettings,
};
use medea_jason::utils::window;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen(module = "/tests/mock_navigator.js")]
extern "C" {
    pub type MockNavigator;

    #[wasm_bindgen(constructor)]
    pub fn new() -> MockNavigator;

    #[wasm_bindgen(method, setter = errorGetUserMedia)]
    fn error_get_user_media(this: &MockNavigator, err: JsValue);

    #[wasm_bindgen(method, setter = errorEnumerateDevices)]
    fn error_enumerate_devices(this: &MockNavigator, err: JsValue);

    #[wasm_bindgen(method)]
    fn stop(this: &MockNavigator);
}

pub fn get_test_tracks() -> (Track, Track) {
    (
        Track {
            id: TrackId(1),
            direction: Direction::Send {
                receivers: vec![PeerId(2)],
                mid: None,
            },
            media_type: MediaType::Audio(AudioSettings {}),
        },
        Track {
            id: TrackId(2),
            direction: Direction::Send {
                receivers: vec![PeerId(2)],
                mid: None,
            },
            media_type: MediaType::Video(VideoSettings {}),
        },
    )
}

/// Resolves after provided number of milliseconds.
pub async fn resolve_after(delay_ms: i32) -> Result<(), JsValue> {
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes, delay_ms,
            )
            .unwrap();
    }))
    .await?;
    Ok(())
}

/// Waits for [`Result`] from [`oneshot::Receiver`] with tests result with 1
/// second deadline.
///
/// Panics if deadline is exceeded or provided rx resolves to `Err`.
pub async fn wait_and_check_test_result(
    rx: oneshot::Receiver<Result<(), String>>,
) {
    let result =
        futures::future::select(Box::pin(rx), Box::pin(resolve_after(500)))
            .await;
    match result {
        Either::Left((rx_result, _)) => {
            rx_result.unwrap().unwrap();
        }
        Either::Right(_) => {
            panic!("on_close callback didn't fired");
        }
    };
}
