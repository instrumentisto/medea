//! Functionality for calling platform callbacks.

use std::cell::RefCell;

use super::Function;

/// Wrapper for a single argument callback function.
pub struct Callback<A>(pub RefCell<Option<Function<A>>>);

impl<A> Callback<A> {
    /// Sets an inner [`Function`].
    #[inline]
    pub fn set_func(&self, f: Function<A>) {
        self.0.borrow_mut().replace(f);
    }

    /// Indicates whether this [`Callback`] is set.
    #[inline]
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.0.borrow().as_ref().is_some()
    }
}

impl Callback<()> {
    /// Invokes underlying [`Function`] (if any) passing no arguments to it.
    #[inline]
    pub fn call0(&self) {
        if let Some(f) = self.0.borrow().as_ref() {
            f.call0()
        };
    }
}

impl<A> Default for Callback<A> {
    #[inline]
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}
