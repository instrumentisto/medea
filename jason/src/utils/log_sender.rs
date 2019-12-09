use std::{cell::RefCell, rc::Rc};

use derive_more as dm;
use futures::future::LocalBoxFuture;
use js_sys::JsString;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestInit, RequestMode, Response};

use super::{window, IntervalHandle, JasonError, JsCaused, JsError};

/// Key in a local storage for storing logs.
pub const JASON_LOG_KEY: &str = "jason_log";

#[wasm_bindgen(inline_js = "export const local_storage_get = (key) => { let \
                            value = window.localStorage.getItem(key); return \
                            value;}")]
extern "C" {
    fn local_storage_get(key: &str) -> Option<JsString>;
}

#[wasm_bindgen(inline_js = "export const local_storage_set = (key, value) => \
                            { window.localStorage.setItem(key, value); }")]
extern "C" {
    #[allow(clippy::needless_pass_by_value)]
    fn local_storage_set(key: &str, value: JsString);
}

#[wasm_bindgen(inline_js = "export const local_storage_remove = (key) => { \
                            window.localStorage.removeItem(key); }")]
extern "C" {
    fn local_storage_remove(key: &str);
}

/// Errors that may occur in [`LogSender`].
#[derive(Debug, dm::Display, JsCaused)]
pub enum Error {
    /// Occurs when a local storage unavailable.
    #[display(fmt = "local storage unavailable: {}", _0)]
    NotAccessLocalStorage(JsError),

    /// Occurs when a recurring task for sending logs cannot be created.
    #[display(fmt = "cannot set callback for logs send: {}", _0)]
    SetIntervalHandler(JsError),

    /// Occurs when the log sending to the server fails.
    #[display(fmt = "cannot send logs")]
    SendLogsFailed(#[js(caused)] HTTPClientError),
}

impl From<HTTPClientError> for Error {
    fn from(err: HTTPClientError) -> Self {
        Self::SendLogsFailed(err)
    }
}

struct Inner {
    /// HTTP client for sending logs to remote server .
    transport: Rc<dyn HTTPClient<Body = JsString>>,
    /// Handler of sending `logs` task. Task is dropped if you drop handler.
    logs_task: Option<LogsTaskHandler>,
}

/// Responsible for storing logs in local storage and sending it
/// to the server.
pub struct LogSender(Rc<RefCell<Inner>>);

impl LogSender {
    /// Returns new instance of [`LogSender`] with given interval in
    /// milliseconds for send logs by specified transport.
    pub fn new(
        transport: Rc<dyn HTTPClient<Body = JsString>>,
        interval: i32,
    ) -> Result<Self, Traced<Error>> {
        window()
            .local_storage()
            .map_err(JsError::from)
            .map_err(Error::NotAccessLocalStorage)
            .map_err(tracerr::wrap!())
            .and_then(|_| {
                let inner = Rc::new(RefCell::new(Inner {
                    transport,
                    logs_task: None,
                }));
                let inner_rc = Rc::clone(&inner);
                let do_send = Closure::wrap(Box::new(move || {
                    inner_rc.borrow().send_now();
                })
                    as Box<dyn Fn()>);

                let interval_id = window()
                    .set_interval_with_callback_and_timeout_and_arguments_0(
                        do_send.as_ref().unchecked_ref(),
                        interval,
                    )
                    .map_err(JsError::from)
                    .map_err(Error::SetIntervalHandler)
                    .map_err(tracerr::wrap!())?;

                inner.borrow_mut().logs_task = Some(LogsTaskHandler {
                    _closure: do_send,
                    _interval_handler: IntervalHandle(interval_id),
                });

                Ok(Self(inner))
            })
    }

    /// Appends string to value stored in local storage with specified key.
    pub fn push_to_store(value: JsString) {
        if let Ok(Some(_)) = window().local_storage() {
            let logs = if let Some(log) = local_storage_get(JASON_LOG_KEY) {
                log.concat(&JsValue::from_str(",")).concat(&value.into())
            } else {
                value
            };
            local_storage_set(JASON_LOG_KEY, logs);
        }
    }
}

impl Drop for LogSender {
    /// Stops [`LogSender`] task.
    fn drop(&mut self) {
        self.0.borrow_mut().logs_task.take();
    }
}

/// Handler for binding closure that repeatedly calls a closure with a fixed
/// time delay between each call.
struct LogsTaskHandler {
    _closure: Closure<dyn Fn()>,
    _interval_handler: IntervalHandle,
}

impl Inner {
    /// Runs async task for send logs from local storage to server.
    fn send_now(&self) {
        let client = Rc::clone(&self.transport);
        spawn_local(async move {
            if let Err(e) = async move {
                if let Some(logs) = local_storage_get(JASON_LOG_KEY) {
                    local_storage_remove(JASON_LOG_KEY);
                    let body = JsString::from("[")
                        .concat(&logs.clone().into())
                        .concat(&JsValue::from_str("]"));
                    client
                        .send(body)
                        .await
                        .map_err(|e| {
                            LogSender::push_to_store(logs);
                            e
                        })
                        .map_err(tracerr::map_from_and_wrap!())?;
                }
                Ok::<_, Traced<Error>>(())
            }
            .await
            .map_err(JasonError::from)
            {
                e.print();
            }
        })
    }
}

/// Errors that may occur in [`HTT`].
#[derive(Debug, dm::Display, JsCaused)]
pub enum HTTPClientError {
    /// Occurs when a request cannot be sent to the server.
    #[display(fmt = "cannot send request")]
    InvalidRequest(JsError),

    /// Occurs when a remote server returns an error.
    #[display(
        fmt = "response error: [code = {}, message = {}]",
        code,
        message
    )]
    ResponseFailed { code: u16, message: String },
}

/// HTTP client that uses the browser fetch API to send requests.
#[cfg_attr(feature = "mockable", mockall::automock(type Body=JsString;))]
pub trait HTTPClient {
    /// Type of request body.
    type Body: AsRef<JsValue>;

    /// Sends given body to remote server.
    fn send(
        &self,
        body: Self::Body,
    ) -> LocalBoxFuture<'static, Result<(), Traced<HTTPClientError>>>;
}

/// HTTP client that uses the browser fetch API to send requests.
pub struct FetchHTTPClient {
    /// Endpoint of remote server for sending request.
    url: String,
}

impl FetchHTTPClient {
    /// Returns new instance of [`FetchHTTPClient`] with specified url of remote
    /// server.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
        }
    }
}

impl HTTPClient for FetchHTTPClient {
    type Body = JsString;

    /// Sends given body as a `application/json` POST request to remote server.
    fn send(
        &self,
        body: Self::Body,
    ) -> LocalBoxFuture<'static, Result<(), Traced<HTTPClientError>>> {
        let mut opts = RequestInit::new();
        opts.method("POST");
        opts.mode(RequestMode::Cors);
        opts.body(Some(body.as_ref()));

        let request = Request::new_with_str_and_init(&self.url, &opts).unwrap();
        request
            .headers()
            .set("Content-Type", "application/json")
            .unwrap();

        Box::pin(async move {
            JsFuture::from(window().fetch_with_request(&request))
                .await
                .map_err(JsError::from)
                .map_err(HTTPClientError::InvalidRequest)
                .map_err(tracerr::wrap!())
                .and_then(|resp_value| {
                    assert!(resp_value.is_instance_of::<Response>());
                    let resp: Response = resp_value.dyn_into().unwrap();
                    if !resp.ok() {
                        return Err(tracerr::new!(
                            HTTPClientError::ResponseFailed {
                                code: resp.status(),
                                message: resp.status_text(),
                            }
                        ));
                    }
                    Ok(())
                })
        })
    }
}
