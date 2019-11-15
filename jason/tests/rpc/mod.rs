use std::{cell::RefCell, rc::Rc};

use futures::{
    channel::{mpsc, oneshot},
    future::Either,
    StreamExt,
};
use medea_client_api_proto::{ClientMsg, Event, PeerId, ServerMsg};
use medea_jason::rpc::{
    websocket::Error, CloseMsg, RpcClient, RpcClientImpl, RpcTransport,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::resolve_after;

wasm_bindgen_test_configure!(run_in_browser);

struct Inner {
    /// [`mpsc::UnboundedSender`]s for `on_message` callback of
    /// [`RpcTransportMock`].
    on_message: Vec<mpsc::UnboundedSender<Result<ServerMsg, Error>>>,

    /// [`oneshot::Sender`] for `on_close` callback of [`RpcTransportMock`].
    on_close: Option<oneshot::Sender<CloseMsg>>,

    /// [`oneshot::Sender`] with which all [`ClientMsg`]s from
    /// [`RpcTransport::send`] will be sent.
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
    /// Returns [`RpcTransportMock`] without any callbacks.
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Inner {
            on_message: Vec::new(),
            on_close: None,
            on_send: None,
        })))
    }

    /// Emulates receiving of [`ServerMsg`] by [`RpcTransport`] from a server.
    pub fn send_on_message(&self, msg: ServerMsg) {
        self.0
            .borrow()
            .on_message
            .iter()
            .for_each(|q| q.unbounded_send(Ok(msg.clone())).unwrap());
    }

    /// Returns [`mpsc::UnboundedReceiver`] which will receive all
    /// [`ClientMessage`]s which will be sent with [`RpcTransport::send`].
    fn on_send(&self) -> mpsc::UnboundedReceiver<ClientMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_send = Some(tx);
        rx
    }

    fn close(&self, close_msg: CloseMsg) {
        self.0.borrow_mut().on_close.take().map(|on_close| {
            on_close.send(close_msg).unwrap();
        });
    }
}

impl Drop for RpcTransportMock {
    fn drop(&mut self) {
        self.close(CloseMsg::Normal(String::new()));
    }
}

#[wasm_bindgen_test]
async fn on_message() {
    let rpc_transport = RpcTransportMock::new();
    let ws = RpcClientImpl::new(10);
    let mut stream = ws.subscribe();

    let server_event = Event::PeerCreated {
        peer_id: PeerId(0),
        tracks: vec![],
        ice_servers: vec![],
        sdp_offer: None,
    };
    let server_event_clone = server_event.clone();

    spawn_local(async move {
        assert_eq!(stream.next().await.unwrap(), server_event_clone);
    });
    ws.connect(Box::new(rpc_transport.clone())).await.unwrap();
    rpc_transport.send_on_message(ServerMsg::Event(server_event));
}

#[wasm_bindgen_test]
async fn heartbeat() {
    let rpc_transport = RpcTransportMock::new();
    let ws = RpcClientImpl::new(500);

    let mut on_send_stream = rpc_transport.on_send();
    ws.connect(Box::new(rpc_transport)).await.unwrap();

    let test_result = futures::future::select(
        Box::pin(async move {
            let mut ping_count = 0;
            while let Some(event) = on_send_stream.next().await {
                match event {
                    ClientMsg::Ping(_) => {
                        ping_count += 1;
                        if ping_count > 1 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }),
        Box::pin(resolve_after(600)),
    )
    .await;
    match test_result {
        Either::Left(_) => (),
        Either::Right(_) => panic!("Ping doesn't sent after ping interval."),
    }
}
