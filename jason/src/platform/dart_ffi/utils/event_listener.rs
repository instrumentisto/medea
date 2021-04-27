use std::{marker::PhantomData, ops::Deref, rc::Rc};

use derive_more::{Display, From};
use tracerr::Traced;

use crate::{platform, utils::JsCaused};

/// Failed to bind to [`EventTarget`][1] event.
///
/// [1]: https://developer.mozilla.org/en-US/docs/Web/API/EventTarget
#[derive(Clone, Debug, Display, From, JsCaused, PartialEq)]
#[js(error = "platform::Error")]
pub struct EventListenerBindError(platform::Error);

/// Wrapper for closure that handles some [`EventTarget`] event.
///
/// [`EventTarget`]: web_sys::EventTarget
#[derive(Debug)]
pub struct EventListener<T, A>
where
    T: Deref<Target = web_sys::EventTarget>,
{
    t: PhantomData<T>,
    a: PhantomData<A>,
}

impl<T, A> EventListener<T, A>
where
    T: Deref<Target = web_sys::EventTarget>,
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
        unimplemented!()
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
        unimplemented!()
    }
}

impl<T, A> Drop for EventListener<T, A>
where
    T: Deref<Target = web_sys::EventTarget>,
{
    /// Drops [`EventListener`]'s closure and unregisters appropriate event
    /// handler.
    fn drop(&mut self) {
        unimplemented!()
    }
}
