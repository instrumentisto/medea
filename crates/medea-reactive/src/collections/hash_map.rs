use std::{
    cell::RefCell,
    collections::{hash_map::Iter, HashMap},
    hash::Hash,
};

use futures::{channel::mpsc, Stream};

#[derive(Debug, Clone)]
pub struct ObservableHashMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    store: HashMap<K, V>,
    on_insert_subs: RefCell<Vec<mpsc::UnboundedSender<(K, V)>>>,
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<(K, V)>>>,
}

impl<K, V> ObservableHashMap<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            on_insert_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        for sub in self.on_insert_subs.borrow().iter() {
            let _ = sub.unbounded_send((key.clone(), value.clone()));
        }
        let out = self.store.insert(key, value);

        out
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let removed_item = self.store.remove(key);
        if let Some(item) = &removed_item {
            for sub in self.on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send((key.clone(), item.clone()));
            }
        }

        removed_item
    }

    pub fn on_insert(&self) -> impl Stream<Item = (K, V)> {
        let (tx, rx) = mpsc::unbounded();

        for (key, value) in &self.store {
            let _ = tx.unbounded_send((key.clone(), value.clone()));
        }
        self.on_insert_subs.borrow_mut().push(tx);

        rx
    }

    pub fn on_remove(&self) -> impl Stream<Item = (K, V)> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.store.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.store.get_mut(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
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
