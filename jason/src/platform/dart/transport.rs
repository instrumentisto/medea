use std::cell::{Cell, RefCell};

use dart_sys::Dart_Handle;
use futures::prelude::stream::LocalBoxStream;
use medea_client_api_proto::{ClientMsg, ServerMsg};
use tracerr::Traced;

use crate::{
    platform::{RpcTransport, TransportError, TransportState},
    rpc::{ApiUrl, ClientDisconnect},
    utils::dart::into_dart_string,
};

type Result<T, E = Traced<TransportError>> = std::result::Result<T, E>;

type NewFunction = extern "C" fn(*const libc::c_char) -> Dart_Handle;
static mut NEW_FUNCTION: Option<NewFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

type SendFunction = extern "C" fn(Dart_Handle, *const libc::c_char);
static mut SEND_FUNCTION: Option<SendFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__send(f: SendFunction) {
    SEND_FUNCTION = Some(f);
}

type CloseFunction = extern "C" fn(Dart_Handle, i32, *const libc::c_char);
static mut CLOSE_FUNCTION: Option<CloseFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketRpcTransport__close(
    f: CloseFunction,
) {
    CLOSE_FUNCTION = Some(f);
}

#[derive(Clone, Debug)]
pub struct WebSocketRpcTransport {
    handle: Dart_Handle,
    close_reason: Cell<ClientDisconnect>,
}

impl WebSocketRpcTransport {
    pub async fn new(url: ApiUrl) -> Result<Self> {
        unsafe {
            let handle = NEW_FUNCTION.unwrap()(into_dart_string(
                url.as_ref().to_string(),
            ));
            Ok(Self {
                handle,
                close_reason: Cell::new(
                    ClientDisconnect::RpcTransportUnexpectedlyDropped,
                ),
            })
        }
    }
}

impl RpcTransport for WebSocketRpcTransport {
    fn on_message(&self) -> LocalBoxStream<'static, ServerMsg> {
        todo!()
    }

    fn set_close_reason(&self, reason: ClientDisconnect) {
        self.close_reason.set(reason);
    }

    fn send(&self, msg: &ClientMsg) -> Result<(), Traced<TransportError>> {
        let msg = serde_json::to_string(msg).unwrap();
        unsafe {
            SEND_FUNCTION.unwrap()(self.handle, into_dart_string(msg));
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
            CLOSE_FUNCTION.unwrap()(self.handle, 1000, into_dart_string(rsn));
        }
    }
}
