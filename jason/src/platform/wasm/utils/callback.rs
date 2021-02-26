//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::{cell::RefCell, marker::PhantomData};
use wasm_bindgen::JsValue;

/// Wrapper for a single argument JS function.
pub struct Callback<A>(RefCell<Option<Function<A>>>);

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
    /// Invokes JS function (if any).
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
    /// Invokes JS function (if any) with provided argument.
    pub fn call1<T: Into<A>>(&self, arg: T) {
        if let Some(f) = self.0.borrow().as_ref() {
            f.call1(arg.into())
        };
    }
}

/// Typed wrapper for [`js_sys::Function`] with single argument and no result.
pub struct Function<T> {
    inner: js_sys::Function,
    _arg: PhantomData<T>,
}

impl Function<()> {
    /// Invokes JS function.
    pub fn call0(&self) {
        std::mem::drop(self.inner.call0(&JsValue::NULL));
    }
}

impl<T: Into<JsValue>> Function<T> {
    /// Invokes JS function with provided argument.
    pub fn call1(&self, arg: T) {
        std::mem::drop(self.inner.call1(&JsValue::NULL, &arg.into()));
    }
}

impl<T> From<js_sys::Function> for Function<T> {
    fn from(func: js_sys::Function) -> Self {
        Self {
            inner: func,
            _arg: PhantomData::default(),
        }
    }
}
