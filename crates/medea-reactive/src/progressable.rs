use std::{cell::RefCell, mem, ops::Deref, rc::Rc};

use futures::{channel::oneshot, future::LocalBoxFuture};

#[derive(Clone, Debug)]
pub(crate) struct ProgressableManager {
    counter: Rc<RefCell<u32>>,
    progress_subs: Rc<RefCell<Vec<oneshot::Sender<()>>>>,
}

impl ProgressableManager {
    pub(crate) fn new() -> Self {
        Self {
            counter: Rc::new(RefCell::new(0)),
            progress_subs: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub(crate) fn incr_processors_count(&self, count: u32) {
        *self.counter.borrow_mut() += count;
    }

    pub(crate) fn new_value<D>(
        &self,
        value: D,
    ) -> ProgressableObservableValue<D> {
        ProgressableObservableValue {
            value,
            counter: Rc::clone(&self.counter),
            progress_subs: Rc::clone(&self.progress_subs),
        }
    }

    pub(crate) fn when_all_processed(&self) -> LocalBoxFuture<'static, ()> {
        if *self.counter.borrow() > 0 {
            let (tx, rx) = oneshot::channel();
            self.progress_subs.borrow_mut().push(tx);

            Box::pin(async move {
                let _ = rx.await;
            })
        } else {
            Box::pin(futures::future::ready(()))
        }
    }
}

#[derive(Debug)]
pub struct ProgressableObservableValue<D> {
    value: D,
    counter: Rc<RefCell<u32>>,
    progress_subs: Rc<RefCell<Vec<oneshot::Sender<()>>>>,
}

impl<D> Drop for ProgressableObservableValue<D> {
    fn drop(&mut self) {
        *self.counter.borrow_mut() -= 1;
        if *self.counter.borrow() == 0 {
            let progress_subs: Vec<_> =
                mem::take(&mut self.progress_subs.borrow_mut());
            progress_subs.into_iter().for_each(|s| {
                let _ = s.send(());
            });
        }
    }
}

impl<D> AsRef<D> for ProgressableObservableValue<D> {
    fn as_ref(&self) -> &D {
        &self.value
    }
}

impl<D> Deref for ProgressableObservableValue<D> {
    type Target = D;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
