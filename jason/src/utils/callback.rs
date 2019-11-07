//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::{cell::RefCell, marker::PhantomData};

use js_sys::Function as JsFunction;
use wasm_bindgen::JsValue;

/// Wrapper for a single argument JS function.
pub struct Callback<A> {
    f: RefCell<Option<JsFunction>>,
    _arg: PhantomData<A>,
}

impl<A> Default for Callback<A> {
    #[inline]
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _arg: PhantomData,
        }
    }
}

impl<A: Into<JsValue>> Callback<A> {
    /// Sets inner JS function.
    #[inline]
    pub fn set_func(&self, f: JsFunction) {
        self.f.borrow_mut().replace(f);
    }

    /// Invokes JS function if any.
    ///
    /// Returns `None` if no callback is set, otherwise returns its invocation
    /// result.
    pub fn call(&self, arg: A) -> Option<Result<JsValue, JsValue>> {
        self.f
            .borrow()
            .as_ref()
            .map(|f| f.call1(&JsValue::NULL, &arg.into()))
    }

    /// Indicates if callback is set.
    #[inline]
    pub fn is_set(&self) -> bool {
        self.f.borrow().as_ref().is_some()
    }
}

impl<A> From<JsFunction> for Callback<A> {
    #[inline]
    fn from(f: JsFunction) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _arg: PhantomData,
        }
    }
}

/// Wrapper for a JS functions with two arguments.
///
/// Can be used if you need to conditionally invoke function passing one of two
/// args, e.g. first arg in case of success, and second as error.
#[allow(clippy::module_name_repetitions)]
pub struct Callback2<A1, A2> {
    f: RefCell<Option<JsFunction>>,
    _arg1: PhantomData<A1>,
    _arg2: PhantomData<A2>,
}

impl<A1, A2> Default for Callback2<A1, A2> {
    #[inline]
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _arg1: PhantomData,
            _arg2: PhantomData,
        }
    }
}

impl<A1: Into<JsValue>, A2: Into<JsValue>> Callback2<A1, A2> {
    /// Sets inner JS function.
    #[inline]
    pub fn set_func(&self, f: JsFunction) {
        self.f.borrow_mut().replace(f);
    }

    /// Invokes JS function passing both arguments.
    ///
    /// Returns `None` if no callback is set, otherwise returns its invocation
    /// result.
    pub fn call(
        &self,
        arg1: Option<A1>,
        arg2: Option<A2>,
    ) -> Option<Result<JsValue, JsValue>> {
        self.f.borrow().as_ref().map(|f| {
            f.call2(
                &JsValue::NULL,
                &arg1.map_or(JsValue::NULL, Into::into),
                &arg2.map_or(JsValue::NULL, Into::into),
            )
        })
    }

    /// Invokes JS function passing only first argument.
    #[inline]
    pub fn call1(&self, arg1: A1) -> Option<Result<JsValue, JsValue>> {
        self.call(Some(arg1), None)
    }

    /// Invokes JS function passing only second argument.
    #[inline]
    pub fn call2(&self, arg2: A2) -> Option<Result<JsValue, JsValue>> {
        self.call(None, Some(arg2))
    }
}

impl<A1, A2> From<JsFunction> for Callback2<A1, A2> {
    #[inline]
    fn from(f: JsFunction) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _arg1: PhantomData,
            _arg2: PhantomData,
        }
    }
}
