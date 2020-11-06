//! Implementation of the observable analogue of the [`Cell`].
//!
//! Subscription to changes works the same way as [`ObservableField`],
//! but working with underlying data of [`ObservableCell`] is different.
//!
//! [`Cell`]: std::cell::Cell
//! [`ObservableField`]: crate::ObservableField

#![allow(clippy::module_name_repetitions)]

use std::cell::{Ref, RefCell};

use futures::{future::LocalBoxFuture, stream::LocalBoxStream};

use super::{
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
/// [`ObservableField`]: crate::ObservableField
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
        Ref::map(reference, |observable| &**observable)
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
        std::mem::swap(&mut *self.0.borrow_mut().borrow_mut(), &mut new_data);
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

#[cfg(test)]
mod observable_cell {
    use std::time::Duration;

    use futures::StreamExt as _;
    use tokio::time::timeout;

    use crate::ObservableCell;

    #[tokio::test]
    async fn subscription() {
        let field = ObservableCell::new(0);
        let subscription = field.subscribe();

        field.set(100);
        assert_eq!(subscription.skip(1).next().await.unwrap(), 100);
    }

    #[tokio::test]
    async fn when() {
        let field = ObservableCell::new(0);
        let when_will_be_greater_than_5 = field.when(|upd| upd > &5);

        field.set(6);
        timeout(
            Duration::from_millis(50),
            Box::pin(when_will_be_greater_than_5),
        )
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn when_eq() {
        let field = ObservableCell::new(0);
        let when_will_be_5 = field.when_eq(5);

        field.set(5);
        timeout(Duration::from_millis(50), Box::pin(when_will_be_5))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn only_initial_update_emitted() {
        let field = ObservableCell::new(0);
        let mut subscription = field.subscribe();
        assert_eq!(subscription.next().await.unwrap(), 0);

        let _ =
            timeout(Duration::from_millis(10), Box::pin(subscription.next()))
                .await
                .unwrap_err();
    }

    #[tokio::test]
    async fn when_eq_never_resolves() {
        let field = ObservableCell::new(0);
        let when_will_be_5 = field.when_eq(5);

        let _ = timeout(Duration::from_millis(10), Box::pin(when_will_be_5))
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn data_mutates() {
        let field = ObservableCell::new(0);
        assert_eq!(*field.borrow(), 0);
        field.set(100_500);
        assert_eq!(*field.borrow(), 100_500);
    }

    #[tokio::test]
    async fn updates_emitted_on_replace() {
        let field = ObservableCell::new(0);
        let mut subscription = field.subscribe().skip(1);

        assert_eq!(field.replace(100), 0);
        assert_eq!(*field.borrow(), 100);

        assert_eq!(subscription.next().await.unwrap(), 100);
    }

    #[tokio::test]
    async fn when_on_replace() {
        let field = ObservableCell::new(0);
        let when_will_be_greater_than_5 = field.when(|upd| upd > &5);

        assert_eq!(field.replace(6), 0);

        timeout(
            Duration::from_millis(50),
            Box::pin(when_will_be_greater_than_5),
        )
        .await
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn when_eq_on_replace() {
        let field = ObservableCell::new(0);
        let when_will_be_5 = field.when_eq(5);

        assert_eq!(field.replace(5), 0);

        timeout(Duration::from_millis(50), Box::pin(when_will_be_5))
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn get() {
        let field = ObservableCell::new(0);
        assert_eq!(field.get(), 0);

        field.set(5);
        assert_eq!(field.get(), 5);

        assert_eq!(field.replace(10), 5);
        assert_eq!(field.get(), 10);
    }

    #[tokio::test]
    async fn emits_changes_on_mutate() {
        let field = ObservableCell::new(0);
        let mut subscription = field.subscribe().skip(1);

        field.mutate(|mut data| *data = 100);
        assert_eq!(subscription.next().await.unwrap(), 100);
    }

    #[tokio::test]
    async fn when_with_mutate() {
        let field = ObservableCell::new(0);
        let when_will_be_5 = field.when_eq(5);

        field.mutate(|mut data| *data = 5);
        timeout(Duration::from_millis(50), Box::pin(when_will_be_5))
            .await
            .unwrap()
            .unwrap();
    }
}
