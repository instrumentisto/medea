use std::{cell::RefCell, mem, ops::Deref, rc::Rc};

use futures::{channel::oneshot, future::LocalBoxFuture};

use super::ObservableCell;

#[derive(Clone, Debug)]
pub(crate) struct ProgressableManager {
    counter: Rc<ObservableCell<u32>>,
}

impl ProgressableManager {
    pub(crate) fn new() -> Self {
        Self {
            counter: Rc::new(ObservableCell::new(0)),
        }
    }

    pub(crate) fn incr_processors_count(&self, count: u32) {
        self.counter.mutate(|mut c| *c += count);
    }

    pub(crate) fn new_value<D>(
        &self,
        value: D,
    ) -> ProgressableObservableValue<D> {
        ProgressableObservableValue {
            value,
            counter: Rc::clone(&self.counter),
        }
    }

    pub(crate) fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        let fut = self.counter.when_eq(0);
        Box::pin(async move {
            let _ = fut.await;
        })
    }
}

#[derive(Debug)]
pub struct ProgressableObservableValue<D> {
    value: D,
    counter: Rc<ObservableCell<u32>>,
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
