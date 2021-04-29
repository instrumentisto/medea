use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dart_sys::Dart_Handle;
use futures::{channel::mpsc, prelude::stream::LocalBoxStream};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    platform::{
        dart::utils::{callback_listener::StringCallback, handle::DartHandle},
        RpcTransport, TransportError, TransportState,
    },
    rpc::{ApiUrl, ClientDisconnect},
    utils::dart::into_dart_string,
};

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

type NewFunction = extern "C" fn(*const libc::c_char) -> Dart_Handle;

type SendFunction = extern "C" fn(Dart_Handle, *const libc::c_char);

type CloseFunction = extern "C" fn(Dart_Handle, i32, *const libc::c_char);

type OnMessageFunction = extern "C" fn(Dart_Handle, Dart_Handle);

static mut NEW_FUNCTION: Option<NewFunction> = None;

static mut SEND_FUNCTION: Option<SendFunction> = None;

static mut CLOSE_FUNCTION: Option<CloseFunction> = None;

static mut ON_MESSAGE_FUNCTION: Option<OnMessageFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__send(f: SendFunction) {
    SEND_FUNCTION = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__close(
    f: CloseFunction,
) {
    CLOSE_FUNCTION = Some(f);
}

#[derive(Clone, Debug)]
pub struct WebSocketRpcTransport {
    handle: DartHandle,
    on_message_listeners: Rc<RefCell<Vec<mpsc::UnboundedSender<ServerMsg>>>>,
    close_reason: Cell<ClientDisconnect>,
}

impl WebSocketRpcTransport {
    pub async fn new(url: ApiUrl) -> Result<Self> {
        unsafe {
            let handle = NEW_FUNCTION.unwrap()(into_dart_string(
                url.as_ref().to_string(),
            ));
            let on_message_listeners: Rc<
                RefCell<Vec<mpsc::UnboundedSender<ServerMsg>>>,
            > = Rc::new(RefCell::new(Vec::new()));
            ON_MESSAGE_FUNCTION.unwrap()(
                handle,
                StringCallback::callback({
                    let on_message_listeners = Rc::clone(&on_message_listeners);
                    move |msg| {
                        let msg = match serde_json::from_str::<ServerMsg>(&msg)
                        {
                            Ok(parsed) => parsed,
                            Err(e) => {
                                // TODO: protocol versions mismatch? should drop
                                //       connection if so
                                log::error!("{}", tracerr::new!(e));
                                return;
                            }
                        };

                        on_message_listeners.borrow_mut().retain(
                            |on_message| {
                                on_message.unbounded_send(msg.clone()).is_ok()
                            },
                        );
                    }
                }),
            );
            Ok(Self {
                handle: DartHandle::new(handle),
                on_message_listeners,
                close_reason: Cell::new(
                    ClientDisconnect::RpcTransportUnexpectedlyDropped,
                ),
            })
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__on_message(
    f: OnMessageFunction,
) {
    ON_MESSAGE_FUNCTION = Some(f);
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.on_message_listeners.borrow_mut().push(tx);
        Box::pin(rx)
    }

    fn set_close_reason(&self, reason: ClientDisconnect) {
        self.close_reason.set(reason);
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>> {
        let msg = serde_json::to_string(msg).unwrap();
        unsafe {
            SEND_FUNCTION.unwrap()(self.handle.get(), into_dart_string(msg));
        }
        Ok(())
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, TransportState> {
        todo!()
    }
}

impl Drop for WebSocketRpcTransport {
    fn drop(&mut self) {
        let rsn = serde_json::to_string(&self.close_reason.get())
            .expect("Could not serialize close message");
        unsafe {
            CLOSE_FUNCTION.unwrap()(
                self.handle.get(),
                1000,
                into_dart_string(rsn),
            );
        }
    }
}