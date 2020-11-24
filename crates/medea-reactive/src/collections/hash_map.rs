//! Reactive hash map based on [`HashMap`].

use std::{
    cell::RefCell,
    collections::{
        hash_map::{Iter, Values},
        HashMap,
    },
    hash::Hash,
    marker::PhantomData,
};

use futures::{channel::mpsc, future::LocalBoxFuture, Stream};

use crate::{
    collections::subscribers_store::{ProgressableSubStore, SubscribersStore},
    ProgressableObservableValue,
};

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
#[derive(Debug, Clone)]
pub struct ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    /// Data stored by this [`ObservableHashMap`].
    store: HashMap<K, V>,

    /// Subscribers of the [`ObservableHashMap::on_insert`] method.
    on_insert_subs: S,

    /// Subscribers of the [`ObservableHashMap::on_remove`] method.
    on_remove_subs: S,

    _output: PhantomData<O>,
}

impl<K, V>
    ObservableHashMap<
        K,
        V,
        ProgressableSubStore<(K, V)>,
        ProgressableObservableValue<(K, V)>,
    >
where
    K: Hash + Eq + Clone + 'static,
    V: Clone + 'static,
{
    pub fn when_insert_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_insert_subs.when_all_processed()
    }

    pub fn when_remove_completed(&self) -> LocalBoxFuture<'static, ()> {
        self.on_remove_subs.when_all_processed()
    }
}

impl<K, V, S, O> ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    /// Returns new empty [`ObservableHashMap`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a key-value pair into the [`ObservableHashMap`].
    ///
    /// This action will produce [`ObservableHashMap::on_insert`] event.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.on_insert_subs.send((key.clone(), value.clone()));

        self.store.insert(key, value)
    }

    /// Removes a key from the [`ObservableHashMap`], returning the value at
    /// the key if the key was previously in the [`ObservableHashMap`].
    ///
    /// This action will produce [`ObservableHashMap::on_remove`] event.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed_item = self.store.remove(key);
        if let Some(item) = &removed_item {
            self.on_remove_subs.send((key.clone(), item.clone()));
        }

        removed_item
    }

    /// Returns the [`Stream`] to which the inserted key-value pairs will be
    /// sent.
    ///
    /// Also to this [`Stream`] will be sent all already inserted key-value
    /// pairs of this [`ObservableHashMap`].
    pub fn on_insert(&self) -> impl Stream<Item = O> {
        self.on_insert_subs.subscribe(
            self.store
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        )
    }

    /// Returns the [`Stream`] to which the removed key-value pairs will be
    /// sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableHashMap`] on drop.
    pub fn on_remove(&self) -> impl Stream<Item = O> {
        self.on_remove_subs.subscribe(Vec::new())
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.store.get(key)
    }

    /// Returns a mutable reference to the value corresponding to the key.
    ///
    /// Note that mutating of the returned value wouldn't work same as
    /// [`Observable`]s and doesn't spawns [`ObservableHashMap::on_insert`] or
    /// [`ObservableHashMap::on_remove`] events. If you need subscriptions on
    /// value changes then just wrap value to the [`Observable`] and subscribe
    /// to it.
    ///
    /// [`Observable`]: crate::Observable
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.store.get_mut(key)
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The
    /// iterator element type is `(&'a K, &'a V)`.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
    }

    /// An iterator visiting all values in arbitrary order. The iterator element
    /// type is `&'a V`.
    pub fn values(&self) -> Values<'_, K, V> {
        self.store.values()
    }
}

impl<K, V, S, O> Default for ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    fn default() -> Self {
        Self {
            store: HashMap::new(),
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<K, V, S, O> From<HashMap<K, V>> for ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    fn from(from: HashMap<K, V>) -> Self {
        Self {
            store: from,
            on_remove_subs: S::default(),
            on_insert_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, K, V, S, O> IntoIterator for &'a ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<K, V, S, O> Drop for ObservableHashMap<K, V, S, O>
where
    K: Hash + Eq + Clone,
    V: Clone,
    S: SubscribersStore<(K, V), O>,
{
    /// Sends all key-values of a dropped [`ObservableHashMap`] to the
    /// [`ObservableHashMap::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain().for_each(|(key, value)| {
            on_remove_subs.send((key, value));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod progressable {
        use std::time::Duration;

        use futures::StreamExt as _;
        use tokio::time::timeout;

        use crate::collections::ProgressableHashMap;

        use super::*;

        mod when_remove_completed {
            use super::*;

            #[tokio::test]
            async fn waits_for_processing() {
                let mut store = ProgressableHashMap::new();
                store.insert(0, 0);

                let mut on_remove = store.on_remove();
                store.remove(&0).unwrap();

                let mut when_remove_completed = store.when_remove_completed();

                timeout(Duration::from_millis(500), when_remove_completed)
                    .await
                    .unwrap_err();
            }

            #[tokio::test]
            async fn waits_for_value_drop() {
                let mut store = ProgressableHashMap::new();
                store.insert(0, 0);

                let mut on_remove = store.on_remove();
                store.remove(&0);
                let mut when_remove_completed = store.when_remove_completed();
                let value = on_remove.next().await.unwrap();

                timeout(Duration::from_millis(500), when_remove_completed)
                    .await
                    .unwrap_err();
            }

            #[tokio::test]
            async fn resolved_on_value_drop() {
                let mut store = ProgressableHashMap::new();
                store.insert(0, 0);

                let mut on_remove = store.on_remove();
                store.remove(&0).unwrap();
                let mut when_remove_completed = store.when_remove_completed();
                drop(on_remove.next().await.unwrap());

                timeout(Duration::from_millis(500), when_remove_completed)
                    .await
                    .unwrap();
            }

            #[tokio::test]
            async fn resolves_on_empty_sublist() {
                let mut store = ProgressableHashMap::new();
                store.insert(0, 0);

                store.remove(&0).unwrap();
                let mut when_remove_completed = store.when_remove_completed();

                timeout(Duration::from_millis(50), when_remove_completed).await.unwrap();
            }

            #[tokio::test]
            async fn waits_for_two_subs() {
                let mut store = ProgressableHashMap::new();
                store.insert(0, 0);

                let mut first_on_remove = store.on_remove();
                let mut second_on_remove = store.on_remove();
                store.remove(&0).unwrap();
                let mut when_all_remove_processed = store.when_remove_completed();

                drop(first_on_remove.next().await.unwrap());

                timeout(Duration::from_millis(500), when_all_remove_processed).await.unwrap_err();
            }
        }

        mod when_insert_completed {
            use super::*;

            #[tokio::test]
            async fn waits_for_processing() {
                let mut store = ProgressableHashMap::new();

                let mut on_insert = store.on_insert();
                store.insert(0, 0);

                let mut when_insert_completed = store.when_insert_completed();

                timeout(Duration::from_millis(500), when_insert_completed)
                    .await
                    .unwrap_err();
            }

            #[tokio::test]
            async fn waits_for_value_drop() {
                let mut store = ProgressableHashMap::new();

                let mut on_insert = store.on_insert();
                store.insert(0, 0);
                let mut when_insert_completed = store.when_insert_completed();
                let value = on_insert.next().await.unwrap();

                timeout(Duration::from_millis(500), when_insert_completed)
                    .await
                    .unwrap_err();
            }

            #[tokio::test]
            async fn resolved_on_value_drop() {
                let mut store = ProgressableHashMap::new();

                let mut on_insert = store.on_insert();
                store.insert(0, 0);
                let mut when_insert_completed = store.when_insert_completed();
                drop(on_insert.next().await.unwrap());

                timeout(Duration::from_millis(500), when_insert_completed)
                    .await
                    .unwrap();
            }

            #[tokio::test]
            async fn resolves_on_empty_sublist() {
                let mut store = ProgressableHashMap::new();

                store.insert(0, 0);
                let mut when_insert_completed = store.when_insert_completed();

                timeout(Duration::from_millis(50), when_insert_completed).await.unwrap();
            }

            #[tokio::test]
            async fn waits_for_two_subs() {
                let mut store = ProgressableHashMap::new();

                let mut first_on_insert = store.on_insert();
                let mut second_on_insert = store.on_insert();
                store.insert(0, 0);
                let mut when_all_insert_processed = store.when_insert_completed();

                drop(first_on_insert.next().await.unwrap());

                timeout(Duration::from_millis(500), when_all_insert_processed).await.unwrap_err();
            }
        }
    }
}
