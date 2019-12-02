//! Miscellaneous utility structs and functions.

#[macro_use]
mod errors;

mod callback;
mod event_listener;
mod logs_sender;

use js_sys::Reflect;
use medea_client_api_proto::PeerId;
use wasm_bindgen::prelude::*;
use web_sys::Window;

#[doc(inline)]
pub use self::{
    callback::{Callback, Callback2},
    errors::{HandlerDetachedError, JasonError, JsCaused, JsError},
    event_listener::{EventListener, EventListenerBindError},
    logs_sender::LogSender,
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

pub fn store_error(err: JasonError, peer_id: Option<PeerId>) -> JasonError {
    match peer_id {
        Some(id) => push_to_store(
            &format!("{{\"peer_id\":{},\"error\":\"{}\"}}", id, err),
            true,
        ),
        None => push_to_store(&format!("{{\"error\":\"{}\"}}", err), true),
    }
    err
}

pub fn push_to_store(value: &str, as_json: bool) {
    if let Ok(Some(store)) = window().local_storage() {
        let mut log = store
            .get("jason_log")
            .unwrap()
            .map_or("[".to_string(), |s: String| {
                format!("{},", s.trim_end_matches(']'))
            });
        if as_json {
            log = format!("{}{}]", log, value);
        } else {
            log = format!("{}\"{}\"]", log, value);
        }
        store.set("jason_log", &log).unwrap();
    }
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
