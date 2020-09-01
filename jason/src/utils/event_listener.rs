use std::{ops::Deref, rc::Rc};

use derive_more::{Display, From};
use tracerr::Traced;
use wasm_bindgen::{closure::Closure, convert::FromWasmAbi, JsCast};
use web_sys::EventTarget;

use super::{errors::JsCaused, JsError};

/// Failed to bind to [`EventTarget`][1] event.
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/EventTarget
#[derive(Clone, Debug, Display, From, JsCaused, PartialEq)]
pub struct EventListenerBindError(JsError);

/// Wrapper for closure that handles some [`EventTarget`] event.
pub struct EventListener<T, A>
where
    T: Deref<Target = EventTarget>,
{
    event_name: &'static str,
    target: Rc<T>,
    closure: Closure<dyn FnMut(A)>,
}

impl<T, A> EventListener<T, A>
where
    T: Deref<Target = EventTarget>,
    A: FromWasmAbi + 'static,
{
    /// Creates new [`EventListener`] from a given [`FnMut`] `closure`.
    ///
    /// # Errors
    ///
    /// Errors if [`EventListener`] bound fails.
    pub fn new_mut<F>(
        target: Rc<T>,
        event_name: &'static str,
        closure: F,
    ) -> Result<Self, Traced<EventListenerBindError>>
    where
        F: FnMut(A) + 'static,
    {
        let closure = Closure::wrap(Box::new(closure) as Box<dyn FnMut(A)>);

        target
            .add_event_listener_with_callback(
                event_name,
                closure.as_ref().unchecked_ref(),
            )
            .map_err(JsError::from)
            .map_err(EventListenerBindError::from)
            .map_err(tracerr::wrap!())?;

        Ok(Self {
            event_name,
            target,
            closure,
        })
    }

    /// Creates new [`EventListener`] from a given [`FnOnce`] `closure`.
    ///
    /// # Errors
    ///
    /// Errors if [`EventListener`] bound fails.
    pub fn new_once<F>(
        target: Rc<T>,
        event_name: &'static str,
        closure: F,
    ) -> Result<Self, Traced<EventListenerBindError>>
    where
        F: FnOnce(A) + 'static,
    {
        let closure: Closure<dyn FnMut(A)> = Closure::once(closure);

        target
            .add_event_listener_with_callback(
                event_name,
                closure.as_ref().unchecked_ref(),
            )
            .map_err(JsError::from)
            .map_err(EventListenerBindError::from)
            .map_err(tracerr::wrap!())?;

        Ok(Self {
            event_name,
            target,
            closure,
        })
    }
}

impl<T, A> Drop for EventListener<T, A>
where
    T: Deref<Target = EventTarget>,
{
    /// Drops [`EventListener`]'s closure and unregisters appropriate event
    /// handler.
    fn drop(&mut self) {
        if let Err(err) = (self.target.as_ref() as &web_sys::EventTarget)
            .remove_event_listener_with_callback(
                self.event_name,
                self.closure.as_ref().unchecked_ref(),
            )
        {
            log::error!("Failed to remove EventListener: {:?}", err);
        }
    }
}
