//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;
mod log_sender;

use js_sys::Reflect;
use medea_client_api_proto::PeerId;
use wasm_bindgen::prelude::*;
use web_sys::Window;

#[cfg(feature = "mockable")]
#[doc(inline)]
pub use self::log_sender::MockHTTPClient;
#[doc(inline)]
pub use self::{
    callback::{Callback, Callback2},
    errors::{HandlerDetachedError, JasonError, JsCaused, JsError},
    event_listener::{EventListener, EventListenerBindError},
    log_sender::{
        FetchHTTPClient, HTTPClient, HTTPClientError, LogSender, JASON_LOG_KEY,
    },
};

/// Returns [`Window`] object.
///
/// # Panics
///
/// When global [`Window`] object is inaccessible.
pub fn window() -> Window {
    // Cannot use `lazy_static` since `window` is `!Sync`.
    // Safe to unwrap.
    web_sys::window().unwrap()
}

macro_rules! store_jason_error {
    ($peer:expr) => {
        |e| {
            $crate::utils::store_error(&e, $peer);
            e
        }
    };
    ($e:expr, $peer:expr) => {
        $crate::utils::store_error($e, $peer);
    };
}

/// Stores [`JasonError`] into local storage for further sending to the server.
pub fn store_error(err: &JasonError, peer_id: Option<PeerId>) {
    let value = match peer_id {
        Some(id) => format!("\"peer_id={} error={}\"", id, err),
        None => format!("\"error={}\"", err),
    };
    LogSender::push_to_store(value.replace("\n", "\\n").into());
}

/// Wrapper around interval timer ID.
pub struct IntervalHandle(pub i32);

impl Drop for IntervalHandle {
    /// Clears interval with provided ID.
    fn drop(&mut self) {
        window().clear_interval_with_handle(self.0);
    }
}

/// Upgrades newtyped [`Weak`] reference, returning [`HandlerDetachedError`] if
/// failed, or maps the [`Rc`]-referenced value with provided `$closure`
/// otherwise.
///
/// [`Rc`]: std::rc::Rc
/// [`Weak`]: std::rc::Weak
macro_rules! map_weak {
    ($v:ident, $closure:expr) => {{
        $v.0.upgrade()
            .ok_or(
                $crate::utils::JasonError::from(tracerr::new!(
                    $crate::utils::HandlerDetachedError
                ))
                .into(),
            )
            .map($closure)
    }};
}

/// Returns property of JS object by name if its defined.
/// Converts the value with a given predicate.
pub fn get_property_by_name<T, F, U>(
    value: &T,
    name: &str,
    into: F,
) -> Option<U>
where
    T: AsRef<wasm_bindgen::JsValue>,
    F: Fn(wasm_bindgen::JsValue) -> Option<U>,
{
    Reflect::get(value.as_ref(), &JsValue::from_str(name))
        .ok()
        .map_or_else(|| None, into)
}
