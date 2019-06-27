use std::{ops::Deref, rc::Rc};

use wasm_bindgen::{closure::Closure, convert::FromWasmAbi, JsCast};
use web_sys::EventTarget;

use crate::utils::WasmErr;

/// Wrapper for closure that handles some
/// [`EventTarget`](https://developer.mozilla.org/ru/docs/Web/API/EventTarget)
/// event. Implement drop that drops provided closure and unregisters
/// event handler.
pub struct EventListener<T, A>
where
    T: Deref<Target = EventTarget>,
{
    event_name: &'static str,
    target: Rc<T>,
    closure: Closure<dyn FnMut(A)>,
}

impl<T: Deref<Target = EventTarget>, A: FromWasmAbi + 'static>
    EventListener<T, A>
{
    pub fn new_mut<F>(
        target: Rc<T>,
        event_name: &'static str,
        closure: F,
    ) -> Result<Self, WasmErr>
    where
        F: FnMut(A) + 'static,
    {
        let closure = Closure::wrap(Box::new(closure) as Box<dyn FnMut(A)>);

        target.add_event_listener_with_callback(
            event_name,
            closure.as_ref().unchecked_ref(),
        )?;

        Ok(Self {
            event_name,
            target,
            closure,
        })
    }

    pub fn new_once<F>(
        target: Rc<T>,
        event_name: &'static str,
        closure: F,
    ) -> Result<Self, WasmErr>
    where
        F: FnOnce(A) + 'static,
    {
        let closure: Closure<dyn FnMut(A)> = Closure::once(closure);

        target.add_event_listener_with_callback(
            event_name,
            closure.as_ref().unchecked_ref(),
        )?;

        Ok(Self {
            event_name,
            target,
            closure,
        })
    }
}

impl<T: Deref<Target = EventTarget>, A> Drop for EventListener<T, A> {
    fn drop(&mut self) {
        if let Err(err) = (self.target.as_ref() as &web_sys::EventTarget)
            .remove_event_listener_with_callback(
                self.event_name,
                self.closure.as_ref().unchecked_ref(),
            )
        {
            WasmErr::from(err).log_err();
        }
    }
}
