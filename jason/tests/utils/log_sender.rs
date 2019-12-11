//! Tests for [`medea_jason::utils::LogSender`].

use std::rc::Rc;

use futures::{future, FutureExt as _};
use js_sys::JsString;
use mockall::predicate;
use wasm_bindgen_test::*;

use crate::resolve_after;
use medea_jason::utils::{
    window, HTTPClientError, LogSender, MockHTTPClient, JASON_LOG_KEY,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn push_to_store() {
    let store = window().local_storage().unwrap().unwrap();
    store.clear().unwrap();
    store.set_item(JASON_LOG_KEY, "exists_value").unwrap();
    let value = JsString::from("new_value");
    LogSender::push_to_store(value);
    let store = window().local_storage().unwrap().unwrap();
    assert_eq!(
        &store.get_item(JASON_LOG_KEY).unwrap().unwrap(),
        "exists_value,new_value"
    );
}

#[wasm_bindgen_test]
async fn send_log_if_exists_in_store() {
    let store = window().local_storage().unwrap().unwrap();
    store.clear().unwrap();
    let mut client = MockHTTPClient::new();
    let log = "test_log";
    let expected_body = format!("[{}]", log);
    client
        .expect_send()
        .withf(move |s| {
            assert_eq!(*s, expected_body);
            true
        })
        .return_once(|_| future::pending().boxed());
    let sender = LogSender::new(Rc::new(client), 100);
    let value = JsString::from(log);
    LogSender::push_to_store(value);
    resolve_after(120).await.unwrap();
}

#[wasm_bindgen_test]
async fn back_log_to_store_if_send_fail() {
    let store = window().local_storage().unwrap().unwrap();
    store.clear().unwrap();
    let mut client = MockHTTPClient::new();
    let log = "test_log";
    let expected_body = format!("[{}]", log);
    client
        .expect_send()
        .withf(move |s| {
            assert_eq!(*s, expected_body);
            let local = window().local_storage().unwrap().unwrap();
            assert_eq!(local.get_item(JASON_LOG_KEY).unwrap(), None);
            true
        })
        .return_once(|_| {
            future::ready(Err(tracerr::new!(HTTPClientError::ResponseFailed {
                code: 400,
                message: "error".to_string(),
            })))
            .boxed()
        });
    let sender = LogSender::new(Rc::new(client), 100);
    LogSender::push_to_store(JsString::from(log));
    resolve_after(150).await.unwrap();
    assert_eq!(store.get_item(JASON_LOG_KEY).unwrap().unwrap(), "test_log");
}
