use std::{
    cell::RefCell,
    collections::{hash_set::Iter, HashSet},
    hash::Hash,
};
use std::iter;
use std::marker::PhantomData;

use futures::{channel::mpsc, Stream};
use futures::stream::LocalBoxStream;

use crate::{progressable::ProgressableManager, ProgressableObservableValue};

pub trait SubscribersStore<T, O>: Default {
    fn send(&self, value: T);

    fn subscribe(&self, initial_values: Vec<T>) -> LocalBoxStream<'static, O>;
}

#[derive(Debug)]
pub struct BasicSubStore<T>(RefCell<Vec<mpsc::UnboundedSender<T>>>);

impl<T> Default for BasicSubStore<T> {
    fn default() -> Self {
        Self(RefCell::new(Vec::new()))
    }
}

impl<T> SubscribersStore<T, T> for BasicSubStore<T> where T: Clone + 'static {
    fn send(&self, value: T) {
        self.0.borrow_mut().retain(|sub| {
            sub.unbounded_send(value.clone()).is_ok()
        });
    }

    fn subscribe(&self, initial_values: Vec<T>) -> LocalBoxStream<'static, T> {
        let (tx, rx) = mpsc::unbounded();
        initial_values.into_iter()
            .for_each(|value| {
                let _ = tx.unbounded_send(value);
            });

        Box::pin(rx)
    }
}

#[derive(Debug)]
pub struct ProgressableSubStore<T> {
    store: RefCell<Vec<mpsc::UnboundedSender<ProgressableObservableValue<T>>>>,
    manager: ProgressableManager,
}

impl<T> Default for ProgressableSubStore<T> {
    fn default() -> Self {
        Self {
            store: RefCell::new(Vec::new()),
            manager: ProgressableManager::new(),
        }
    }
}

impl<T> SubscribersStore<T, ProgressableObservableValue<T>> for ProgressableSubStore<T>
    where
        T: Clone + 'static,
{
    fn send(&self, value: T) {
        self.store.borrow_mut().retain(|sub| {
            self.manager.incr_processors_count(1);
            let value = self.manager.new_value(value.clone());

            sub.unbounded_send(value).is_ok()
        });
    }

    fn subscribe(
        &self,
        initial_values: Vec<T>,
    ) -> LocalBoxStream<'static, ProgressableObservableValue<T>> {
        let (tx, rx) = mpsc::unbounded();

        initial_values.into_iter().for_each(|value| {
            self.manager.incr_processors_count(1);
            let value = self.manager.new_value(value);
            let _ = tx.unbounded_send(value);
        });

        self.store.borrow_mut().push(tx);

        Box::pin(rx)
    }
}
