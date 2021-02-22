//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::{cell::RefCell, marker::PhantomData};

use js_sys::Function as JsFunction;

/// Wrapper for JS function with no arguments.
#[derive(Default)]
pub struct Callback0 {
    f: RefCell<Option<JsFunction>>,
}

impl Callback0 {
    /// Sets inner JS function.
    #[inline]
    pub fn set_func(&self, f: JsFunction) {
        self.f.borrow_mut().replace(f);
    }

    /// Invokes JS function if any.
    ///
    /// Returns `None` if no callback is set, otherwise returns its invocation
    /// result.
    pub fn call(&self) -> Option<Result<JsValue, JsValue>> {
        self.f.borrow().as_ref().map(|f| f.call0(&JsValue::NULL))
    }

    /// Indicates whether callback is set.
    #[inline]
    pub fn is_set(&self) -> bool {
        self.f.borrow().as_ref().is_some()
    }
}

/// Wrapper for a single argument JS function.
pub struct Callback1<A> {
    f: RefCell<Option<JsFunction>>,
    _arg: PhantomData<A>,
}

impl<A> Default for Callback1<A> {
    #[inline]
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _arg: PhantomData,
        }
    }
}

impl<A> Callback1<A> {
    /// Sets inner JS function.
    #[inline]
    pub fn set_func(&self, f: JsFunction) {
        self.f.borrow_mut().replace(f);
    }

    /// Indicates if callback is set.
    #[inline]
    pub fn is_set(&self) -> bool {
        self.f.borrow().as_ref().is_some()
    }
}

impl<A: Into<JsValue>> Callback1<A> {
    /// Invokes JS function if any.
    ///
    /// Returns `None` if no callback is set, otherwise returns its invocation
    /// result.
    pub fn call<T: Into<A>>(&self, arg: T) -> Option<Result<JsValue, JsValue>> {
        self.f
            .borrow()
            .as_ref()
            .map(|f| f.call1(&JsValue::NULL, &arg.into().into()))
    }
}

impl<A> From<JsFunction> for Callback1<A> {
    #[inline]
    fn from(f: JsFunction) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _arg: PhantomData,
        }
    }
}
