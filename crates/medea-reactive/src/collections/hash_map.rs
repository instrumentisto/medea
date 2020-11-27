//! Reactive hash map based on [`HashMap`].

use std::{
    collections::hash_map::{Iter, Values},
    hash::Hash,
    marker::PhantomData,
};

use futures::{future::LocalBoxFuture, Stream};

use crate::subscribers_store::{common, progressable, SubscribersStore};

/// Reactive hash map based on [`HashMap`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashMap<K, V> =
    HashMap<K, V, progressable::SubStore<(K, V)>, progressable::Value<(K, V)>>;
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
/// # Usage of when all completed functions
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
/// // hash_map.when_insert_completed().await; <- wouldn't be resolved
/// let value = on_insert.next().await.unwrap();
/// // hash_map.when_insert_completed().await; <- wouldn't be resolved
/// drop(value);
///
/// hash_map.when_insert_completed().await; // will be resolved
/// # });
/// ```
#[derive(Debug, Clone)]
pub struct HashMap<K, V, S: SubscribersStore<(K, V), O>, O> {
    /// Data stored by this [`HashMap`].
    store: std::collections::HashMap<K, V>,

    /// Subscribers of the [`HashMap::on_insert`] method.
    on_insert_subs: S,

    /// Subscribers of the [`HashMap::on_remove`] method.
    on_remove_subs: S,

    /// Phantom type of [`HashMap::on_insert`] and
    /// [`HashMap::on_remove`] output.
    _output: PhantomData<O>,
}

impl<K, V> ProgressableHashMap<K, V>
where
    K: Hash + Eq + Clone + 'static,
    V: Clone + 'static,
{
    /// Returns [`Future`] which will be resolved when all insertion updates
    /// will be processed by [`HashMap::on_insert`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_insert_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_insert_subs.when_all_processed()
    }

    /// Returns [`Future`] which will be resolved when all remove updates will
    /// be processed by [`HashMap::on_remove`] subscribers.
    ///
    /// [`Future`]: std::future::Future
    #[inline]
    pub fn when_remove_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_remove_subs.when_all_processed()
    }
}

impl<K, V, S: SubscribersStore<(K, V), O>, O> HashMap<K, V, S, O> {
    /// Returns new empty [`HashMap`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The
    /// iterator element type is `(&'a K, &'a V)`.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
    }

    /// An iterator visiting all values in arbitrary order. The iterator element
    /// type is `&'a V`.
    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        self.store.values()
    }

    /// Returns the [`Stream`] to which the removed key-value pairs will be
    /// sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`HashMap`] on drop.
    #[inline]
    pub fn on_remove(&self) -> impl Stream<Item = O> {
        self.on_remove_subs.new_subscription(Vec::new())
    }
}

impl<K, V, S, O> HashMap<K, V, S, O>
where
    K: Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    /// Returns the [`Stream`] to which the inserted key-value pairs will be
    /// sent.
    ///
    /// Also to this [`Stream`] will be sent all already inserted key-value
    /// pairs of this [`HashMap`].
    #[inline]
    pub fn on_insert(&self) -> impl Stream<Item = O> {
        self.on_insert_subs.new_subscription(
            self.store
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }
}

impl<K, V, S, O> HashMap<K, V, S, O>
where
    K: Hash + Eq,
    S: SubscribersStore<(K, V), O>,
{
    /// Returns a reference to the value corresponding to the key.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.store.get(key)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// Note that mutating of the returned value wouldn't work same as
    /// [`Observable`]s and doesn't spawns [`HashMap::on_insert`] or
    /// [`HashMap::on_remove`] events. If you need subscriptions on
    /// value changes then just wrap value to the [`Observable`] and subscribe
    /// to it.
    ///
    /// [`Observable`]: crate::Observable
    #[inline]
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
    /// Inserts a key-value pair into the [`HashMap`].
    ///
    /// This action will produce [`HashMap::on_insert`] event.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.on_insert_subs
            .send_update((key.clone(), value.clone()));

        self.store.insert(key, value)
    }

    /// Removes a key from the [`HashMap`], returning the value at
    /// the key if the key was previously in the [`HashMap`].
    ///
    /// This action will produce [`HashMap::on_remove`] event.
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
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain().for_each(|(key, value)| {
            on_remove_subs.send_update((key, value));
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::StreamExt as _;
    use tokio::time::timeout;

    use crate::collections::ProgressableHashMap;

    mod when_remove_completed {
        use super::*;

        #[tokio::test]
        async fn waits_for_processing() {
            let mut store = ProgressableHashMap::new();
            let _ = store.insert(0, 0);

            let _on_remove = store.on_remove();
            let _ = store.remove(&0).unwrap();

            let when_remove_completed = store.when_remove_completed();

            let _ = timeout(Duration::from_millis(500), when_remove_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn waits_for_value_drop() {
            let mut store = ProgressableHashMap::new();
            let _ = store.insert(0, 0);

            let mut on_remove = store.on_remove();
            let _ = store.remove(&0);
            let when_remove_completed = store.when_remove_completed();
            let _value = on_remove.next().await.unwrap();

            let _ = timeout(Duration::from_millis(500), when_remove_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn resolved_on_value_drop() {
            let mut store = ProgressableHashMap::new();
            let _ = store.insert(0, 0);

            let mut on_remove = store.on_remove();
            let _ = store.remove(&0).unwrap();
            let when_remove_completed = store.when_remove_completed();
            drop(on_remove.next().await.unwrap());

            timeout(Duration::from_millis(500), when_remove_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn resolves_on_empty_sublist() {
            let mut store = ProgressableHashMap::new();
            let _ = store.insert(0, 0);

            let _ = store.remove(&0).unwrap();
            let when_remove_completed = store.when_remove_completed();

            timeout(Duration::from_millis(50), when_remove_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn waits_for_two_subs() {
            let mut store = ProgressableHashMap::new();
            let _ = store.insert(0, 0);

            let mut first_on_remove = store.on_remove();
            let _second_on_remove = store.on_remove();
            let _ = store.remove(&0).unwrap();
            let when_all_remove_processed = store.when_remove_completed();

            drop(first_on_remove.next().await.unwrap());

            let _ =
                timeout(Duration::from_millis(500), when_all_remove_processed)
                    .await
                    .unwrap_err();
        }
    }

    mod when_insert_completed {
        use super::*;

        #[tokio::test]
        async fn waits_for_processing() {
            let mut store = ProgressableHashMap::new();

            let _on_insert = store.on_insert();
            let _ = store.insert(0, 0);

            let when_insert_completed = store.when_insert_completed();

            let _ = timeout(Duration::from_millis(500), when_insert_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn waits_for_value_drop() {
            let mut store = ProgressableHashMap::new();

            let mut on_insert = store.on_insert();
            let _ = store.insert(0, 0);
            let when_insert_completed = store.when_insert_completed();
            let _value = on_insert.next().await.unwrap();

            let _ = timeout(Duration::from_millis(500), when_insert_completed)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn resolved_on_value_drop() {
            let mut store = ProgressableHashMap::new();

            let mut on_insert = store.on_insert();
            let _ = store.insert(0, 0);
            let when_insert_completed = store.when_insert_completed();
            drop(on_insert.next().await.unwrap());

            timeout(Duration::from_millis(500), when_insert_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn resolves_on_empty_sublist() {
            let mut store = ProgressableHashMap::new();

            let _ = store.insert(0, 0);
            let when_insert_completed = store.when_insert_completed();

            timeout(Duration::from_millis(50), when_insert_completed)
                .await
                .unwrap();
        }

        #[tokio::test]
        async fn waits_for_two_subs() {
            let mut store = ProgressableHashMap::new();

            let mut first_on_insert = store.on_insert();
            let _second_on_insert = store.on_insert();
            let _ = store.insert(0, 0);
            let when_all_insert_processed = store.when_insert_completed();

            drop(first_on_insert.next().await.unwrap());

            let _ =
                timeout(Duration::from_millis(500), when_all_insert_processed)
                    .await
                    .unwrap_err();
        }
    }
}
