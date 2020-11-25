//! Reactive vector based on [`Vec`].

use std::{marker::PhantomData, slice::Iter};

use futures::{future::LocalBoxFuture, Stream};

use crate::subscribers_store::{progressable, SubscribersStore};

/// Reactive vector based on [`Vec`].
///
/// # Usage
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// use medea_reactive::collections::ObservableVec;
///
/// # executor::block_on(async {
/// let mut vec = ObservableVec::new();
///
/// // You can subscribe on push event:
/// let mut pushes = vec.on_push();
///
/// vec.push("foo");
///
/// let pushed_item = pushes.next().await.unwrap();
/// assert_eq!(pushed_item, "foo");
///
/// // Also you can subscribe on remove event:
/// let mut removals = vec.on_remove();
///
/// vec.remove(0);
///
/// let removed_item = removals.next().await.unwrap();
/// assert_eq!(removed_item, "foo");
///
/// // On Vec structure drop, all items will be sent to the on_remove stream:
/// vec.push("foo-1");
/// vec.push("foo-2");
/// drop(vec);
/// let removed_items: Vec<_> = removals.take(2)
///     .collect()
///     .await;
/// assert_eq!(removed_items[0], "foo-1");
/// assert_eq!(removed_items[1], "foo-2");
/// # });
/// ```
#[derive(Debug)]
pub struct ObservableVec<T, S: SubscribersStore<T, O>, O> {
    /// Data stored by this [`ObservableVec`].
    store: Vec<T>,

    /// Subscribers of the [`ObservableVec::on_push`] method.
    on_push_subs: S,

    /// Subscribers of the [`ObservableVec::on_remove`] method.
    on_remove_subs: S,

    /// Phantom type of [`ObservableVec::on_push`] and
    /// [`ObservableVec::on_remove`] output.
    _output: PhantomData<O>,
}

impl<T> ObservableVec<T, progressable::SubStore<T>, progressable::Value<T>>
where
    T: Clone + 'static,
{
    /// Returns [`Future`] which will be resolved when all push updates will be
    /// processed by [`ObservableVec::on_push`] subscribers.
    #[inline]
    #[must_use]
    pub fn when_push_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_push_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all remove updates will
    /// be processed by [`ObservableVec::on_remove`] subscribers.
    #[inline]
    #[must_use]
    pub fn when_remove_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_push_subs.when_all_processed()
    }
}

impl<T, S: SubscribersStore<T, O>, O> ObservableVec<T, S, O> {
    /// Returns new empty [`ObservableVec`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// An iterator visiting all elements in arbitrary order. The iterator
    /// element type is `&'a T`.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableVec`] on drop.
    #[inline]
    pub fn on_remove(&self) -> impl Stream<Item = O> {
        self.on_remove_subs.new_subscription(Vec::new())
    }
}

impl<T, S, O> ObservableVec<T, S, O>
where
    T: Clone,
    S: SubscribersStore<T, O>,
{
    /// Appends an element to the back of a collection.
    ///
    /// This will produce [`ObservableVec::on_push`] event.
    pub fn push(&mut self, value: T) {
        self.store.push(value.clone());

        self.on_push_subs.send_update(value);
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// This will produce [`ObservableVec::on_remove`] event.
    pub fn remove(&mut self, index: usize) -> T {
        let value = self.store.remove(index);
        self.on_remove_subs.send_update(value.clone());

        value
    }

    /// Returns the [`Stream`] to which the pushed values will be sent.
    ///
    /// Also to this [`Stream`] will be sent all already pushed values
    /// of this [`ObservableVec`].
    #[inline]
    pub fn on_push(&self) -> impl Stream<Item = O> {
        self.on_push_subs.new_subscription(self.store.to_vec())
    }
}

impl<T, S: SubscribersStore<T, O>, O> Default for ObservableVec<T, S, O> {
    #[inline]
    fn default() -> Self {
        Self {
            store: Vec::new(),
            on_push_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<T, S: SubscribersStore<T, O>, O> From<Vec<T>> for ObservableVec<T, S, O> {
    #[inline]
    fn from(from: Vec<T>) -> Self {
        Self {
            store: from,
            on_push_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, T, S: SubscribersStore<T, O>, O> IntoIterator
    for &'a ObservableVec<T, S, O>
{
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<T, S: SubscribersStore<T, O>, O> Drop for ObservableVec<T, S, O> {
    /// Sends all items of a dropped [`ObservableVec`] to the
    /// [`ObservableVec::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain(..).for_each(|value| {
            on_remove_subs.send_update(value);
        });
    }
}

impl<T, S, O> AsRef<[T]> for ObservableVec<T, S, O>
where
    T: Clone,
    S: SubscribersStore<T, O>,
{
    #[inline]
    fn as_ref(&self) -> &[T] {
        &self.store
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::StreamExt as _;
    use tokio::time::timeout;

    use crate::collections::ProgressableVec;

    mod when_push_completed {
        use super::*;

        #[tokio::test]
        async fn waits_for_processing() {
            let mut store = ProgressableVec::new();

            let _on_push = store.on_push();
            store.push(0);

            let when_push_completed = store.when_push_completed();

            let _ = timeout(Duration::from_millis(500), when_push_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn waits_for_value_drop() {
            let mut store = ProgressableVec::new();

            let mut on_push = store.on_push();
            store.push(0);
            let when_push_completed = store.when_push_completed();
            let _value = on_push.next().await.unwrap();

            let _ = timeout(Duration::from_millis(500), when_push_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn resolved_on_value_drop() {
            let mut store = ProgressableVec::new();

            let mut on_push = store.on_push();
            store.push(0);
            let when_push_completed = store.when_push_completed();
            drop(on_push.next().await.unwrap());

            timeout(Duration::from_millis(500), when_push_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn resolves_on_empty_sublist() {
            let mut store = ProgressableVec::new();

            store.push(0);
            let when_push_completed = store.when_push_completed();

            timeout(Duration::from_millis(50), when_push_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn waits_for_two_subs() {
            let mut store = ProgressableVec::new();

            let mut first_on_push = store.on_push();
            let _second_on_push = store.on_push();
            store.push(0);
            let when_all_push_processed = store.when_push_completed();

            drop(first_on_push.next().await.unwrap());

            let _ =
                timeout(Duration::from_millis(500), when_all_push_processed)
                    .await
                    .unwrap_err();
        }
    }
}
