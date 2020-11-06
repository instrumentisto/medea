#![cfg(target_arch = "wasm32")]

use medea_jason::rpc::{TransportError, WebSocketRpcTransport};
use url::Url;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn could_not_init_socket_err() {
    use TransportError::*;

    match WebSocketRpcTransport::new(
        Url::parse("ws://0.0.0.0:60000").unwrap().into(),
    )
    .await
    {
        Ok(_) => unreachable!(),
        Err(err) => match err.into_inner() {
            InitSocket => {}
            _ => unreachable!(),
        },
    }
}
