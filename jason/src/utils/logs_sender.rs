use std::{cell::RefCell, rc::Rc};

use derive_more as dm;
use futures::future::LocalBoxFuture;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestInit, RequestMode, Response};

use super::{window, IntervalHandle, JasonError, JsCaused, JsError};

/// Key in a local storage for storing logs.
const JASON_LOG_KEY: &str = "jason_log";

/// The key in local storage for body of request that could not be sent on the
/// first try.
const SEND_BODY_KEY: &str = "jason_send";

#[wasm_bindgen(inline_js = "export const local_storage_get = (key) => { let \
                            value = window.localStorage.getItem(key); return \
                            value;}")]
extern "C" {
    fn local_storage_get(key: &str) -> Option<js_sys::JsString>;
}

#[wasm_bindgen(inline_js = "export const local_storage_set = (key, value) => \
                            { window.localStorage.setItem(key, value); }")]
extern "C" {
    #[allow(clippy::needless_pass_by_value)]
    fn local_storage_set(key: &str, value: js_sys::JsString);
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

    /// Occurs when a log request cannot be sent to the server.
    #[display(fmt = "cannot send logs")]
    CannotSendLogs(JsError),

    /// Occurs when a remote server returns an error.
    #[display(
        fmt = "response error: [code = {}, message = {}]",
        code,
        message
    )]
    ServerFailed { code: u16, message: String },
}

struct Inner {
    /// Url of remote server for sending logs.
    url: Rc<String>,
    /// Handler of sending `logs` task. Task is dropped if you drop handler.
    logs_task: Option<LogsTaskHandler>,
}

/// Responsible for storing logs in local storage and sending it
/// to the server.
pub struct LogSender(Rc<RefCell<Inner>>);

impl LogSender {
    /// Returns new instance of [`LogSender`] with given interval in
    /// milliseconds for send logs to specified url.
    pub fn new(url: &str, interval: i32) -> Result<Self, Traced<Error>> {
        window()
            .local_storage()
            .map_err(JsError::from)
            .map_err(Error::NotAccessLocalStorage)
            .map_err(tracerr::wrap!())
            .and_then(|_| {
                let inner = Rc::new(RefCell::new(Inner {
                    url: Rc::new(url.to_string()),
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

    /// Stores given string value in local storage.
    pub fn push_to_store(value: &str) {
        if let Ok(Some(_)) = window().local_storage() {
            let pattern = js_sys::RegExp::new("\\n", "g");
            let str_value = js_sys::JsString::from("\"")
                .concat(&JsValue::from_str(value))
                .concat(&JsValue::from_str("\""))
                .replace_by_pattern(&pattern, "\\n");
            let logs = if let Some(log) = local_storage_get(JASON_LOG_KEY) {
                log.concat(&JsValue::from_str(","))
                    .concat(&str_value.into())
            } else {
                str_value
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
        let url = Rc::clone(&self.url);
        spawn_local(async move {
            if let Err(e) = async move {
                if let Some(body) = local_storage_get(SEND_BODY_KEY) {
                    FetchHttpClient::post(&url, body)
                        .await
                        .map(|_| local_storage_remove(SEND_BODY_KEY))
                        .map_err(tracerr::wrap!())?;
                }
                if let Some(logs) = local_storage_get(JASON_LOG_KEY) {
                    local_storage_remove(JASON_LOG_KEY);
                    let body = js_sys::JsString::from("[")
                        .concat(&logs.into())
                        .concat(&JsValue::from_str("]"));
                    FetchHttpClient::post(&url, body.clone())
                        .await
                        .map_err(|e| {
                            local_storage_set(SEND_BODY_KEY, body);
                            e
                        })
                        .map_err(tracerr::wrap!())?;
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

#[cfg_attr(feature = "mockable", mockall::automock)]
trait HttpClient {
    /// Sends given body as a `text/plain` POST request to the specified URL.
    fn post(
        url: &str,
        body: js_sys::JsString,
    ) -> LocalBoxFuture<'static, Result<(), Traced<Error>>>;
}

struct FetchHttpClient {}

impl HttpClient for FetchHttpClient {
    /// Sends given body as a `application/json` POST request
    /// to the specified URL.
    fn post(
        url: &str,
        body: js_sys::JsString,
    ) -> LocalBoxFuture<'static, Result<(), Traced<Error>>> {
        let mut opts = RequestInit::new();
        opts.method("POST");
        opts.mode(RequestMode::Cors);
        opts.body(Some(&body.into()));

        let request = Request::new_with_str_and_init(url, &opts).unwrap();
        request
            .headers()
            .set("Content-Type", "application/json")
            .unwrap();

        Box::pin(async move {
            JsFuture::from(window().fetch_with_request(&request))
                .await
                .map_err(JsError::from)
                .map_err(Error::CannotSendLogs)
                .map_err(tracerr::wrap!())
                .and_then(|resp_value| {
                    assert!(resp_value.is_instance_of::<Response>());
                    let resp: Response = resp_value.dyn_into().unwrap();
                    if !resp.ok() {
                        return Err(tracerr::new!(Error::ServerFailed {
                            code: resp.status(),
                            message: resp.status_text(),
                        }));
                    }
                    Ok(())
                })
        })
    }
}
