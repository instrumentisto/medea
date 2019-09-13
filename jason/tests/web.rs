#![cfg(target_arch = "wasm32")]

mod api;
mod peer;

use futures::{future::IntoFuture, sync::oneshot::channel, Future};
use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, PeerId, Track, TrackId, VideoSettings,
};
use medea_jason::utils::{window, WasmErr};
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

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

pub fn resolve_after(delay: i32) -> impl Future<Item = (), Error = JsValue> {
    let (done, wait) = channel();
    let cb = Closure::once_into_js(move || {
        done.send(()).unwrap();
    });
    window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            &cb.into(),
            delay,
        )
        .unwrap();
    wait.into_future()
        .map_err(|_| WasmErr::from("canceled").into())
}
