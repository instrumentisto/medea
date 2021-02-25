use std::marker::PhantomData;

use wasm_bindgen::JsValue;

pub struct Function<T> {
    inner: js_sys::Function,
    _arg: PhantomData<T>,
}

impl Function<()> {
    pub fn call0(&self) {
        let _ = self.inner.call0(&JsValue::NULL);
    }
}

impl<T: Into<JsValue>> Function<T> {
    pub fn call1(&self, arg: T) {
        let _ = self.inner.call1(&JsValue::NULL, &arg.into());
    }
}

impl<T> From<js_sys::Function> for Function<T> {
    fn from(func: js_sys::Function) -> Self {
        Self {
            inner: func,
            _arg: Default::default(),
        }
    }
}
