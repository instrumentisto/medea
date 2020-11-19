//! Implementations of reactive collections based on [`std::collections`].

#![allow(clippy::module_name_repetitions)]

pub mod hash_map;
pub mod hash_set;
pub mod vec;
mod subscribers_store;

use self::hash_set::ObservableHashSet as HashSet;
use self::subscribers_store::ProgressableSubStore;

#[doc(inline)]
pub use {
    hash_map::ObservableHashMap,
    vec::ObservableVec,
};
use crate::ProgressableObservableValue;
use crate::collections::subscribers_store::BasicSubStore;

pub type ProgressableHashSet<T> = HashSet<T, ProgressableSubStore<T>, ProgressableObservableValue<T>>;
pub type ObservableHashSet<T> = HashSet<T, BasicSubStore<T>, T>;