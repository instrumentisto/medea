use crate::utils::into_dart_string;
use dart_sys::Dart_Handle;

#[no_mangle]
pub unsafe extern "C" fn websocket_test() {
    let ws = NEW_FUNCTION.unwrap()(into_dart_string(
        "wss://echo.websocket.org".to_string(),
    ));
    let listener = WsMessageListener::new(Box::new(
        (|msg| {
            assert_eq!(msg, "foobar");
            super::print("Message received".to_string());
        }),
    ));
    ON_MESSAGE_FUNCTION.unwrap()(ws, Box::into_raw(Box::new(listener)));
    SEND_FUNCTION.unwrap()(ws, into_dart_string("foobar".to_string()));
}

pub struct WsMessageListener {
    callback: Box<dyn Fn(String)>,
}

impl WsMessageListener {
    pub fn new(callback: Box<dyn Fn(String)>) -> Self {
        Self { callback }
    }

    pub fn call(&self, msg: String) {
        (self.callback)(msg);
    }
}

#[no_mangle]
pub unsafe extern "C" fn WsMessageListener__call(
    listener: *mut WsMessageListener,
    msg: *const libc::c_char,
) {
    let listener = Box::from_raw(listener);
    listener.call(crate::utils::from_dart_string(msg));
}

type NewFunction = extern "C" fn(addr: *const libc::c_char) -> Dart_Handle;

static mut NEW_FUNCTION: Option<NewFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketChannel__new(f: NewFunction) {
    NEW_FUNCTION = Some(f);
}

type OnMessageFunction =
    extern "C" fn(ws: Dart_Handle, listener: *mut WsMessageListener);

static mut ON_MESSAGE_FUNCTION: Option<OnMessageFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketChannel__on_message(
    f: OnMessageFunction,
) {
    ON_MESSAGE_FUNCTION = Some(f);
}

type SendFunction = extern "C" fn(ws: Dart_Handle, msg: *const libc::c_char);

static mut SEND_FUNCTION: Option<SendFunction> = None;

#[no_mangle]
pub unsafe extern "C" fn register_WebSocketChannel__send(f: SendFunction) {
    SEND_FUNCTION = Some(f);
}
