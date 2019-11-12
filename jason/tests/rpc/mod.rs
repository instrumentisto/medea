use medea_jason::rpc::{
    WebsocketRpcClient,
    websocket::{RpcTransportMock, Error},
};
use medea_client_api_proto::ServerMsg;
use wasm_bindgen_futures::spawn_local;

pub async fn get_mocker_rpc_client() -> WebsocketRpcClient {
    let rpc_transport = RpcTransportMock::new();
    let (message_sender, message_receiver) = futures::channel::mpsc::unbounded();
    rpc_transport.expect_on_message()
        .return_once(move |f: Box<dyn FnMut(Result<ServerMsg, Error>)>| {
            spawn_local(message_receiver.map(|message: ServerMsg| {
                (f)(Ok(message))
            }))
        });
}