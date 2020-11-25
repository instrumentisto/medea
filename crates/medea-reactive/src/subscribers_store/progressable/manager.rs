use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::ObservableCell;

use super::value::Value;

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

    pub(crate) fn new_value<D>(&self, value: D) -> Value<D> {
        Value::new(value, Rc::clone(&self.counter))
    }

    pub(crate) fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        let fut = self.counter.when_eq(0);
        Box::pin(async move {
            let _ = fut.await;
        })
    }
}
