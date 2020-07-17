//! Reactive optional type based on [`Option`].

use std::cell::RefCell;

use futures::{channel::oneshot, future::LocalBoxFuture};

/// Reactive optional type based on [`Option`].
///
/// # Usage
/// ```rust
/// # use medea_reactive::ObservableOption;
/// # use futures::{executor};
///
/// # executor::block_on(async {
/// let mut foo = ObservableOption::none();
///
/// let when_foo_will_be_some = foo.when_some();
///
/// foo.replace(1i32);
///
/// assert_eq!(when_foo_will_be_some.await.unwrap(), 1);
///
/// foo.replace(2);
///
/// // This future wouldn't be resolved, because Option
/// // transited from 'Some' to 'Some'.
/// //
/// // foo.when_some().await;
/// # })
/// ```
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct ObservableOption<T: Clone> {
    storage: Option<T>,
    subscribers: RefCell<Vec<oneshot::Sender<T>>>,
}

impl<T> ObservableOption<T>
where
    T: Clone + 'static,
{
    /// Returns observable `Some(T)` analog.
    pub fn new(value: T) -> Self {
        Self {
            storage: Some(value),
            subscribers: RefCell::default(),
        }
    }

    /// Returns observable `None` analog.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Replaces the actual value in the option by the value given in parameter,
    /// returning the old value if present,
    /// leaving a [`Some`] in its place without deinitializing either one.
    ///
    /// If [`ObservableOption`] was `None` before, then
    /// [`ObservableOption::when_some`] would be resolved.
    ///
    /// If [`ObservableOption`] was `Some` before, then
    /// [`ObservableOption::when_some`] wouldn't be resolved.
    pub fn replace(&mut self, value: T) -> Option<T> {
        let is_none_before = self.storage.is_none();

        let old_value = self.storage.replace(value.clone());

        if is_none_before {
            for subscriber in self.subscribers.borrow_mut().drain(..) {
                let _ = subscriber.send(value.clone());
            }
        }

        old_value
    }

    /// Takes the value out of the option, leaving a `None` in its place.
    pub fn take(&mut self) -> Option<T> {
        self.storage.take()
    }

    /// Converts from `&ObservableOption<T>` to `Option<&T>`.
    pub fn as_ref(&self) -> Option<&T> {
        self.storage.as_ref()
    }

    /// Returns [`Future`] which will be resolved when [`ObservableOption`] will
    /// be `Some`.
    ///
    /// Also, returned [`Future`] will be resolved instantly if value is
    /// currently `Some`.
    pub fn when_some(
        &self,
    ) -> LocalBoxFuture<'static, Result<T, oneshot::Canceled>> {
        if let Some(value) = self.storage.as_ref().map(Clone::clone) {
            Box::pin(async move { Ok(value) })
        } else {
            let (tx, rx) = oneshot::channel();
            self.subscribers.borrow_mut().push(tx);

            Box::pin(rx)
        }
    }

    /// Returns `true` if the option is a `None` value.
    pub fn is_none(&self) -> bool {
        self.storage.is_none()
    }

    /// Returns `true` if the option is a `Some` value.
    pub fn is_some(&self) -> bool {
        self.storage.is_some()
    }

    /// Inserts a value computed from `f` into the option if it is [`None`],
    /// then returns a mutable reference to the contained value.
    ///
    /// If [`ObservableOption`] was `None` before, then
    /// [`ObservableOption::when_some`] would be resolved.
    ///
    /// If [`ObservableOption`] was `Some` before, then
    /// [`ObservableOption::when_some`] wouldn't be resolved.
    pub fn get_or_insert_with<F: FnOnce() -> T>(&mut self, f: F) -> &mut T {
        let is_none_before = self.storage.is_none();

        let out = self.storage.get_or_insert_with(f);

        if is_none_before {
            for subscriber in self.subscribers.borrow_mut().drain(..) {
                let _ = subscriber.send(out.clone());
            }
        }

        out
    }
}

impl<T> Default for ObservableOption<T> where T: Clone {
    fn default() -> Self {
        Self {
            storage: None,
            subscribers: RefCell::default(),
        }
    }
}
