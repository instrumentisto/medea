use std::{ops::Deref, rc::Rc};

use crate::ObservableCell;

/// Wrapper around data which will decrement underlying counter on [`Drop`].
#[derive(Debug)]
pub struct Value<D> {
    value: D,
    counter: Rc<ObservableCell<u32>>,
}

impl<D> Value<D> {
    /// Returns new [`Value`] with a provided data and counter.
    pub fn new(value: D, counter: Rc<ObservableCell<u32>>) -> Value<D> {
        Self { value, counter }
    }
}

impl<D> Drop for Value<D> {
    fn drop(&mut self) {
        self.counter.mutate(|mut c| *c -= 1);
    }
}

impl<D> AsRef<D> for Value<D> {
    fn as_ref(&self) -> &D {
        &self.value
    }
}

impl<D> Deref for Value<D> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
