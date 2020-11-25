//! Reactive hash set based on [`HashSet`].

use std::{
    collections::{hash_set::Iter, HashSet},
    hash::Hash,
    marker::PhantomData,
};

use futures::Stream;

use crate::subscribers_store::SubscribersStore;

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
/// // When you update ObservableHashSet by another HashSet all events will
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

#[derive(Debug)]
pub struct ObservableHashSet<
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
    O,
> {
    /// Data stored by this [`ObservableHashSet`].
    store: HashSet<T>,

    on_insert_subs: S,

    on_remove_subs: S,

    _output: PhantomData<O>,
}

impl<T, S, O> ObservableHashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    /// Returns new empty [`ObservableHashSet`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a value to the set.
    ///
    /// If the set did not have this value present, `true` is returned.
    ///
    /// If the set did have this value present, `false` is returned.
    ///
    /// This will produce [`ObservableHashSet::on_insert`] event.
    pub fn insert(&mut self, value: T) -> bool {
        if self.store.insert(value.clone()) {
            self.on_insert_subs.send_update(value);
            true
        } else {
            false
        }
    }

    /// Removes a value from the set. Returns whether the value was present in
    /// the set.
    ///
    /// This will produce [`ObservableHashSet::on_remove`] event.
    pub fn remove(&mut self, value: &T) -> Option<T> {
        let value = self.store.take(value);

        if let Some(value) = &value {
            self.on_remove_subs.send_update(value.clone());
        }

        value
    }

    /// An iterator visiting all elements in arbitrary order. The iterator
    /// element type is `&'a T`.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    /// Returns the [`Stream`] to which the inserted values will be
    /// sent.
    ///
    /// Also to this [`Stream`] will be sent all already inserted values
    /// of this [`ObservableHashSet`].
    pub fn on_insert(&self) -> impl Stream<Item = O> {
        self.on_insert_subs
            .new_subscription(self.store.iter().cloned().collect())
    }

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableHashSet`] on drop.
    pub fn on_remove(&self) -> impl Stream<Item = O> {
        self.on_remove_subs.new_subscription(Vec::new())
    }

    /// Makes this [`ObservableHashSet`] exactly the same as the passed
    /// [`HashSet`].
    ///
    /// This function will calculate diff between [`ObservableHashSet`] and
    /// provided [`HashSet`] and will spawn [`ObservableHashSet::on_insert`]
    /// and [`ObservableHashSet::on_remove`] if set is changed.
    ///
    /// For the usage example you can read [`ObservableHashSet`] doc.
    pub fn update(&mut self, updated: HashSet<T>) {
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

    /// Returns `true` if the set contains a value.
    pub fn contains(&self, value: &T) -> bool {
        self.store.contains(value)
    }
}

impl<T, S, O> Default for ObservableHashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    fn default() -> Self {
        Self {
            store: HashSet::new(),
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}

impl<'a, T: Clone + Eq + Hash, S: SubscribersStore<T, O>, O> IntoIterator
    for &'a ObservableHashSet<T, S, O>
{
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<T, S, O> Drop for ObservableHashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    /// Sends all values of a dropped [`ObservableHashSet`] to the
    /// [`ObservableHashSet::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain().for_each(|value| {
            on_remove_subs.send_update(value);
        });
    }
}

impl<T, S, O> From<HashSet<T>> for ObservableHashSet<T, S, O>
where
    T: Clone + Hash + Eq + 'static,
    S: SubscribersStore<T, O>,
{
    fn from(from: HashSet<T>) -> Self {
        Self {
            store: from,
            on_insert_subs: S::default(),
            on_remove_subs: S::default(),
            _output: PhantomData::default(),
        }
    }
}
