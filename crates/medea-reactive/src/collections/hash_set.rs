use std::{
    cell::RefCell,
    collections::{hash_set::Iter, HashSet},
};

use futures::{channel::mpsc, Stream};
use std::{collections::hash_map::RandomState, hash::Hash};

#[derive(Debug)]
pub struct ObservableHashSet<T: Clone + Hash + Eq> {
    store: HashSet<T>,
    on_insert_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
}

impl<T> ObservableHashSet<T>
where
    T: Clone + Hash + Eq,
{
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, value: T) -> bool {
        for sub in self.on_insert_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        self.store.insert(value)
    }

    pub fn remove(&mut self, index: &T) -> Option<T> {
        let value = self.store.take(index);
        if let Some(value) = &value {
            for sub in self.on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send(value.clone());
            }
        }

        value
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    pub fn on_insert(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();

        for value in self.store.iter().cloned() {
            let _ = tx.unbounded_send(value);
        }

        self.on_insert_subs.borrow_mut().push(tx);

        rx
    }

    pub fn on_remove(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
    }

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
    fn from(from: HashSet<T, RandomState>) -> Self {
        Self {
            store: from,
            on_insert_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }
}
