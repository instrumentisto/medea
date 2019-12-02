use std::rc::Rc;

use derive_more as dm;
use tracerr::Traced;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Request, RequestInit, RequestMode, Response};

use super::{window, IntervalHandle, JasonError, JsCaused, JsError};
use wasm_bindgen::__rt::core::cell::RefCell;

/// Errors that may occur in [`LogSender`].
#[derive(Debug, dm::Display, JsCaused)]
pub enum Error {
    /// Occurs when a local storage unavailable.
    #[display(fmt = "local storage unavailable: {}", _0)]
    NotAccessLocalStorage(JsError),

    /// Occurs when a handler cannot be set to send `logs`.
    #[display(fmt = "cannot set callback for logs send: {}", _0)]
    SetIntervalHandler(JsError),

    #[display(fmt = "cannot send logs")]
    CannotSendLogs(JsError),

    #[display(
        fmt = "response error: [code = {}, message = {}]",
        code,
        message
    )]
    ServerFailed { code: u16, message: String },
}

struct Inner {
    url: String,
    /// Handler of sending `logs` task. Task is dropped if you drop handler.
    logs_task: Option<LogsTaskHandler>,
}

pub struct LogSender(Rc<RefCell<Inner>>);

impl LogSender {
    /// Returns new instance of [`LogSender`] with given interval in
    /// milliseconds for send logs to specified url.
    pub fn new(url: &str, interval: i32) -> Result<Self, Traced<Error>> {
        let inner = Rc::new(RefCell::new(Inner {
            url: url.to_string(),
            logs_task: None,
        }));
        let inner_rc = Rc::clone(&inner);
        let do_send = Closure::wrap(Box::new(move || {
            inner_rc.borrow().send_now();
        }) as Box<dyn FnMut()>);

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
    _closure: Closure<dyn FnMut()>,
    _interval_handler: IntervalHandle,
}

impl Inner {
    fn send_now(&self) {
        let url = self.url.clone();
        spawn_local(async move {
            if let Err(e) = async move {
                if let Some(store) = window()
                    .local_storage()
                    .map_err(JsError::from)
                    .map_err(Error::NotAccessLocalStorage)
                    .map_err(tracerr::wrap!())?
                {
                    if let Ok(Some(logs)) = store.get("send_log") {
                        send(&url, &logs)
                            .await
                            .map(|_| store.delete("send_log").unwrap())
                            .map_err(tracerr::wrap!())?;
                    }
                    if let Ok(Some(logs)) = store.get("jason_log") {
                        store.delete("jason_log").unwrap();
                        let json = format!("{{\"errors\":{}}}", logs);
                        send(&url, &json)
                            .await
                            .map_err(|e| {
                                store.set("send_log", &json).unwrap();
                                e
                            })
                            .map_err(tracerr::wrap!())?;
                    }
                }
                Ok::<_, Traced<Error>>(())
            }
            .await
            {
                JasonError::from(e).print();
            }
        })
    }
}

pub async fn send(url: &str, body: &str) -> Result<(), Traced<Error>> {
    let mut opts = RequestInit::new();
    opts.method("POST");
    opts.mode(RequestMode::Cors);
    let js_value = JsValue::from_str(body);
    opts.body(Some(&js_value));

    let request = Request::new_with_str_and_init(url, &opts).unwrap();

    request
        .headers()
        .set("Content-Type", "application/json")
        .unwrap();

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
