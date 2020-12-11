use std::cell::{Ref, RefCell};

use futures::{future::LocalBoxFuture, stream::LocalBoxStream};

use crate::{
    subscribers_store::progressable, Guarded, MutObservableFieldGuard,
};

use super::Progressable;

#[derive(Debug)]
pub struct ProgressableCell<D>(RefCell<Progressable<D>>);

impl<D> ProgressableCell<D>
where
    D: 'static,
{
    #[inline]
    pub fn new(data: D) -> Self {
        Self(RefCell::new(Progressable::new(data)))
    }

    /// Returns immutable reference to an underlying data.
    #[inline]
    pub fn borrow(&self) -> Ref<'_, D> {
        let reference = self.0.borrow();
        Ref::map(reference, |observable| &**observable)
    }
}

impl<D> ProgressableCell<D>
where
    D: Clone + 'static,
{
    /// Returns copy of an underlying data.
    #[inline]
    pub fn get(&self) -> D {
        self.0.borrow().data.clone()
    }

    /// Returns [`Stream`] into which underlying data updates will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn subscribe(&self) -> LocalBoxStream<'static, Guarded<D>> {
        self.0.borrow().subscribe()
    }

    /// Returns [`Future`] which will be resolved when all data updates will be
    /// processed by subscribers.
    ///
    /// [`Future`]: std::future::Future
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.0.borrow().when_all_processed()
    }
}

impl<D> ProgressableCell<D>
where
    D: Clone + PartialEq + 'static,
{
    /// Sets the `new_data` value as an underlying data.
    #[inline]
    pub fn set(&self, new_data: D) {
        *self.0.borrow_mut().borrow_mut() = new_data;
    }

    /// Replaces the contained underlying data with the given `new_data` value,
    /// and returns the old one.
    #[inline]
    pub fn replace(&self, mut new_data: D) -> D {
        std::mem::swap(&mut *self.0.borrow_mut().borrow_mut(), &mut new_data);
        new_data
    }

    /// Updates an underlying data using the provided function, which will
    /// accept a mutable reference to an underlying data.
    #[inline]
    pub fn mutate<F>(&self, f: F)
    where
        F: FnOnce(MutObservableFieldGuard<'_, D, progressable::SubStore<D>>),
    {
        (f)(self.0.borrow_mut().borrow_mut());
    }
}
