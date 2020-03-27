use std::{cell::RefCell, rc::Rc, slice::Iter, vec::IntoIter};

use futures::{channel::mpsc, Stream};

#[derive(Debug)]
pub struct ObservableVec<T: Clone> {
    store: Vec<T>,
    on_push_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
}

impl<T> ObservableVec<T>
where
    T: Clone,
{
    pub fn new() -> Self {
        Self {
            store: Vec::new(),
            on_push_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }

    pub fn push(&mut self, value: T) {
        for sub in self.on_push_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        self.store.push(value)
    }

    pub fn remove(&mut self, index: usize) -> T {
        let value = self.store.remove(index);
        for sub in self.on_remove_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        value
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    pub fn on_push(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();

        for value in self.store.iter().cloned() {
            let _ = tx.unbounded_send(value);
        }

        self.on_push_subs.borrow_mut().push(tx);

        rx
    }

    pub fn on_remove(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
    }
}

impl<T: Clone> From<Vec<T>> for ObservableVec<T> {
    fn from(from: Vec<T>) -> Self {
        Self {
            store: from,
            on_push_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
    }
}

impl<'a, T: Clone> IntoIterator for &'a ObservableVec<T> {
    type IntoIter = Iter<'a, T>;
    type Item = &'a T;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<T> Drop for ObservableVec<T>
where
    T: Clone,
{
    fn drop(&mut self) {
        let mut store = &mut self.store;
        let on_remove_subs = &self.on_remove_subs;
        store.drain(..).for_each(|value| {
            for sub in on_remove_subs.borrow().iter() {
                let _ = sub.unbounded_send(value.clone());
            }
        });
    }
}

impl<T: Clone> AsRef<[T]> for ObservableVec<T> {
    fn as_ref(&self) -> &[T] {
        &self.store
    }
}
