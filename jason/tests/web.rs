#![cfg(target_arch = "wasm32")]

/// Makes eq assertion and if it fails, sends error to provided tx.
///
/// Panics if send operation fails.
/// `$test_tx` - [`oneshot::Sender`] to which comparison error will be sent
///
/// `$a` - left item of comparision
///
/// `$b` - right item of comparision
macro_rules! callback_assert_eq {
    ($tx:tt, $a:expr, $b:expr) => {
        if $a != $b {
            $tx.send(Err(format!("{} != {}", $a, $b))).unwrap();
            return;
        }
    };
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
