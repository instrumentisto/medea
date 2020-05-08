//! Implementation of the observable analogue of the [`Cell`].
//!
//! Subscription to changes works the same way as [`ObservableField`],
//! but working with underlying data of [`ObservableCell`] is different.

#![allow(clippy::module_name_repetitions)]

use std::{
    cell::{Ref, RefCell},
    ops::{Deref, DerefMut},
};

use futures::{future::LocalBoxFuture, stream::LocalBoxStream};

use crate::{
    DefaultSubscribers, DroppedError, MutObservableFieldGuard, Observable,
};

/// Observable analogue of [`Cell`].
///
/// Subscription to changes works the same way as [`ObservableField`],
/// but working with underlying data of [`ObservableCell`] is different.
///
/// # `ObservableCell` underlying data access
///
/// ## For `Copy` types
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// let foo = ObservableCell::new(0i32);
///
/// // If data implements `Copy` then you can get a copy of the current value:
/// assert_eq!(foo.get(), 0);
/// ```
///
/// ## Reference to an underlying data
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// struct Foo(i32);
///
/// impl Foo {
///     pub fn new(num: i32) -> Self {
///         Self(num)
///     }
///
///     pub fn get_num(&self) -> i32 {
///         self.0
///     }
/// }
///
/// let foo = ObservableCell::new(Foo::new(100));
/// assert_eq!(foo.borrow().get_num(), 100);
/// ```
///
/// # Mutation of an underlying data
///
/// ```
/// use medea_reactive::ObservableCell;
///
/// let foo = ObservableCell::new(0i32);
///
/// // You can just set some data:
/// foo.set(100);
/// assert_eq!(foo.get(), 100);
///
/// // Or replace data with new data and get the old data:
/// let old_value = foo.replace(200);
/// assert_eq!(old_value, 100);
/// assert_eq!(foo.get(), 200);
///
/// // Or mutate this data:
/// foo.mutate(|mut data| *data = 300);
/// assert_eq!(foo.get(), 300);
/// ```
///
/// [`Cell`]: std::cell::Cell
#[derive(Debug)]
pub struct ObservableCell<D>(RefCell<Observable<D>>);

impl<D> ObservableCell<D>
where
    D: 'static,
{
    /// Returns new [`ObservableCell`] with subscribable mutations.
    ///
    /// Also, you can subscribe to concrete mutation with
    /// [`ObservableCell::when`] or [`ObservableCell::when_eq`] methods.
    ///
    /// This container can mutate internally. See [`ObservableCell`] docs
    /// for more info.
    #[inline]
    pub fn new(data: D) -> Self {
        Self(RefCell::new(Observable::new(data)))
    }

    /// Returns immutable reference to an underlying data.
    #[inline]
    pub fn borrow(&self) -> Ref<'_, D> {
        let reference = self.0.borrow();
        Ref::map(reference, |observable| observable.deref())
    }

    /// Returns [`Future`] which will resolve only on modifications that
    /// the given `assert_fn` returns `true` on.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when<F>(
        &self,
        assert_fn: F,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>>
    where
        F: Fn(&D) -> bool + 'static,
    {
        self.0.borrow().when(assert_fn)
    }
}

impl<D> ObservableCell<D>
where
    D: Copy + 'static,
{
    /// Returns copy of an underlying data.
    #[inline]
    pub fn get(&self) -> D {
        **self.0.borrow()
    }
}

impl<D> ObservableCell<D>
where
    D: Clone + 'static,
{
    /// Returns [`Stream`] into which underlying data updates will be emitted.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn subscribe(&self) -> LocalBoxStream<'static, D> {
        self.0.borrow().subscribe()
    }
}

impl<D> ObservableCell<D>
where
    D: PartialEq + 'static,
{
    /// Returns [`Future`] which will resolve only when data of this
    /// [`ObservableCell`] will become equal to the provided `should_be` value.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_eq(
        &self,
        should_be: D,
    ) -> LocalBoxFuture<'static, Result<(), DroppedError>> {
        self.0.borrow().when_eq(should_be)
    }
}

impl<D> ObservableCell<D>
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
        std::mem::swap(
            self.0.borrow_mut().borrow_mut().deref_mut(),
            &mut new_data,
        );
        new_data
    }

    /// Updates an underlying data using the provided function, which will
    /// accept a mutable reference to an underlying data.
    #[inline]
    pub fn mutate<F>(&self, f: F)
    where
        F: FnOnce(MutObservableFieldGuard<'_, D, DefaultSubscribers<D>>),
    {
        (f)(self.0.borrow_mut().borrow_mut());
    }
}
