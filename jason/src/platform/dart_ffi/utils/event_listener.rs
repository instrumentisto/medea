use std::{marker::PhantomData, rc::Rc};

use derive_more::{Display, From};
use tracerr::Traced;

use crate::{platform, utils::JsCaused};

/// Failed to bind to specified event.
#[derive(Clone, Debug, Display, From, JsCaused, PartialEq)]
#[js(error = "platform::Error")]
pub struct EventListenerBindError(platform::Error);

/// Wrapper for the closure that handles some event.
#[derive(Debug)]
pub struct EventListener<T, A> {
    t: PhantomData<T>,
    a: PhantomData<A>,
}

impl<T, A> EventListener<T, A> {
    /// Creates a new [`EventListener`] from the given [`FnMut`] `closure`.
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

    /// Creates a new [`EventListener`] from the given [`FnOnce`] `closure`.
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
