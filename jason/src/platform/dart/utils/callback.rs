use dart_sys::Dart_Handle;
use std::{cell::RefCell, marker::PhantomData};

pub struct Callback<A>(RefCell<Option<Function<A>>>);

impl<A> Callback<A> {
    pub fn set_func(&self, f: Function<A>) {
        todo!()
    }

    pub fn is_set(&self) -> bool {
        todo!()
    }
}

// TODO: Maybe it's not needed
impl Callback<()> {
    pub fn call0(&self) {
        todo!()
    }
}

impl<A> Default for Callback<A> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

pub struct Function<A> {
    handle: Dart_Handle,
    _ty: PhantomData<A>,
}

impl Function<()> {
    pub fn call0(&self) {
        todo!()
    }
}

impl<A> Function<A> {
    pub fn call1(&self, arg: A) {
        todo!()
    }
}
