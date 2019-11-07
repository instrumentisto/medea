#![cfg(target_arch = "wasm32")]

mod api;
mod media;
mod peer;

use anyhow::Result;
use futures::channel::oneshot;
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, PeerId, Track, TrackId, VideoSettings,
};
use medea_jason::utils::{window, JasonError};
use wasm_bindgen::prelude::*;
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

    #[wasm_bindgen]
    fn unwrap_error(err: JsValue) -> JasonError;
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

pub async fn resolve_after(delay: i32) -> Result<()> {
    let (done, wait) = oneshot::channel();
    let cb = Closure::once_into_js(move || {
        done.send(()).unwrap();
    });
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            &cb.into(),
            delay,
        )
        .unwrap();

    Ok(wait.await?)
}
