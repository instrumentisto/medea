#![cfg(target_arch = "wasm32")]

use futures::{
    channel::{mpsc, oneshot},
    StreamExt,
};
use medea_client_api_proto::PeerId;
use medea_jason::{api, connection::Connections, platform};
use wasm_bindgen::{closure::Closure, JsValue};
use wasm_bindgen_test::*;

use crate::{
    get_audio_track, get_video_track, timeout, wait_and_check_test_result,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn on_new_connection_fires() {
    let cons = Connections::default();

    let (cb, test_result) = js_callback!(|handle: api::ConnectionHandle| {
        cb_assert_eq!(
            handle.get_remote_member_id().unwrap(),
            "bob".to_string()
        );
    });
    cons.on_new_connection(platform::Function::from(js_sys::Function::from(
        cb,
    )));

    cons.create_connection(PeerId(1), &"bob".into());

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn on_remote_track_added_fires() {
    let cons = Connections::default();

    cons.create_connection(PeerId(1), &"bob".into());

    let con = cons.get(&"bob".into()).unwrap();
    let con_handle = con.new_handle();
    let (cb, test_result) = js_callback!(|track: api::RemoteMediaTrack| {
        cb_assert_eq!(track.kind(), api::MediaKind::Video);
    });
    con_handle
        .on_remote_track_added(platform::Function::from(
            js_sys::Function::from(cb),
        ))
        .unwrap();

    con.add_remote_track(get_video_track().await.into());

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn tracks_are_added_to_connection() {
    let cons = Connections::default();

    cons.create_connection(PeerId(1), &"bob".into());

    let con = cons.get(&"bob".into()).unwrap();
    let con_handle = api::ConnectionHandle::from(con.new_handle());

    let (tx, rx) = oneshot::channel();
    let closure = Closure::once_into_js(move |track: api::RemoteMediaTrack| {
        assert!(tx.send(track).is_ok());
    });
    con_handle.on_remote_track_added(closure.into()).unwrap();

    con.add_remote_track(get_video_track().await.into());
    let video_track = timeout(100, rx).await.unwrap().unwrap();
    assert_eq!(video_track.kind(), api::MediaKind::Video);

    let (tx, rx) = oneshot::channel();
    let closure = Closure::once_into_js(move |track: api::RemoteMediaTrack| {
        assert!(tx.send(track).is_ok());
    });
    con_handle.on_remote_track_added(closure.into()).unwrap();
    con.add_remote_track(get_audio_track().await.into());
    let audio_track = timeout(200, rx).await.unwrap().unwrap();
    assert_eq!(audio_track.kind(), api::MediaKind::Audio);
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
    con_handle
        .on_close(platform::Function::from(js_sys::Function::from(on_close)))
        .unwrap();

    cons.close_connection(PeerId(1));

    wait_and_check_test_result(test_result, || {}).await;
}

#[wasm_bindgen_test]
async fn two_peers_in_one_connection_works() {
    let cons = Connections::default();

    let (test_tx, mut test_rx) = mpsc::unbounded();
    let on_new_connection =
        Closure::wrap(Box::new(move |_: api::ConnectionHandle| {
            test_tx.unbounded_send(()).unwrap();
        }) as Box<dyn Fn(api::ConnectionHandle)>)
        .into_js_value();
    cons.on_new_connection(platform::Function::from(js_sys::Function::from(
        on_new_connection,
    )));

    cons.create_connection(PeerId(1), &"bob".into());
    test_rx.next().await.unwrap();

    cons.create_connection(PeerId(2), &"bob".into());
    timeout(300, test_rx.next()).await.unwrap_err();
}

#[wasm_bindgen_test]
async fn create_two_connections() {
    let cons = Connections::default();

    let (test_tx, mut test_rx) = mpsc::unbounded();
    let on_new_connection =
        Closure::wrap(Box::new(move |_: api::ConnectionHandle| {
            test_tx.unbounded_send(()).unwrap();
        }) as Box<dyn Fn(api::ConnectionHandle)>)
        .into_js_value();
    cons.on_new_connection(platform::Function::from(js_sys::Function::from(
        on_new_connection,
    )));

    cons.create_connection(PeerId(1), &"bob".into());
    test_rx.next().await.unwrap();

    cons.create_connection(PeerId(2), &"alice".into());
    timeout(300, test_rx.next()).await.unwrap();
}
