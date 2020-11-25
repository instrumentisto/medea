use std::{ops::Deref, rc::Rc};

use crate::ObservableCell;

#[derive(Debug)]
pub struct ProgressableObservableValue<D> {
    value: D,
    counter: Rc<ObservableCell<u32>>,
}

impl<D> ProgressableObservableValue<D> {
    pub fn new(
        value: D,
        counter: Rc<ObservableCell<u32>>,
    ) -> ProgressableObservableValue<D> {
        Self { value, counter }
    }
}

impl<D> Drop for ProgressableObservableValue<D> {
    fn drop(&mut self) {
        self.counter.mutate(|mut c| *c -= 1);
    }
}

impl<D> AsRef<D> for ProgressableObservableValue<D> {
    fn as_ref(&self) -> &D {
        &self.value
    }
}

impl<D> Deref for ProgressableObservableValue<D> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
