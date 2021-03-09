//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::{cell::RefCell, marker::PhantomData};

use wasm_bindgen::JsValue;

/// Wrapper for a single argument JS function.
pub struct Callback<A>(RefCell<Option<Function<A>>>);

impl<A> Callback<A> {
    /// Sets an inner JS function.
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
    /// Invokes its JS function (if any) passing no arguments to it.
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

impl<A: Into<wasm_bindgen::JsValue>> Callback<A> {
    /// Invokes JS function (if any) passing the single provided `arg`ument to
    /// it.
    #[inline]
    pub fn call1<T: Into<A>>(&self, arg: T) {
        if let Some(f) = self.0.borrow().as_ref() {
            f.call1(arg.into())
        };
    }
}

/// Typed wrapper for a [`js_sys::Function`] with a single argument and no
/// result.
pub struct Function<T> {
    /// [`js_sys::Function`] itself.
    inner: js_sys::Function,

    /// Type of the function argument.
    _arg: PhantomData<T>,
}

impl Function<()> {
    /// Invokes a JS function passing no arguments to it.
    #[inline]
    pub fn call0(&self) {
        drop(self.inner.call0(&JsValue::NULL));
    }
}

impl<T: Into<JsValue>> Function<T> {
    /// Invokes a JS function passing the provided single `arg`ument to it.
    #[inline]
    pub fn call1(&self, arg: T) {
        drop(self.inner.call1(&JsValue::NULL, &arg.into()));
    }
}

impl<T> From<js_sys::Function> for Function<T> {
    #[inline]
    fn from(func: js_sys::Function) -> Self {
        Self {
            inner: func,
            _arg: PhantomData,
        }
    }
}
