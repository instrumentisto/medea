//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::cell::RefCell;

use crate::platform::Function;

/// Wrapper for a single argument JS function.
pub struct Callback<A>(RefCell<Option<Function<A>>>);

impl<A> Default for Callback<A> {
    #[inline]
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

impl<A> Callback<A> {
    /// Sets inner JS function.
    #[inline]
    pub fn set_func(&self, f: Function<A>) {
        self.0.borrow_mut().replace(f);
    }

    /// Indicates if callback is set.
    #[inline]
    pub fn is_set(&self) -> bool {
        self.0.borrow().as_ref().is_some()
    }
}

impl Callback<()> {
    pub fn call0(&self) {
        self.0.borrow().as_ref().map(|f| f.call0());
    }
}

impl<A: Into<wasm_bindgen::JsValue>> Callback<A> {
    /// Invokes JS function if any.
    ///
    /// Returns `None` if no callback is set, otherwise returns its invocation
    /// result.
    pub fn call1<T: Into<A>>(&self, arg: T) {
        self.0.borrow().as_ref().map(|f| f.call1(arg.into()));
    }
}
