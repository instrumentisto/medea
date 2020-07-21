#![cfg(target_arch = "wasm32")]

use futures::channel::oneshot;
use medea_client_api_proto::{PeerId, TrackId};
use medea_jason::{
    api::{ConnectionHandle, Connections},
    peer::RemoteMediaStream,
};
use wasm_bindgen::{closure::Closure, JsValue};
use wasm_bindgen_test::*;

use crate::{
    get_audio_track, get_video_track, timeout, wait_and_check_test_result,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn on_new_connection_fires() {
    let cons = Connections::default();

    let (cb, test_result) = js_callback!(|handle: ConnectionHandle| {
        cb_assert_eq!(
            handle.get_remote_member_id().unwrap(),
            "bob".to_string()
        );
    });
    cons.on_new_connection(cb.into());

    cons.create_connection(PeerId(1), &"bob".into());

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_remote_stream_fires() {
    let cons = Connections::default();

    cons.create_connection(PeerId(1), &"bob".into());

    let con = cons.get(&"bob".into()).unwrap();
    let con_handle = con.new_handle();

    let (cb, test_result) = js_callback!(|stream: RemoteMediaStream| {
        cb_assert_eq!(
            stream
                .get_media_stream()
                .unwrap()
                .get_video_tracks()
                .length(),
            1
        );
    });
    con_handle.on_remote_stream(cb.into()).unwrap();

    con.add_remote_track(TrackId(1), get_video_track().await);

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn tracks_are_added_to_remote_stream() {
    let cons = Connections::default();

    cons.create_connection(PeerId(1), &"bob".into());

    let con = cons.get(&"bob".into()).unwrap();
    let con_handle = con.new_handle();

    let (tx, rx) = oneshot::channel();
    let closure = Closure::once_into_js(move |stream: RemoteMediaStream| {
        assert!(tx.send(stream).is_ok());
    });
    con_handle.on_remote_stream(closure.into()).unwrap();

    con.add_remote_track(TrackId(1), get_video_track().await);

    let stream = timeout(100, rx).await.unwrap().unwrap();
    let stream = stream.get_media_stream().unwrap();
    assert_eq!(stream.get_tracks().length(), 1);

    con.add_remote_track(TrackId(2), get_audio_track().await);
    assert_eq!(stream.get_tracks().length(), 2);
}

#[wasm_bindgen_test]
async fn on_closed_fires() {
    let cons = Connections::default();
    cons.create_connection(PeerId(1), &"bob".into());
    let con = cons.get(&"bob".into()).unwrap();
    let con_handle = con.new_handle();

    let (on_close, test_result) = js_callback!(|nothing: JsValue| {
        cb_assert_eq!(nothing.is_undefined(), true);
    });
    con_handle.on_close(on_close.into()).unwrap();

    cons.close_connection(PeerId(1));

    wait_and_check_test_result(test_result, || {}).await;
}
