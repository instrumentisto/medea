//! Implementation of the wrapper around data which will decrement underlying
//! counter on [`Drop`].

use std::{ops::Deref, rc::Rc};

use crate::ObservableCell;

/// Wrapper around data which will decrement underlying counter on [`Drop`].
#[derive(Debug)]
pub struct Guarded<T> {
    value: T,
    guard: Guard,
}

impl<T> Guarded<T> {
    /// Returns new [`Guarded`] with a provided value. Creates [`Guard`] based
    /// on provided `counter`.
    pub(super) fn new(
        value: T,
        counter: Rc<ObservableCell<u32>>,
    ) -> Guarded<T> {
        Self {
            value,
            guard: Guard::new(counter),
        }
    }

    /// Destructs [`Guarded`] into inner value and its [`Guard`].
    pub fn into_parts(self) -> (T, Guard) {
        (self.value, self.guard)
    }

    /// Destructs [`Guarded`] into inner value dropping  its [`Guard`] in place.
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> AsRef<T> for Guarded<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T> Deref for Guarded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

/// Guard backed by counter that increments on [`Guard`] and decrements in its
/// [`Drop`] implementation.
#[derive(Debug)]
pub struct Guard(Rc<ObservableCell<u32>>);

impl Guard {
    fn new(counter: Rc<ObservableCell<u32>>) -> Self {
        counter.mutate(|mut c| {
            *c = c.checked_add(1).unwrap();
        });
        Self(counter)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.mutate(|mut c| {
            *c = c.checked_sub(1).unwrap();
        });
    }
}
