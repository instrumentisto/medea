#![cfg(target_arch = "wasm32")]

use medea_jason::rpc::{TransportError, WebSocketRpcTransport};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn bad_url_err() {
    use TransportError::*;

    match WebSocketRpcTransport::new("asd").await {
        Ok(_) => unreachable!(),
        Err(err) => match err.into_inner() {
            CreateSocket(err) => {
                assert_eq!(err.name, "SyntaxError");
            }
            _ => unreachable!(),
        },
    }
}

#[wasm_bindgen_test]
async fn could_not_init_socket_err() {
    use TransportError::*;

    match WebSocketRpcTransport::new("ws://0.0.0.0").await {
        Ok(_) => unreachable!(),
        Err(err) => match err.into_inner() {
            InitSocket => {}
            _ => unreachable!(),
        },
    }
}
