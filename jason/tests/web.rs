#![cfg(target_arch = "wasm32")]

mod api;
mod peer;

use wasm_bindgen_test::*;

use medea_client_api_proto::{
    AudioSettings, Direction, MediaType, Track, VideoSettings,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn some_dummy_test() {
    assert_eq!("asd", "asd");
}

pub fn get_test_tracks() -> (Track, Track) {
    (
        Track {
            id: 1,
            direction: Direction::Send {
                receivers: vec![2],
                mid: None,
            },
            media_type: MediaType::Audio(AudioSettings {}),
        },
        Track {
            id: 2,
            direction: Direction::Send {
                receivers: vec![2],
                mid: None,
            },
            media_type: MediaType::Video(VideoSettings {}),
        },
    )
}
