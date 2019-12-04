use std::{cell::RefCell, rc::Rc};

use derive_more as dm;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestInit, RequestMode, Response};

use super::{window, IntervalHandle, JasonError, JsCaused, JsError};

/// Key in a local storage for storing logs.
const JASON_LOG_KEY: &str = "jason_log";

/// The key in local storage for logs that could not be sent on the first try.
const SEND_LOG_KEY: &str = "jason_send";

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
        let inner = Rc::new(RefCell::new(Inner {
            url: Rc::new(url.to_string()),
            logs_task: None,
        }));
        let inner_rc = Rc::clone(&inner);
        let do_send = Closure::wrap(Box::new(move || {
            inner_rc.borrow().send_now();
        }) as Box<dyn Fn()>);

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
    }

    /// Stores given string value in local storage.
    pub fn push_to_store(value: &str) {
        if let Ok(Some(store)) = window().local_storage() {
            let mut log = store
                .get("jason_log")
                .unwrap()
                .map_or("[".to_string(), |s: String| {
                    format!("{},", s.trim_end_matches(']'))
                });
            log = format!("{}{}]", log, value);
            store.set("jason_log", &log).unwrap();
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
                if let Some(store) = window()
                    .local_storage()
                    .map_err(JsError::from)
                    .map_err(Error::NotAccessLocalStorage)
                    .map_err(tracerr::wrap!())?
                {
                    if let Ok(Some(logs)) = store.get(SEND_LOG_KEY) {
                        send(&url, &logs)
                            .await
                            .map(|_| store.delete(SEND_LOG_KEY).unwrap())
                            .map_err(tracerr::wrap!())?;
                    }
                    if let Ok(Some(logs)) = store.get(JASON_LOG_KEY) {
                        store.delete(JASON_LOG_KEY).unwrap();
                        send(&url, &logs)
                            .await
                            .map_err(|e| {
                                store.set(SEND_LOG_KEY, &logs).unwrap();
                                e
                            })
                            .map_err(tracerr::wrap!())?;
                    }
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

/// Sends given body as a `text/plain` POST request to the specified URL.
pub async fn send(url: &str, body: &str) -> Result<(), Traced<Error>> {
    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    let js_value = JsValue::from_str(body);
    opts.body(Some(&js_value));

    let request = Request::new_with_str_and_init(url, &opts).unwrap();

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
}
