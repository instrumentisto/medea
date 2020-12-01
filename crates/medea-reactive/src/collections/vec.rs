//! Reactive vector based on [`Vec`].

use std::{marker::PhantomData, slice::Iter};

use futures::{
    future, future::LocalBoxFuture, stream::LocalBoxStream, FutureExt as _,
};

use crate::subscribers_store::{common, progressable, SubscribersStore};

/// Reactive vector based on [`Vec`] with ability to recognise when all updates
/// was processed by subscribers.
pub type ProgressableVec<T> =
    Vec<T, progressable::SubStore<T>, progressable::Value<T>>;
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
/// # Usage of when all completed functions
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
    inner: std::vec::Vec<T>,

    /// Subscribers of the [`Vec::on_push`] method.
    push_subs: S,

    /// Subscribers of the [`Vec::on_remove`] method.
    remove_subs: S,

    /// Phantom type of [`Vec::on_push`] and
    /// [`Vec::on_remove`] output.
    _output: PhantomData<O>,
}

impl<T> ProgressableVec<T>
where
    T: Clone + 'static,
{
    /// Returns [`Future`] which will be resolved when all push updates will be
    /// processed by [`Vec::on_push`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_push_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.push_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all remove updates will
    /// be processed by [`Vec::on_remove`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_remove_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.remove_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all push and remove
    /// updates will be processed by subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        Box::pin(
            future::join(
                self.when_remove_processed(),
                self.when_push_processed(),
            )
            .map(|(_, _)| ()),
        )
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

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`Vec`] on drop.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_remove(&self) -> LocalBoxStream<'static, O> {
        self.remove_subs.new_subscription(std::vec::Vec::new())
    }
}

impl<T, S, O> Vec<T, S, O>
where
    T: Clone,
    S: SubscribersStore<T, O>,
{
    /// Appends an element to the back of a collection.
    ///
    /// This will produce [`Vec::on_push`] event.
    pub fn push(&mut self, value: T) {
        self.inner.push(value.clone());

        self.push_subs.send_update(value);
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// This will produce [`Vec::on_remove`] event.
    pub fn remove(&mut self, index: usize) -> T {
        let value = self.inner.remove(index);
        self.remove_subs.send_update(value.clone());

        value
    }

    /// Returns the [`Stream`] to which the pushed values will be sent.
    ///
    /// Also to this [`Stream`] will be sent all already pushed values
    /// of this [`Vec`].
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_push(&self) -> LocalBoxStream<'static, O> {
        self.push_subs.new_subscription(self.inner.to_vec())
    }
}

impl<T, S: SubscribersStore<T, O>, O> Default for Vec<T, S, O> {
    #[inline]
    fn default() -> Self {
        Self {
            inner: std::vec::Vec::new(),
            push_subs: S::default(),
            remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<T, S: SubscribersStore<T, O>, O> From<std::vec::Vec<T>> for Vec<T, S, O> {
    #[inline]
    fn from(from: std::vec::Vec<T>) -> Self {
        Self {
            inner: from,
            push_subs: S::default(),
            remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, T, S: SubscribersStore<T, O>, O> IntoIterator for &'a Vec<T, S, O> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<T, S: SubscribersStore<T, O>, O> Drop for Vec<T, S, O> {
    /// Sends all items of a dropped [`Vec`] to the
    /// [`Vec::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.inner;
        let on_remove_subs = &self.remove_subs;
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
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::{poll, task::Poll, StreamExt as _};
    use tokio::time::timeout;

    use crate::collections::ProgressableVec;

    mod when_push_processed {
        use super::*;

        #[tokio::test]
        async fn wait_for_push() {
            let mut vec = ProgressableVec::new();

            let on_push = vec.on_push();
            vec.push(0);

            assert_eq!(poll!(vec.when_push_processed()), Poll::Pending);
            drop(on_push);
            assert_eq!(poll!(vec.when_push_processed()), Poll::Ready(()));
        }

        #[tokio::test]
        async fn wait_for_remove() {
            let mut vec = ProgressableVec::new();

            let on_remove = vec.on_remove();
            vec.push(0);
            let _ = vec.remove(0);

            assert_eq!(poll!(vec.when_remove_processed()), Poll::Pending);
            drop(on_remove);
            assert_eq!(poll!(vec.when_remove_processed()), Poll::Ready(()));
        }

        #[tokio::test]
        async fn resolves_on_empty_sublist() {
            let mut vec = ProgressableVec::new();

            vec.push(0);
            let when_push_processed = vec.when_push_processed();

            timeout(Duration::from_millis(50), when_push_processed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn waits_for_two_subs() {
            let mut vec = ProgressableVec::new();

            let mut first_on_push = vec.on_push();
            let _second_on_push = vec.on_push();
            vec.push(0);
            let when_all_push_processed = vec.when_push_processed();

            drop(first_on_push.next().await.unwrap());

            let _ =
                timeout(Duration::from_millis(500), when_all_push_processed)
                    .await
                    .unwrap_err();
        }
    }
}
