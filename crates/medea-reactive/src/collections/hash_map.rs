//! Reactive hash map backed by [`HashMap`][1].
//!
//! [1]: std::collections::HashMap

use std::{
    collections::hash_map::{Iter, Values},
    hash::Hash,
    iter::FromIterator,
    marker::PhantomData,
};

use futures::stream::{LocalBoxStream, StreamExt as _};

use crate::subscribers_store::{
    common, progressable,
    progressable::{AllProcessed, Processed},
    SubscribersStore,
};

/// Reactive hash map based on [`HashMap`][1] with additional functionality of
/// tracking progress made by its subscribers. Its [`HashMap::on_insert()`] and
/// [`HashMap::on_remove()`] subscriptions return values wrapped in
/// [`progressable::Guarded`], and implementation tracks all
/// [`progressable::Guard`]s.
///
/// [1]: std::collections::HashMap
pub type ProgressableHashMap<K, V> = HashMap<
    K,
    V,
    progressable::SubStore<(K, V)>,
    progressable::Guarded<(K, V)>,
>;

/// Reactive hash map based on [`HashMap`].
pub type ObservableHashMap<K, V> =
    HashMap<K, V, common::SubStore<(K, V)>, (K, V)>;

/// Reactive hash map based on [`HashMap`].
///
/// # Usage
///
/// ```rust
/// # use std::collections::HashMap;
/// # use futures::{executor, StreamExt as _};
/// use medea_reactive::collections::ObservableHashMap;
///
/// # executor::block_on(async {
/// let mut map = ObservableHashMap::new();
///
/// // You can subscribe on insert action:
/// let mut inserts = map.on_insert();
/// map.insert("foo", "bar");
/// let (key, val) = inserts.next()
///     .await
///     .unwrap();
/// assert_eq!(key, "foo");
/// assert_eq!(val, "bar");
///
/// // Also you can subscribe on remove action:
/// let mut removals = map.on_remove();
/// map.remove(&"foo");
/// let (key, val) = removals.next()
///     .await
///     .unwrap();
/// assert_eq!(key, "foo");
/// assert_eq!(val, "bar");
///
/// // Remove subscription will also receive all items of the HashMap when it
/// // will be dropped:
/// map.insert("foo-1", "bar-1");
/// map.insert("foo-2", "bar-2");
/// drop(map);
/// let removed_items: HashMap<_, _> = removals.take(2)
///     .collect()
///     .await;
/// assert_eq!(removed_items["foo-1"], "bar-1");
/// assert_eq!(removed_items["foo-2"], "bar-2");
/// # });
/// ```
///
/// # Waiting for subscribers to complete
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// use medea_reactive::collections::ProgressableHashMap;
///
/// # executor::block_on(async {
/// let mut hash_map = ProgressableHashMap::new();
///
/// let mut on_insert = hash_map.on_insert();
/// hash_map.insert(1, 1);
///
/// // hash_map.when_insert_processed().await; <- wouldn't be resolved
/// let value = on_insert.next().await.unwrap();
/// // hash_map.when_insert_processed().await; <- wouldn't be resolved
/// drop(value);
///
/// hash_map.when_insert_processed().await; // will be resolved
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct HashMap<K, V, S: SubscribersStore<(K, V), O>, O> {
    /// Data stored by this [`HashMap`].
    store: std::collections::HashMap<K, V>,

    /// Subscribers of the [`HashMap::on_insert()`] method.
    on_insert_subs: S,

    /// Subscribers of the [`HashMap::on_remove()`] method.
    on_remove_subs: S,

    /// Phantom type of [`HashMap::on_insert()`] and [`HashMap::on_remove()`]
    /// output.
    _output: PhantomData<O>,
}

impl<K, V> ProgressableHashMap<K, V>
where
    K: Hash + Eq + Clone + 'static,
    V: Clone + 'static,
{
    /// Returns [`Future`] resolving when all insertion updates will be
    /// processed by [`HashMap::on_insert()`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_insert_processed(&self) -> Processed<'static> {
        self.on_insert_subs.when_all_processed()
    }

    /// Returns [`Future`] resolving when all remove updates will be processed
    /// by [`HashMap::on_remove()`] subscribers.
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

impl<K, V, S: SubscribersStore<(K, V), O>, O> HashMap<K, V, S, O> {
    /// Creates new empty [`HashMap`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// [`Iterator`] visiting all key-value pairs in an arbitrary order.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
    }

    /// [`Iterator`] visiting all values in an arbitrary order.
    #[inline]
    #[must_use]
    pub fn values(&self) -> Values<'_, K, V> {
        self.store.values()
    }

    /// Returns [`Stream`] yielding inserted key-value pairs to this
    /// [`HashMap`].
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    #[must_use]
    pub fn on_insert(&self) -> LocalBoxStream<'static, O> {
        self.on_insert_subs.subscribe()
    }

    /// Returns [`Stream`] yielding removed key-value pairs from this
    /// [`HashMap`].
    ///
    /// Note, that this [`Stream`] will yield all key-value pairs of this
    /// [`HashMap`] on [`Drop`].
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    #[must_use]
    pub fn on_remove(&self) -> LocalBoxStream<'static, O> {
        self.on_remove_subs.subscribe()
    }
}

impl<K, V, S, O> HashMap<K, V, S, O>
where
    K: Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
    O: 'static,
{
    /// Returns [`Stream`] containing values from this [`HashMap`].
    ///
    /// Returned [`Stream`] contains only current values. It won't update on new
    /// inserts, but you can merge returned [`Stream`] with a
    /// [`HashMap::on_insert()`] [`Stream`] if you want to process current
    /// values and values that will be inserted.
    ///
    /// [`Stream`]: futures::Stream
    #[inline]
    pub fn replay_on_insert(&self) -> LocalBoxStream<'static, O> {
        Box::pin(futures::stream::iter(
            self.store
                .iter()
                .map(|(k, v)| self.on_insert_subs.wrap((k.clone(), v.clone())))
                .collect::<Vec<_>>(),
        ))
    }

    /// Chains [`HashMap::replay_on_insert()`] with a [`HashMap::on_insert()`].
    #[inline]
    pub fn on_insert_with_replay(&self) -> LocalBoxStream<'static, O> {
        Box::pin(self.replay_on_insert().chain(self.on_insert()))
    }
}

impl<K, V, S, O> HashMap<K, V, S, O>
where
    K: Hash + Eq,
    S: SubscribersStore<(K, V), O>,
{
    /// Returns a reference to the value corresponding to the `key`.
    #[inline]
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.store.get(key)
    }

    /// Returns a mutable reference to the value corresponding to the `key`.
    ///
    /// Note, that mutating of the returned value wouldn't work same as
    /// [`Observable`]s and doesn't spawns [`HashMap::on_insert()`] or
    /// [`HashMap::on_remove()`] events. If you need subscriptions on value
    /// changes then just wrap the value into an [`Observable`] and subscribe to
    /// it.
    ///
    /// [`Observable`]: crate::Observable
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.store.get_mut(key)
    }
}

impl<K, V, S, O> HashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    /// Removes all entries which are not present in the provided [`HashMap`].
    ///
    /// [`HashMap`]: std::collections::HashMap
    pub fn remove_not_present<A>(
        &mut self,
        other: &std::collections::HashMap<K, A>,
    ) {
        self.iter()
            .filter_map(|(id, _)| {
                if other.contains_key(id) {
                    None
                } else {
                    Some(id.clone())
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(|id| drop(self.remove(&id)));
    }

    /// Inserts a key-value pair to this [`HashMap`].
    ///
    /// Emits [`HashMap::on_insert()`] event and may emit
    /// [`HashMap::on_remove()`] event if insert replaces a value contained in
    /// this [`HashMap`].
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let removed_value = self.store.insert(key.clone(), value.clone());
        if let Some(removed_value) = &removed_value {
            self.on_remove_subs
                .send_update((key.clone(), removed_value.clone()));
        }

        self.on_insert_subs.send_update((key, value));

        removed_value
    }

    /// Removes the `key` from this [`HashMap`], returning the value behind it,
    /// if any.
    ///
    /// Emits [`HashMap::on_remove()`] event if value with provided key is
    /// removed from this [`HashMap`].
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed_item = self.store.remove(key);
        if let Some(item) = &removed_item {
            self.on_remove_subs.send_update((key.clone(), item.clone()));
        }

        removed_item
    }
}

impl<K, V, S: SubscribersStore<(K, V), O>, O> Default for HashMap<K, V, S, O> {
    #[inline]
    fn default() -> Self {
        Self {
            store: std::collections::HashMap::new(),
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<K, V, S: SubscribersStore<(K, V), O>, O>
    From<std::collections::HashMap<K, V>> for HashMap<K, V, S, O>
{
    #[inline]
    fn from(from: std::collections::HashMap<K, V>) -> Self {
        Self {
            store: from,
            on_remove_subs: S::default(),
            on_insert_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, K, V, S: SubscribersStore<(K, V), O>, O> IntoIterator
    for &'a HashMap<K, V, S, O>
{
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<K, V, S: SubscribersStore<(K, V), O>, O> Drop for HashMap<K, V, S, O> {
    /// Sends all key-values of a dropped [`HashMap`] to the
    /// [`HashMap::on_remove`] subs.
    fn drop(&mut self) {
        for (k, v) in self.store.drain() {
            self.on_remove_subs.send_update((k, v));
        }
    }
}

impl<K, V, S: SubscribersStore<(K, V), O>, O> FromIterator<(K, V)>
    for HashMap<K, V, S, O>
where
    K: Hash + Eq,
{
    #[inline]
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self {
            store: std::collections::HashMap::from_iter(iter),
            on_remove_subs: S::default(),
            on_insert_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::{poll, task::Poll, FutureExt as _, StreamExt as _};

    use crate::collections::ProgressableHashMap;

    #[tokio::test]
    async fn replace_triggers_on_remove() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0u32, 0u32);

        let mut on_insert = map.on_insert();
        let mut on_remove = map.on_remove();

        assert_eq!(map.insert(0, 1).unwrap(), 0);

        assert_eq!(*on_insert.next().await.unwrap(), (0, 1));
        assert_eq!(*on_remove.next().await.unwrap(), (0, 0));
    }

    #[tokio::test]
    async fn replay_on_insert() {
        let mut map = ProgressableHashMap::new();

        let _ = map.insert(0, 0);
        let _ = map.insert(1, 2);
        let _ = map.insert(1, 2);
        let _ = map.insert(2, 3);

        let inserts: Vec<_> = map
            .replay_on_insert()
            .map(|val| val.into_inner())
            .collect()
            .await;

        assert_eq!(inserts.len(), 3);
        assert!(inserts.contains(&(0, 0)));
        assert!(inserts.contains(&(1, 2)));
        assert!(inserts.contains(&(2, 3)));
    }

    #[tokio::test]
    async fn when_remove_processed() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0, 0);

        let mut on_remove = map.on_remove();

        assert_eq!(poll!(map.when_remove_processed()), Poll::Ready(()));
        assert_eq!(map.remove(&0), Some(0));
        assert_eq!(poll!(map.when_remove_processed()), Poll::Pending);

        let (val, guard) = on_remove.next().await.unwrap().into_parts();

        assert_eq!(val, (0, 0));
        assert_eq!(poll!(map.when_remove_processed()), Poll::Pending);
        drop(guard);
        assert_eq!(poll!(map.when_remove_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn multiple_when_remove_processed_subs() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0, 0);

        let mut on_remove1 = map.on_remove();
        let mut on_remove2 = map.on_remove();

        assert_eq!(poll!(map.when_remove_processed()), Poll::Ready(()));
        let _ = map.remove(&0).unwrap();
        assert_eq!(poll!(map.when_remove_processed()), Poll::Pending);

        assert_eq!(on_remove1.next().await.unwrap().into_inner(), (0, 0));
        assert_eq!(poll!(map.when_remove_processed()), Poll::Pending);
        assert_eq!(on_remove2.next().await.unwrap().into_inner(), (0, 0));

        assert_eq!(poll!(map.when_remove_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn when_insert_processed() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0, 0);

        let mut on_insert = map.on_insert();

        assert_eq!(poll!(map.when_insert_processed()), Poll::Ready(()));
        let _ = map.insert(2, 3);
        assert_eq!(poll!(map.when_insert_processed()), Poll::Pending);

        let (val, guard) = on_insert.next().await.unwrap().into_parts();

        assert_eq!(val, (2, 3));
        assert_eq!(poll!(map.when_insert_processed()), Poll::Pending);
        drop(guard);
        assert_eq!(poll!(map.when_insert_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn multiple_when_insert_processed_subs() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0, 0);

        let mut on_insert1 = map.on_insert();
        let mut on_insert2 = map.on_insert();

        assert_eq!(poll!(map.when_insert_processed()), Poll::Ready(()));
        let _ = map.insert(0, 0).unwrap();
        assert_eq!(poll!(map.when_insert_processed()), Poll::Pending);

        assert_eq!(on_insert1.next().await.unwrap().into_inner(), (0, 0));
        assert_eq!(poll!(map.when_insert_processed()), Poll::Pending);
        assert_eq!(on_insert2.next().await.unwrap().into_inner(), (0, 0));

        assert_eq!(poll!(map.when_insert_processed()), Poll::Ready(()));
    }

    #[tokio::test]
    async fn on_remove_on_drop() {
        let mut map = ProgressableHashMap::new();
        let _ = map.insert(0, 0);
        let _ = map.insert(1, 1);

        let remove_processed = map.when_remove_processed().shared();
        let on_remove = map.on_remove();

        drop(map);
        let removed: Vec<_> = on_remove.collect().await;

        assert_eq!(poll!(remove_processed.clone()), Poll::Pending);
        let removed: Vec<_> =
            removed.into_iter().map(|v| v.into_inner()).collect();
        assert_eq!(poll!(remove_processed), Poll::Ready(()));

        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&(0, 0)));
        assert!(removed.contains(&(1, 1)));
    }
}
