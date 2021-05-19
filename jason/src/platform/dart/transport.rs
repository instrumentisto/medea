use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dart_sys::Dart_Handle;
use futures::{channel::mpsc, prelude::stream::LocalBoxStream};
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    api::dart::utils::{c_str_into_string, string_into_c_str},
    platform::{
        dart::utils::{callback_listener::StringCallback, handle::DartHandle},
        RpcTransport, TransportError, TransportState,
    },
    rpc::{ApiUrl, ClientDisconnect},
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
    on_message_listeners: OnMessageListeners,
    close_reason: Cell<ClientDisconnect>,
}

#[no_mangle]
pub unsafe extern "C" fn WebSocketRpcTransport__on_message(
    listeners: *const OnMessageListeners,
    msg: *const libc::c_char,
) {
    let listeners = listeners.as_ref().unwrap();
    listeners.notify_all(c_str_into_string(msg));
}

#[derive(Clone, Debug)]
pub struct OnMessageListeners(
    Rc<RefCell<Vec<mpsc::UnboundedSender<ServerMsg>>>>,
);

impl OnMessageListeners {
    fn new() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }

    fn notify_all(&self, msg: String) {
        let msg = match serde_json::from_str::<ServerMsg>(&msg) {
            Ok(parsed) => parsed,
            Err(e) => {
                // TODO: protocol versions mismatch? should drop
                //       connection if so
                log::error!("{}", tracerr::new!(e));
                return;
            }
        };

        self.0.borrow_mut().retain(|on_message| {
            on_message.unbounded_send(msg.clone()).is_ok()
        });
    }

    fn new_subscriber(&self) -> LocalBoxStream<'static, ServerMsg> {
        let (tx, rx) = mpsc::unbounded();
        self.0.borrow_mut().push(tx);
        Box::pin(rx)
    }
}

impl WebSocketRpcTransport {
    pub async fn new(url: ApiUrl) -> Result<Self> {
        unsafe {
            let handle = NEW_FUNCTION.unwrap()(string_into_c_str(
                url.as_ref().to_string(),
            ));
            let on_message_listeners = OnMessageListeners::new();
            ON_MESSAGE_FUNCTION.unwrap()(
                handle,
                StringCallback::callback({
                    let on_message_listeners = on_message_listeners.clone();
                    move |msg| {
                        log::error!("Message: {}", msg);
                        on_message_listeners.notify_all(msg);
                    }
                })
                .get(),
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
        self.on_message_listeners.new_subscriber()
    }

    fn set_close_reason(&self, reason: ClientDisconnect) {
        self.close_reason.set(reason);
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>> {
        let msg = serde_json::to_string(msg).unwrap();
        unsafe {
            SEND_FUNCTION.unwrap()(self.handle.get(), string_into_c_str(msg));
        }
        Ok(())
    }

    fn on_state_change(&self) -> LocalBoxStream<'static, TransportState> {
        Box::pin(futures::stream::once(async { TransportState::Open }))
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
                string_into_c_str(rsn),
            );
        }
    }
}
