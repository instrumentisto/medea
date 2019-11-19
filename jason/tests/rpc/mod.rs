//! Tests for [`medea_jason::rpc::RpcClient`].

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures::{
    channel::{mpsc, oneshot},
    future::{self, Either},
    StreamExt,
};
use medea_client_api_proto::{ClientMsg, Command, Event, PeerId, ServerMsg};
use medea_jason::rpc::{
    CloseMsg, PinFuture, PinStream, RpcClient, RpcTransport, TransportError,
    WebSocketRpcClient,
};
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen_test::*;

use crate::resolve_after;

wasm_bindgen_test_configure!(run_in_browser);

struct Inner {
    /// [`mpsc::UnboundedSender`]s for `on_message` callback of
    /// [`RpcTransportMock`].
    on_message: Vec<mpsc::UnboundedSender<Result<ServerMsg, TransportError>>>,

    /// [`oneshot::Sender`] for `on_close` callback of [`RpcTransportMock`].
    on_close: Option<oneshot::Sender<CloseMsg>>,

    /// [`oneshot::Sender`] with which all [`ClientMsg`]s from
    /// [`RpcTransport::send`] will be sent.
    on_send: Option<mpsc::UnboundedSender<ClientMsg>>,

    /// [`CloseMsg`] which will be returned in `on_close` callback when
    /// [`RpcTransportMock`] will be dropped.
    ///
    /// If `None` then `CloseMsg::Normal(String::new())` will be sent.
    on_close_reason: Option<CloseMsg>,
}

/// Test mock for [`RpcTrasport`].
#[derive(Clone)]
struct RpcTransportMock(Rc<RefCell<Inner>>);

impl RpcTransport for RpcTransportMock {
    fn on_message(
        &self,
    ) -> Result<PinStream<Result<ServerMsg, TransportError>>, TransportError>
    {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().on_message.push(tx);
        Ok(Box::pin(rx))
    }

    fn on_close(
        &self,
    ) -> Result<PinFuture<Result<CloseMsg, oneshot::Canceled>>, TransportError>
    {
        let (tx, rx) = oneshot::channel();
        self.0.borrow_mut().on_close = Some(tx);
        Ok(Box::pin(rx))
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), TransportError> {
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
            on_close_reason: None,
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

    /// Sets [`CloseMsg`] which will be returned in `on_close` callback when
    /// [`RpcTransportMock`] will be dropped.
    #[allow(dead_code)]
    fn set_on_close_reason(&self, close_msg: CloseMsg) {
        self.0.borrow_mut().on_close_reason = Some(close_msg);
    }
}

impl Drop for RpcTransportMock {
    fn drop(&mut self) {
        let close_msg = self
            .0
            .borrow_mut()
            .on_close_reason
            .take()
            .unwrap_or_else(|| CloseMsg::Normal(String::new()));
        self.0.borrow_mut().on_close.take().map(|on_close| {
            on_close.send(close_msg).unwrap();
        });
    }
}

/// Tests [`WebSocketRpcClient::subscribe`] function.
///
/// # Algorithm:
///
/// 1. Connect [`WebSocketRpcClient`] with [`RpcTransportMock`]
/// 2. Subscribe to [`Event`]s with [`WebSocketRpcClient::subscribe`]
/// 3. Send [`Event`] with [`RpcTransportMock::send_on_message`]
/// 4. Check that subscriber from step 2 receives this [`Event`]
#[wasm_bindgen_test]
async fn message_received_from_transport_is_transmitted_to_sub() {
    let rpc_transport = RpcTransportMock::new();
    let ws = WebSocketRpcClient::new(10);
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
    ws.connect(Rc::new(rpc_transport.clone())).await.unwrap();
    rpc_transport.send_on_message(ServerMsg::Event(server_event));
}

/// Tests that [`WebSocketRpcClient`] sends [`Event::Ping`] to a server.
///
/// # Algorithm
///
/// 1. Connect [`WebSocketRpcClient`] with [`RpcTransportMock`]
/// 2. Subscribe to [`ClientMsg`]s which [`WebSocketRpcClient`] will send with
/// [`RpcTransportMock::on_send`] 3. Wait 600ms for [`ClientMsg::Ping`]
#[wasm_bindgen_test]
async fn heartbeat() {
    let rpc_transport = Rc::new(RpcTransportMock::new());
    let ws = WebSocketRpcClient::new(500);

    let mut on_send_stream = rpc_transport.on_send();
    ws.connect(rpc_transport).await.unwrap();

    let test_result = future::select(
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
        Either::Right(_) => panic!("Ping doesn't sent during ping interval."),
    }
}

/// Tests [`WebSocketRpcClient::unsub`] function.
///
/// # Algorithm
///
/// 1. Subscribe to [`Event`]s with [`WebSocketRpcClient::subscribe`]
/// 2. Call [`WebSocketRpcClient::unsub`]
/// 3. Wait for `None` received from [`WebSocketRpcClient::subscribe`]'s
/// `Stream`
#[wasm_bindgen_test]
async fn unsub_drops_subs() {
    let ws = WebSocketRpcClient::new(500);
    let (test_tx, test_rx) = oneshot::channel();
    let mut subscriber_stream = ws.subscribe();
    spawn_local(async move {
        loop {
            match subscriber_stream.next().await {
                Some(_) => (),
                None => {
                    test_tx.send(()).unwrap();
                    break;
                }
            }
        }
    });
    ws.unsub();

    match future::select(Box::pin(test_rx), Box::pin(resolve_after(1000))).await
    {
        Either::Left(_) => (),
        Either::Right(_) => panic!(
            "'unsub_drops_sub' lasts more that 1s. Most likely 'unsub' is \
             broken."
        ),
    }
}

/// Tests that [`RpcTransport`] will be dropped when [`WebSocketRpcClient`] was
/// dropped.
///
/// # Algorithm
///
/// 1. Create [`WebSocketRpcClient`] with [`RpcTransportMock`] [`Rc`]
/// 2. Drop [`WebSocketRpcClient`]
/// 3. Check that [`RpcTransportMock`]'s [`Rc`] now have only 1
/// [`Rc::strong_count`]
#[wasm_bindgen_test]
async fn transport_is_dropped_when_client_is() {
    let rpc_transport = Rc::new(RpcTransportMock::new());
    let ws = WebSocketRpcClient::new(500);
    ws.connect(rpc_transport.clone()).await.unwrap();
    std::mem::drop(ws);
    assert_eq!(Rc::strong_count(&rpc_transport), 1);
}

/// Tests [`WebSocketRpcClient::send_command`] function.
///
/// # Algorithm
///
/// 1. Connect [`WebSocketRpcClient`] with [`RpcTransportMock`]
/// 2. Subscribe to [`ClientMsg`]s with [`RpcTransportMock::on_send`]
/// 3. Send [`ClientMsg`] with [`WebSocketRpcClient::send_command`]
/// 4. Check that this message received by [`RpcTransportMock`] with
/// [`RpcTransportMock::on_send`] from step 2
#[wasm_bindgen_test]
async fn send_goes_to_transport() {
    let rpc_transport = Rc::new(RpcTransportMock::new());
    let ws = WebSocketRpcClient::new(500);
    ws.connect(rpc_transport.clone()).await.unwrap();
    let (test_tx, test_rx) = oneshot::channel();
    let test_peer_id = PeerId(9999);
    let test_sdp_offer = "Hello world!".to_string();
    let test_cmd = Command::MakeSdpOffer {
        peer_id: test_peer_id.clone(),
        sdp_offer: test_sdp_offer.clone(),
        mids: HashMap::new(),
    };

    let mut on_send_receiver = rpc_transport.on_send();
    spawn_local(async move {
        while let Some(msg) = on_send_receiver.next().await {
            match msg {
                ClientMsg::Command(cmd) => match cmd {
                    Command::MakeSdpOffer {
                        peer_id,
                        sdp_offer,
                        mids: _,
                    } => {
                        assert_eq!(&peer_id, &test_peer_id);
                        assert_eq!(&sdp_offer, &test_sdp_offer);
                        test_tx.send(()).unwrap();
                        break;
                    }
                    _ => (),
                },
                _ => (),
            }
        }
    });

    ws.send_command(test_cmd);

    match future::select(Box::pin(test_rx), Box::pin(resolve_after(1000))).await
    {
        Either::Left(_) => (),
        Either::Right(_) => {
            panic!("Command doesn't reach 'RpcTransport' within a 1s.")
        }
    }
}
