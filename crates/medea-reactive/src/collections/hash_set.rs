//! Reactive hash set based on [`HashSet`].

use std::{collections::hash_set::Iter, hash::Hash, marker::PhantomData};

use futures::stream::LocalBoxStream;

use crate::subscribers_store::{
    common, progressable,
    progressable::{AllProcessed, Processed},
    SubscribersStore,
};

/// Reactive hash set based on [`HashSet`] with an ability to recognize when all
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
/// # Waiting for subscribers to complete
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
    store: std::collections::HashSet<T>,

    /// Subscribers of the [`HashSet::on_insert()`] method.
    on_insert_subs: S,

    /// Subscribers of the [`HashSet::on_remove()`] method.
    on_remove_subs: S,

    /// Phantom type of [`HashSet::on_insert()`] and [`HashSet::on_remove()`]
    /// output.
    _output: PhantomData<O>,
}

impl<T> ProgressableHashSet<T>
where
    T: Clone + 'static,
{
    /// Returns [`Future`] resolving when all push updates will be processed by
    /// [`HashSet::on_insert()`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_insert_processed(&self) -> Processed<'static> {
        self.on_insert_subs.when_all_processed()
    }

    /// Returns [`Future`] resolving when all remove updates will be processed
    /// by [`HashSet::on_remove()`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_remove_processed(&self) -> Processed<'static> {
        self.on_remove_subs.when_all_processed()
    }

    /// Returns [`Future`] resolving when all insert and remove updates will be
    /// processed by subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_all_processed(&self) -> AllProcessed<'static> {
        crate::when_all_processed(vec![
            self.when_remove_processed().into(),
            self.when_insert_processed().into(),
        ])
    }
}

impl<T, S: SubscribersStore<T, O>, O> HashSet<T, S, O> {
    /// Creates new empty [`HashSet`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns [`Iterator`] visiting all values in an arbitrary order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    /// Returns [`Stream`] yielding inserted values to this [`HashSet`].
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    #[must_use]
    pub fn on_insert(&self) -> LocalBoxStream<'static, O> {
        self.on_insert_subs.subscribe()
    }

    /// Returns the [`Stream`] yielding removed values from this [`HashSet`].
    ///
    /// Note, that this [`Stream`] will yield all values of this [`HashSet`] on
    /// [`Drop`].
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    #[must_use]
    pub fn on_remove(&self) -> LocalBoxStream<'static, O> {
        self.on_remove_subs.subscribe()
    }
}

impl<T, S, O> HashSet<T, S, O>
where
    T: Clone + 'static,
    S: SubscribersStore<T, O>,
    O: 'static,
{
    /// Returns [`Stream`] containing values from this [`HashSet`].
    ///
    /// Returned [`Stream`] contains only current values. It won't update on new
    /// inserts, but you can merge returned [`Stream`] with a
    /// [`HashSet::on_insert()`] [`Stream`] if you want to process current
    /// values and values that will be inserted.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn replay_on_insert(&self) -> LocalBoxStream<'static, O> {
        Box::pin(futures::stream::iter(
            self.store
                .clone()
                .into_iter()
                .map(|val| self.on_insert_subs.wrap(val))
                .collect::<Vec<_>>(),
        ))
    }
}

impl<T, S, O> HashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    /// Adds the `value` to this [`HashSet`].
    ///
    /// If it didn't have such `value` present, `true` is returned.
    ///
    /// If it did have such `value` present, `false` is returned.
    ///
    /// This will produce [`HashSet::on_inser()t`] event.
    pub fn insert(&mut self, value: T) -> bool {
        if self.store.insert(value.clone()) {
            self.on_insert_subs.send_update(value);
            true
        } else {
            false
        }
    }

    /// Removes the `value` from this [`HashSet`] and returns it, if any.
    ///
    /// This will produce [`HashSet::on_remove()`] event.
    pub fn remove(&mut self, value: &T) -> Option<T> {
        let value = self.store.take(value);

        if let Some(value) = &value {
            self.on_remove_subs.send_update(value.clone());
        }

        value
    }

    /// Makes this [`HashSet`] exactly the same as the `updated` one.
    ///
    /// It will calculate a diff between this [`HashSet`] and the `updated`, and
    /// will spawn [`HashSet::on_insert()`] and [`HashSet::on_remove()`] if the
    /// diff is not empty.
    ///
    /// For the usage example you can read [`HashSet`] docs.
    pub fn update(&mut self, updated: std::collections::HashSet<T>) {
        let removed_elems = self.store.difference(&updated);
        let inserted_elems = updated.difference(&self.store);

        for removed_elem in removed_elems {
            self.on_remove_subs.send_update(removed_elem.clone());
        }

        for inserted_elem in inserted_elems {
            self.on_insert_subs.send_update(inserted_elem.clone());
        }

        self.store = updated;
    }

    /// Indicates whether this [`HashSet`] contains the `value`.
    #[inline]
    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.store.contains(value)
    }
}

impl<T, S, O> Default for HashSet<T, S, O>
where
    S: SubscribersStore<T, O>,
{
    #[inline]
    fn default() -> Self {
        Self {
            store: std::collections::HashSet::new(),
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
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
        self.store.iter()
    }
}

impl<T, S, O> Drop for HashSet<T, S, O>
where
    S: SubscribersStore<T, O>,
{
    /// Sends all values of a dropped [`HashSet`] to the
    /// [`HashSet::on_remove()`] subscriptions.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
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
            store: from,
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}
