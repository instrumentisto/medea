//! Implementation of the reactive [`HashSet`] data structure based on
//! [`std::collections`].

use std::{
    cell::RefCell,
    collections::{hash_set::Iter, HashSet},
    hash::Hash,
};

use futures::{channel::mpsc, Stream};

/// Reactive [`HashSet`] data structure based on [`std::collections`].
///
/// # Basic usage
///
/// ```rust
/// use medea_reactive::collections::ObservableHashSet;
///
/// let mut set = ObservableHashSet::new();
///
/// // You can just insert items as well as with standard HashMap.
/// set.insert("foo".to_string());
/// // ...or iterate over items.
/// assert_eq!(set.iter().next().unwrap(), &"foo".to_string());
/// // ...and finally remove them.
/// set.remove(&"foo".to_string());
/// ```
///
/// # Subscriptions to the changes
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
/// let mut set_insert_subscription = set.on_insert();
///
/// set.insert("foo".to_string());
///
/// let inserted_item = set_insert_subscription.next()
///     .await
///     .unwrap();
/// assert_eq!(inserted_item, "foo".to_string());
///
/// // Also you can subscribe on remove action:
/// let mut set_remove_subscription = set.on_remove();
///
/// set.remove(&"foo".to_string());
///
/// let removed_item = set_remove_subscription.next()
///     .await
///     .unwrap();
/// assert_eq!(removed_item, "foo".to_string());
///
/// // When you update ObservableHashSet by another HashSet all events
/// // will work fine:
/// set.insert("foo-1".to_string());
/// set.insert("foo-2".to_string());
/// set.insert("foo-3".to_string());
///
/// let mut set_for_update = HashSet::new();
/// set_for_update.insert("foo-1".to_string());
/// set_for_update.insert("foo-4".to_string());
/// set.update(set_for_update);
///
/// let removed_items: HashSet<String> = set_remove_subscription.take(2)
///     .collect()
///     .await;
/// let inserted_item = set_insert_subscription.skip(3)
///     .next()
///     .await
///     .unwrap();
/// assert!(removed_items.contains(&"foo-2".to_string()));
/// assert!(removed_items.contains(&"foo-3".to_string()));
/// assert_eq!(inserted_item, "foo-4".to_string());
/// assert!(set.contains(&"foo-1".to_string()));
/// assert!(set.contains(&"foo-4".to_string()));
/// # });
/// ```
#[derive(Debug)]
pub struct ObservableHashSet<T: Clone + Hash + Eq> {
    /// Data stored by this [`ObservableHashSet`].
    store: HashSet<T>,

    /// Subscribers of the [`ObservableHashSet::on_insert`] method.
    on_insert_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,

    /// Subscribers of the [`ObservableHashSet::on_remove`] method.
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
}

impl<T> ObservableHashSet<T>
where
    T: Clone + Hash + Eq,
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
        for sub in self.on_insert_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        self.store.insert(value)
    }

    /// Removes a value from the set. Returns whether the value was present in
    /// the set.
    ///
    /// This will produce [`ObservableHashSet::on_remove`] event.
    pub fn remove(&mut self, index: &T) -> Option<T> {
        let value = self.store.take(index);
        if let Some(value) = &value {
            for sub in self.on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send(value.clone());
            }
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
    pub fn on_insert(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();

        for value in self.store.iter().cloned() {
            let _ = tx.unbounded_send(value);
        }

        self.on_insert_subs.borrow_mut().push(tx);

        rx
    }

    /// Returns the [`Stream`] to which the removed values will be
    /// sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableHashSet`] on drop.
    pub fn on_remove(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
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
            for remove_sub in self.on_remove_subs.borrow().iter() {
                let _ = remove_sub.unbounded_send(removed_elem.clone());
            }
        }

        for inserted_elem in inserted_elems {
            for insert_sub in self.on_insert_subs.borrow().iter() {
                let _ = insert_sub.unbounded_send(inserted_elem.clone());
            }
        }

        self.store = updated;
    }

    /// Returns `true` if the set contains a value.
    pub fn contains(&self, value: &T) -> bool {
        self.store.contains(value)
    }
}

impl<T> Default for ObservableHashSet<T>
where
    T: Clone + Hash + Eq,
{
    fn default() -> Self {
        Self {
            store: HashSet::new(),
            on_insert_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }
}

impl<'a, T: Clone + Eq + Hash> IntoIterator for &'a ObservableHashSet<T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<T> Drop for ObservableHashSet<T>
where
    T: Clone + Hash + Eq,
{
    /// Sends all values of a dropped [`ObservableHashSet`] to the
    /// [`ObservableHashSet::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain().for_each(|value| {
            for sub in on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send(value.clone());
            }
        });
    }
}

impl<T> From<HashSet<T>> for ObservableHashSet<T>
where
    T: Clone + Hash + Eq,
{
    fn from(from: HashSet<T>) -> Self {
        Self {
            store: from,
            on_insert_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }
}
