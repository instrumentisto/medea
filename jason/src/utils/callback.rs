use std::{cell::RefCell, marker::PhantomData};

use wasm_bindgen::JsValue;

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
    pub fn set_func(&self, f: js_sys::Function) {
        self.f.borrow_mut().replace(f);
    }

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
    pub fn set_func(&self, f: js_sys::Function) {
        self.f.borrow_mut().replace(f);
    }

    pub fn call(&self, arg1: Option<A>, arg2: Option<B>) -> bool {
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

    pub fn call1(&self, arg1: A) -> bool {
        self.call(Some(arg1), None)
    }

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

fn arg_to_jsvalue<A: Into<JsValue>>(arg: Option<A>) -> JsValue {
    match arg {
        None => JsValue::NULL,
        Some(inner) => inner.into(),
    }
}
