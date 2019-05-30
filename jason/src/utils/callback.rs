use std::cell::RefCell;
use std::marker::PhantomData;
use wasm_bindgen::JsValue;

pub struct Callback<T, E> {
    f: RefCell<Option<js_sys::Function>>,
    _phantom: PhantomData<T>,
    _phantom2: PhantomData<E>,
}

impl<T, E> Default for Callback<T, E> {
    fn default() -> Self {
        Self {
            f: RefCell::new(None),
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}

impl<T: Into<JsValue>, E: Into<JsValue>> Callback<T, E> {
    pub fn new() -> Self {
        Self {
            f: RefCell::new(None),
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }

    pub fn set_func(&self, f: js_sys::Function) {
        self.f.borrow_mut().replace(f);
    }

    pub fn call(&self, result: Result<T, E>) -> bool {
        match self.f.borrow().as_ref() {
            None => false,
            Some(f) => {
                match result {
                    Ok(ok) => {
                        let _ = f
                            .call2(&JsValue::NULL, &ok.into(), &JsValue::NULL)
                            .is_ok();
                    }
                    Err(err) => {
                        let _ = f.call2(
                            &JsValue::NULL,
                            &JsValue::NULL,
                            &err.into(),
                        );
                    }
                }
                true
            }
        }
    }

    pub fn call_err(&self, err: E) -> bool {
        self.call(Err(err))
    }

    pub fn call_ok(&self, ok: T) -> bool {
        self.call(Ok(ok))
    }
}

impl<T: Into<JsValue>, E: Into<JsValue>> From<js_sys::Function>
    for Callback<T, E>
{
    fn from(f: js_sys::Function) -> Self {
        Self {
            f: RefCell::new(Some(f)),
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}
