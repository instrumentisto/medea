//! Reactive hash set based on [`HashSet`].

use std::{collections::hash_set::Iter, hash::Hash, marker::PhantomData};

use futures::{
    future, future::LocalBoxFuture, stream::LocalBoxStream, FutureExt,
};

use crate::subscribers_store::{common, progressable, SubscribersStore};

/// Reactive hash set based on [`HashSet`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashSet<T> =
    HashSet<T, progressable::SubStore<T>, progressable::Guarded<T>>;
/// Reactive hash set based on [`HashSet`].
pub type ObservableHashSet<T> = HashSet<T, common::SubStore<T>, T>;

/// Reactive hash set based on [`HashSet`].
///
/// # Usage
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// # use std::collections::HashSet;
/// use medea_reactive::collections::ObservableHashSet;
///
/// # executor::block_on(async {
/// let mut set = ObservableHashSet::new();
///
/// // You can subscribe on insert action:
/// let mut inserts = set.on_insert();
///
/// set.insert("foo");
///
/// let item = inserts.next()
///     .await
///     .unwrap();
/// assert_eq!(item, "foo");
///
/// // Also you can subscribe on remove action:
/// let mut removals = set.on_remove();
///
/// set.remove(&"foo");
///
/// let removed_item = removals.next()
///     .await
///     .unwrap();
/// assert_eq!(removed_item, "foo");
///
/// // When you update HashSet by another HashSet all events will
/// // work fine:
/// set.insert("foo-1");
/// set.insert("foo-2");
/// set.insert("foo-3");
///
/// let mut set_for_update = HashSet::new();
/// set_for_update.insert("foo-1");
/// set_for_update.insert("foo-4");
/// set.update(set_for_update);
///
/// let removed_items: HashSet<_> = removals.take(2)
///     .collect()
///     .await;
/// let inserted_item = inserts.skip(3)
///     .next()
///     .await
///     .unwrap();
/// assert!(removed_items.contains("foo-2"));
/// assert!(removed_items.contains("foo-3"));
/// assert_eq!(inserted_item, "foo-4");
/// assert!(set.contains(&"foo-1"));
/// assert!(set.contains(&"foo-4"));
/// # });
/// ```
///
/// # Usage of when all completed functions
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// use medea_reactive::collections::ProgressableHashSet;
///
/// # executor::block_on(async {
/// let mut hash_set = ProgressableHashSet::new();
///
/// let mut on_insert = hash_set.on_insert();
/// hash_set.insert(1);
///
/// // hash_set.when_insert_processed().await; <- wouldn't be resolved
/// let value = on_insert.next().await.unwrap();
/// // hash_set.when_insert_processed().await; <- wouldn't be resolved
/// drop(value);
///
/// hash_set.when_insert_processed().await; // will be resolved
/// # });
/// ```
#[derive(Debug)]
pub struct HashSet<T, S: SubscribersStore<T, O>, O> {
    /// Data stored by this [`HashSet`].
    inner: std::collections::HashSet<T>,

    /// Subscribers of the [`HashSet::on_insert`] method.
    insert_subs: S,

    /// Subscribers of the [`HashSet::on_remove`] method.
    remove_subs: S,

    /// Phantom type of [`HashSet::on_insert`] and
    /// [`HashSet::on_remove`] output.
    _output: PhantomData<O>,
}

impl<T> ProgressableHashSet<T>
where
    T: Clone + 'static,
{
    /// Returns [`Future`] which will be resolved when all push updates will be
    /// processed by [`HashSet::on_insert`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_insert_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.insert_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all remove updates will
    /// be processed by [`HashSet::on_remove`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_remove_processed(&self) -> LocalBoxFuture<'static, ()> {
        self.remove_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all insert and remove
    /// updates will be processed by subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    #[must_use]
    pub fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        Box::pin(
            future::join(
                self.when_remove_processed(),
                self.when_insert_processed(),
            )
            .map(|(_, _)| ()),
        )
    }
}

impl<T, S: SubscribersStore<T, O>, O> HashSet<T, S, O> {
    /// Returns new empty [`HashSet`].
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

    /// Returns the [`Stream`] to which the inserted values will be
    /// sent.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_insert(&self) -> LocalBoxStream<'static, O> {
        self.insert_subs.subscribe()
    }

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`HashSet`] on drop.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn on_remove(&self) -> LocalBoxStream<'static, O> {
        self.remove_subs.subscribe()
    }
}

impl<T, S, O> HashSet<T, S, O>
where
    T: Clone + 'static,
    S: SubscribersStore<T, O>,
    O: 'static,
{
    /// Returns the [`Stream`] with all already inserted values of this
    /// [`HashSet`].
    ///
    /// This [`Stream`] will have only current values. It doesn't updates on new
    /// inserts, but you can merge ([`stream::select`]) this [`Stream`] with a
    /// [`HashSet::on_insert`] [`Stream`] for that.
    ///
    /// [`Stream`]: futures::Stream
    /// [`stream::select`]: futures::stream::select
    #[inline]
    pub fn replay_on_insert(&self) -> LocalBoxStream<'static, O> {
        Box::pin(futures::stream::iter(
            self.inner
                .clone()
                .into_iter()
                .map(|val| self.insert_subs.wrap(val))
                .collect::<Vec<_>>(),
        ))
    }
}

impl<T, S, O> HashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    /// Adds a value to the set.
    ///
    /// If the set did not have this value present, `true` is returned.
    ///
    /// If the set did have this value present, `false` is returned.
    ///
    /// This will produce [`HashSet::on_insert`] event.
    pub fn insert(&mut self, value: T) -> bool {
        if self.inner.insert(value.clone()) {
            self.insert_subs.send_update(value);
            true
        } else {
            false
        }
    }

    /// Removes a value from the set. Returns whether the value was present in
    /// the set.
    ///
    /// This will produce [`HashSet::on_remove`] event.
    pub fn remove(&mut self, value: &T) -> Option<T> {
        let value = self.inner.take(value);

        if let Some(value) = &value {
            self.remove_subs.send_update(value.clone());
        }

        value
    }

    /// Makes this [`HashSet`] exactly the same as the passed
    /// [`HashSet`].
    ///
    /// This function will calculate diff between [`HashSet`] and
    /// provided [`HashSet`] and will spawn [`HashSet::on_insert`]
    /// and [`HashSet::on_remove`] if set is changed.
    ///
    /// For the usage example you can read [`HashSet`] doc.
    pub fn update(&mut self, updated: std::collections::HashSet<T>) {
        let removed_elems = self.inner.difference(&updated);
        let inserted_elems = updated.difference(&self.inner);

        for removed_elem in removed_elems {
            self.remove_subs.send_update(removed_elem.clone());
        }

        for inserted_elem in inserted_elems {
            self.insert_subs.send_update(inserted_elem.clone());
        }

        self.inner = updated;
    }

    /// Returns `true` if the set contains a value.
    #[inline]
    pub fn contains(&self, value: &T) -> bool {
        self.inner.contains(value)
    }
}

impl<T, S, O> Default for HashSet<T, S, O>
where
    S: SubscribersStore<T, O>,
{
    #[inline]
    fn default() -> Self {
        Self {
            inner: std::collections::HashSet::new(),
            insert_subs: S::default(),
            remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, T, S: SubscribersStore<T, O>, O> IntoIterator
    for &'a HashSet<T, S, O>
{
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

impl<T, S, O> Drop for HashSet<T, S, O>
where
    S: SubscribersStore<T, O>,
{
    /// Sends all values of a dropped [`HashSet`] to the
    /// [`HashSet::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.inner;
        let on_remove_subs = &self.remove_subs;
        store.drain().for_each(|value| {
            on_remove_subs.send_update(value);
        });
    }
}

impl<T, S, O> From<std::collections::HashSet<T>> for HashSet<T, S, O>
where
    S: SubscribersStore<T, O>,
{
    #[inline]
    fn from(from: std::collections::HashSet<T>) -> Self {
        Self {
            inner: from,
            insert_subs: S::default(),
            remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}
