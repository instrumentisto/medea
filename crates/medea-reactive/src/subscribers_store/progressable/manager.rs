//! Implementation of the object which responsible for [`Value`] lifecycle.

use std::rc::Rc;

use futures::future::LocalBoxFuture;

use crate::ObservableCell;

use super::value::Value;

/// Object responsible for creating new [`Value`]s on update and subscribing to
/// all data processed event.
#[derive(Clone, Debug)]
pub(crate) struct Manager(Rc<ObservableCell<u32>>);

impl Manager {
    /// Creates new [`Manager`] with a counter set to `0`.
    pub(crate) fn new() -> Self {
        Self(Rc::new(ObservableCell::new(0)))
    }

    /// Wraps provided value to [`Value`] and increments counter.
    pub(crate) fn new_value<D>(&self, value: D) -> Value<D> {
        self.0.mutate(|mut c| *c += 1);
        Value::new(value, Rc::clone(&self.0))
    }

    /// Returns [`Future`] which will be resolved when all subscribers will
    /// process update events.
    ///
    /// [`Future`]: std::future::Future
    pub(crate) fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        let fut = self.0.when_eq(0);
        Box::pin(async move {
            let _ = fut.await;
        })
    }
}
