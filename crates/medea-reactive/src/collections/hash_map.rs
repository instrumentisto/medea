//! Reactive hash map based on [`HashMap`].

use std::{
    cell::RefCell,
    collections::{
        hash_map::{Iter, Values},
        HashMap,
    },
    hash::Hash,
};

use futures::{channel::mpsc, Stream};

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
pub struct ObservableHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    /// Data stored by this [`ObservableHashMap`].
    store: HashMap<K, V>,

    /// Subscribers of the [`ObservableHashMap::on_insert`] method.
    on_insert_subs: RefCell<Vec<mpsc::UnboundedSender<(K, V)>>>,

    /// Subscribers of the [`ObservableHashMap::on_remove`] method.
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<(K, V)>>>,
}

impl<K, V> ObservableHashMap<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
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
        for sub in self.on_insert_subs.borrow().iter() {
            let _ = sub.unbounded_send((key.clone(), value.clone()));
        }

        self.store.insert(key, value)
    }

    /// Removes a key from the [`ObservableHashMap`], returning the value at
    /// the key if the key was previously in the [`ObservableHashMap`].
    ///
    /// This action will produce [`ObservableHashMap::on_remove`] event.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed_item = self.store.remove(key);
        if let Some(item) = &removed_item {
            for sub in self.on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send((key.clone(), item.clone()));
            }
        }

        removed_item
    }

    /// Returns the [`Stream`] to which the inserted key-value pairs will be
    /// sent.
    ///
    /// Also to this [`Stream`] will be sent all already inserted key-value
    /// pairs of this [`ObservableHashMap`].
    pub fn on_insert(&self) -> impl Stream<Item = (K, V)> {
        let (tx, rx) = mpsc::unbounded();

        for (key, value) in &self.store {
            let _ = tx.unbounded_send((key.clone(), value.clone()));
        }
        self.on_insert_subs.borrow_mut().push(tx);

        rx
    }

    /// Returns the [`Stream`] to which the removed key-value pairs will be
    /// sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableHashMap`] on drop.
    pub fn on_remove(&self) -> impl Stream<Item = (K, V)> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
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

impl<K, V> Default for ObservableHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self {
            store: HashMap::new(),
            on_insert_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }
}

impl<K, V> From<HashMap<K, V>> for ObservableHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn from(from: HashMap<K, V>) -> Self {
        Self {
            store: from,
            on_remove_subs: RefCell::new(Vec::new()),
            on_insert_subs: RefCell::new(Vec::new()),
        }
    }
}

impl<'a, K: Hash + Eq + Clone, V: Clone> IntoIterator
    for &'a ObservableHashMap<K, V>
{
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<K: Hash + Eq + Clone, V: Clone> Drop for ObservableHashMap<K, V> {
    /// Sends all key-values of a dropped [`ObservableHashMap`] to the
    /// [`ObservableHashMap::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain().for_each(|(key, value)| {
            for sub in on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send((key.clone(), value.clone()));
            }
        });
    }
}
