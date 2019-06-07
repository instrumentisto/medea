//! Somewhat convenient wrappers around JS functions used as callbacks.

use std::{cell::RefCell, marker::PhantomData};

use wasm_bindgen::JsValue;

/// Wrapper for single arg JS functions.
pub struct Callback<A: Into<JsValue>> {
    f: RefCell<Option<js_sys::Function>>,
    _phantom: PhantomData<A>,
}

impl<A: Into<JsValue>> Default for Callback<A> {
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _phantom: PhantomData,
        }
    }
}

impl<A: Into<JsValue>> Callback<A> {

    /// Sets inner JS function.
    pub fn set_func(&self, f: js_sys::Function) {
        self.f.borrow_mut().replace(f);
    }

    /// Invokes JS function if any. Returns `true` if function is set and was invoked, false otherwise.
    pub fn call(&self, arg: A) -> bool {
        match self.f.borrow().as_ref() {
            None => false,
            Some(f) => {
                let _ = f.call1(&JsValue::NULL, &arg.into());

                true
            }
        }
    }
}

impl<A: Into<JsValue>> From<js_sys::Function> for Callback<A> {
    fn from(f: js_sys::Function) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _phantom: PhantomData,
        }
    }
}

/// Wrapper for JS functions with two args. Can be used if you need to conditionally invoke function passing one of two args, e.g. first arg in case of success, and second as error.
#[allow(clippy::module_name_repetitions)]
pub struct Callback2<A: Into<JsValue>, B: Into<JsValue>> {
    f: RefCell<Option<js_sys::Function>>,
    _phantom: PhantomData<A>,
    _phantom2: PhantomData<B>,
}

impl<A: Into<JsValue>, B: Into<JsValue>> Default for Callback2<A, B> {
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}

impl<A: Into<JsValue>, B: Into<JsValue>> Callback2<A, B> {

    /// Sets inner JS function.
    pub fn set_func(&self, f: js_sys::Function) {
        self.f.borrow_mut().replace(f);
    }

    /// Call JS function passing both args.
    pub fn call(&self, arg1: Option<A>, arg2: Option<B>) -> bool {

        fn arg_to_jsvalue<A: Into<JsValue>>(arg: Option<A>) -> JsValue {
            match arg {
                None => JsValue::NULL,
                Some(inner) => inner.into(),
            }
        }

        match self.f.borrow().as_ref() {
            None => false,
            Some(f) => {
                let _ = f.call2(
                    &JsValue::NULL,
                    &arg_to_jsvalue(arg1),
                    &arg_to_jsvalue(arg2),
                );

                true
            }
        }
    }

    /// Call JS function passing only first arg.
    pub fn call1(&self, arg1: A) -> bool {
        self.call(Some(arg1), None)
    }

    /// Call JS function passing only second arg.
    pub fn call2(&self, arg2: B) -> bool {
        self.call(None, Some(arg2))
    }
}

impl<A: Into<JsValue>, B: Into<JsValue>> From<js_sys::Function>
    for Callback2<A, B>
{
    fn from(f: js_sys::Function) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}
