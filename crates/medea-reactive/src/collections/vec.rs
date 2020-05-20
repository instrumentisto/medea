//! Reactive vector based on [`Vec`].

use std::{cell::RefCell, slice::Iter};

use futures::{channel::mpsc, Stream};

/// Reactive vector based on [`Vec`].
///
/// # Usage
///
/// ```rust
/// # use futures::{executor, StreamExt as _, Stream};
/// use medea_reactive::collections::ObservableVec;
///
/// # executor::block_on(async {
/// let mut vec = ObservableVec::new();
///
/// // You can subscribe on push event:
/// let mut pushes = vec.on_push();
///
/// vec.push("foo");
///
/// let pushed_item = pushes.next().await.unwrap();
/// assert_eq!(pushed_item, "foo");
///
/// // Also you can subscribe on remove event:
/// let mut removals = vec.on_remove();
///
/// vec.remove(0);
///
/// let removed_item = removals.next().await.unwrap();
/// assert_eq!(removed_item, "foo");
///
/// // On Vec structure drop, all items will be sent to the on_remove stream:
/// vec.push("foo-1");
/// vec.push("foo-2");
/// drop(vec);
/// let removed_items: Vec<_> = removals.take(2)
///     .collect()
///     .await;
/// assert_eq!(removed_items[0], "foo-1");
/// assert_eq!(removed_items[1], "foo-2");
/// # });
/// ```
#[derive(Debug)]
pub struct ObservableVec<T: Clone> {
    /// Data stored by this [`ObservableVec`].
    store: Vec<T>,

    /// Subscribers of the [`ObservableVec::on_push`] method.
    on_push_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,

    /// Subscribers of the [`ObservableVec::on_remove`] method.
    on_remove_subs: RefCell<Vec<mpsc::UnboundedSender<T>>>,
}

impl<T> ObservableVec<T>
where
    T: Clone,
{
    /// Returns new empty [`ObservableVec`].
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an element to the back of a collection.
    ///
    /// This will produce [`ObservableVec::on_push`] event.
    pub fn push(&mut self, value: T) {
        for sub in self.on_push_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        self.store.push(value)
    }

    /// Removes and returns the element at position `index` within the vector,
    /// shifting all elements after it to the left.
    ///
    /// This will produce [`ObservableVec::on_remove`] event.
    pub fn remove(&mut self, index: usize) -> T {
        let value = self.store.remove(index);
        for sub in self.on_remove_subs.borrow().iter() {
            let _ = sub.unbounded_send(value.clone());
        }

        value
    }

    /// An iterator visiting all elements in arbitrary order. The iterator
    /// element type is `&'a T`.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.into_iter()
    }

    /// Returns the [`Stream`] to which the pushed values will be sent.
    ///
    /// Also to this [`Stream`] will be sent all already pushed values
    /// of this [`ObservableVec`].
    pub fn on_push(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();

        for value in self.store.iter().cloned() {
            let _ = tx.unbounded_send(value);
        }

        self.on_push_subs.borrow_mut().push(tx);

        rx
    }

    /// Returns the [`Stream`] to which the removed values will be sent.
    ///
    /// Note that to this [`Stream`] will be sent all items of the
    /// [`ObservableVec`] on drop.
    pub fn on_remove(&self) -> impl Stream<Item = T> {
        let (tx, rx) = mpsc::unbounded();
        self.on_remove_subs.borrow_mut().push(tx);

        rx
    }
}

impl<T> Default for ObservableVec<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self {
            store: Vec::new(),
            on_push_subs: RefCell::new(Vec::new()),
            on_remove_subs: RefCell::new(Vec::new()),
        }
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
    /// Sends all items of a dropped [`ObservableVec`] to the
    /// [`ObservableVec::on_remove`] subs.
    fn drop(&mut self) {
        let store = &mut self.store;
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
