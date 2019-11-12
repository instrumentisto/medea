use crate::resolve_after;
use futures::{
    channel::{mpsc, oneshot},
    future::Either,
    SinkExt, StreamExt, TryFutureExt,
};
use medea_client_api_proto::{ClientMsg, Event, PeerId, ServerMsg};
use medea_jason::rpc::{
    websocket::{Error, MockRpcTransport, RpcTransport},
    CloseMsg, RpcClient, WebsocketRpcClient,
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

struct Inner {
    on_message: Vec<mpsc::UnboundedSender<Result<ServerMsg, Error>>>,
    on_close: Option<oneshot::Sender<CloseMsg>>,
    on_send: Option<mpsc::UnboundedSender<ClientMsg>>,
}

#[derive(Clone)]
struct RpcTransportMock(Rc<RefCell<Inner>>);

impl RpcTransport for RpcTransportMock {
    fn on_message(
        &self,
    ) -> Result<mpsc::UnboundedReceiver<Result<ServerMsg, Error>>, Error> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message.push(tx);
        Ok(rx)
    }

    fn on_close(&self) -> Result<oneshot::Receiver<CloseMsg>, Error> {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close = Some(tx);
        Ok(rx)
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), Error> {
        self.0
            .borrow()
            .on_send
            .as_ref()
            .map(|q| q.unbounded_send(msg.clone()));
        Ok(())
    }
}

impl RpcTransportMock {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Inner {
            on_message: Vec::new(),
            on_close: None,
            on_send: None,
        })))
    }

    pub fn send_on_message(&self, msg: ServerMsg) {
        self.0
            .borrow()
            .on_message
            .iter()
            .for_each(|q| q.unbounded_send(Ok(msg.clone())).expect("asdf"));
    }

    fn on_send(&self) -> mpsc::UnboundedReceiver<ClientMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_send = Some(tx);
        rx
    }
}

#[wasm_bindgen_test]
async fn on_message() {
    let rpc_transport = RpcTransportMock::new();
    let ws = WebsocketRpcClient::new(10);
    let mut stream = ws.subscribe();
    let server_event = Event::PeerCreated {
        peer_id: PeerId(0),
        tracks: vec![],
        ice_servers: vec![],
        sdp_offer: None,
    };
    let server_event_clone = server_event.clone();
    spawn_local(async move {
        stream.next().await;
    });
    ws.connect(Box::new(rpc_transport.clone())).await;
    rpc_transport.send_on_message(ServerMsg::Event(server_event));
}

#[wasm_bindgen_test]
async fn heartbeat() {
    let rpc_transport = RpcTransportMock::new();
    let ws = WebsocketRpcClient::new(500);

    let mut on_send_stream = rpc_transport.on_send();
    ws.connect(Box::new(rpc_transport)).await;

    let res = futures::future::select(
        Box::pin(async move {
            let mut ping_count = 0;
            while let Some(event) = on_send_stream.next().await {
                match event {
                    ClientMsg::Ping(_) => {
                        if ping_count > 0 {
                            break;
                        } else {
                            ping_count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }),
        Box::pin(resolve_after(600)),
    )
    .await;
    match res {
        Either::Left(_) => (),
        Either::Right(_) => panic!(),
    }
}
