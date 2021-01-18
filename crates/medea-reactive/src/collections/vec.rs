//! Reactive vector based on [`Vec`].

use std::{marker::PhantomData, slice::Iter};

use futures::stream::LocalBoxStream;

use crate::subscribers_store::{
    common, progressable,
    progressable::{processed::AllProcessed, Processed},
    SubscribersStore,
};

/// Reactive vector based on [`Vec`] with additional functionality of tracking
/// progress made by its subscribers. Its [`Vec::on_push()`] and
/// [`Vec::on_remove()`] subscriptions return values wrapped in a
/// [`progressable::Guarded`], and the implementation tracks all
/// [`progressable::Guard`]s.
pub type ProgressableVec<T> =
    Vec<T, progressable::SubStore<T>, progressable::Guarded<T>>;

/// Reactive vector based on [`Vec`].
pub type ObservableVec<T> = Vec<T, common::SubStore<T>, T>;

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
///
/// # Waiting for subscribers to complete
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// use medea_reactive::collections::ProgressableVec;
///
/// # executor::block_on(async {
/// let mut vec = ProgressableVec::new();
///
/// let mut on_push = vec.on_push();
/// vec.push(1);
///
/// // vec.when_push_processed().await; <- wouldn't be resolved
/// let value = on_push.next().await.unwrap();
/// // vec.when_push_processed().await; <- wouldn't be resolved
/// drop(value);
///
/// vec.when_push_processed().await; // will be resolved
/// # });
/// ```
#[derive(Debug)]
pub struct Vec<T, S: SubscribersStore<T, O>, O> {
    /// Data stored by this [`Vec`].
    store: std::vec::Vec<T>,

    /// Subscribers of the [`Vec::on_push`] method.
    on_push_subs: S,

    /// Subscribers of the [`Vec::on_remove`] method.
    on_remove_subs: S,

    /// Phantom type of [`Vec::on_push()`] and [`Vec::on_remove()`] output.
    _output: PhantomData<O>,
}

impl<T> ProgressableVec<T>
where
    T: Clone + 'static,
{
    /// Returns [`Future`] resolving when all push updates will be processed by
    /// [`Vec::on_push()`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_push_processed(&self) -> Processed<'static> {
        self.on_push_subs.when_all_processed()
    }

    /// Returns [`Future`] resolving when all remove updates will be processed
    /// by [`Vec::on_remove()`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_remove_processed(&self) -> Processed<'static> {
        self.on_remove_subs.when_all_processed()
    }

    /// Returns [`Future`] resolving when all push and remove updates will be
    /// processed by subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_all_processed(&self) -> AllProcessed<'static> {
        crate::when_all_processed(vec![
            self.when_remove_processed().into(),
            self.when_push_processed().into(),
        ])
    }
}

impl<T, S: SubscribersStore<T, O>, O> Vec<T, S, O> {
    /// Returns new empty [`Vec`].
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

    /// Returns the [`Stream`] to which the pushed values will be sent.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_push(&self) -> LocalBoxStream<'static, O> {
        self.on_push_subs.subscribe()
    }

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`Vec`] on drop.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_remove(&self) -> LocalBoxStream<'static, O> {
        self.on_remove_subs.subscribe()
    }
}

impl<T, S, O> Vec<T, S, O>
where
    T: Clone,
    S: SubscribersStore<T, O>,
    O: 'static,
{
    /// Appends a value to the back of this [`Vec`].
    ///
    /// This will produce [`Vec::on_push()`] event.
    pub fn push(&mut self, value: T) {
        self.store.push(value.clone());

        self.on_push_subs.send_update(value);
    }

    /// Removes and returns the value at position `index` within this [`Vec`],
    /// shifting all values after it to the left.
    ///
    /// This will produce [`Vec::on_remove()`] event.
    pub fn remove(&mut self, index: usize) -> T {
        let value = self.store.remove(index);
        self.on_remove_subs.send_update(value.clone());

        value
    }

    /// Returns [`Stream`] containing values from this [`Vec`].
    ///
    /// Returned [`Stream`] contains only current values. It won't update on new
    /// pushes, but you can merge returned [`Stream`] with a [`Vec::on_push`]
    /// [`Stream`] if you want to process current values and values that will be
    /// inserted.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn replay_on_push(&self) -> LocalBoxStream<'static, O> {
        Box::pin(futures::stream::iter(
            self.store
                .clone()
                .into_iter()
                .map(|val| self.on_push_subs.wrap(val))
                .collect::<std::vec::Vec<_>>(),
        ))
    }
}

impl<T, S: SubscribersStore<T, O>, O> Default for Vec<T, S, O> {
    #[inline]
    fn default() -> Self {
        Self {
            store: std::vec::Vec::new(),
            on_push_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<T, S: SubscribersStore<T, O>, O> From<std::vec::Vec<T>> for Vec<T, S, O> {
    #[inline]
    fn from(from: std::vec::Vec<T>) -> Self {
        Self {
            store: from,
            on_push_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, T, S: SubscribersStore<T, O>, O> IntoIterator for &'a Vec<T, S, O> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<T, S: SubscribersStore<T, O>, O> Drop for Vec<T, S, O> {
    /// Sends all items of a dropped [`Vec`] to the [`Vec::on_remove()`]
    /// subscriptions.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain(..).for_each(|value| {
            on_remove_subs.send_update(value);
        });
    }
}

impl<T, S, O> AsRef<[T]> for Vec<T, S, O>
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
    use futures::{poll, task::Poll, StreamExt as _};

    use super::ProgressableVec;

    #[tokio::test]
    async fn replay_on_push() {
        let mut vec = ProgressableVec::from(vec![1, 2, 3]);

        let replay_on_push = vec.replay_on_push();
        let on_push = vec.on_push();

        vec.push(4);

        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
        let replayed: Vec<_> = replay_on_push.collect().await;
        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);

        let replayed: Vec<_> =
            replayed.into_iter().map(|val| val.into_inner()).collect();

        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
        drop(on_push);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));

        assert_eq!(replayed.len(), 3);
        assert!(replayed.contains(&1));
        assert!(replayed.contains(&2));
        assert!(replayed.contains(&3));
    }

    #[tokio::test]
    async fn when_push_processed() {
        let mut vec = ProgressableVec::new();
        let _ = vec.push(0);

        let mut on_push = vec.on_push();

        assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));
        let _ = vec.push(1);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
        //
        let (val, guard) = on_push.next().await.unwrap().into_parts();

        assert_eq!(val, 1);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
        drop(guard);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn multiple_when_push_processed_subs() {
        let mut vec = ProgressableVec::new();
        let _ = vec.push(0);

        let mut on_push1 = vec.on_push();
        let mut on_push2 = vec.on_push();

        assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));
        vec.push(0);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);

        assert_eq!(on_push1.next().await.unwrap().into_inner(), 0);
        assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
        assert_eq!(on_push2.next().await.unwrap().into_inner(), 0);

        assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn when_remove_processed() {
        let mut vec = ProgressableVec::new();
        let _ = vec.push(10);

        let mut on_remove = vec.on_remove();

        assert_eq!(poll!(vec.when_remove_processed()), Poll::Ready(()));
        assert_eq!(vec.remove(0), 10);
        assert_eq!(poll!(vec.when_remove_processed()), Poll::Pending);

        let (val, guard) = on_remove.next().await.unwrap().into_parts();

        assert_eq!(val, 10);
        assert_eq!(poll!(vec.when_remove_processed()), Poll::Pending);
        drop(guard);
        assert_eq!(poll!(vec.when_remove_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn multiple_when_remove_processed_subs() {
        let mut vec = ProgressableVec::new();
        let _ = vec.push(10);

        let mut on_remove1 = vec.on_remove();
        let mut on_remove2 = vec.on_remove();

        assert_eq!(poll!(vec.when_remove_processed()), Poll::Ready(()));
        assert_eq!(vec.remove(0), 10);
        assert_eq!(poll!(vec.when_remove_processed()), Poll::Pending);

        assert_eq!(on_remove1.next().await.unwrap().into_inner(), 10);
        assert_eq!(poll!(vec.when_remove_processed()), Poll::Pending);
        assert_eq!(on_remove2.next().await.unwrap().into_inner(), 10);

        assert_eq!(poll!(vec.when_remove_processed()), Poll::Ready(()));
    }
}
