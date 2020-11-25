//! Implementations of reactive collections based on [`std::collections`].

#![allow(clippy::module_name_repetitions)]

pub mod hash_map;
pub mod hash_set;
pub mod vec;

use self::{
    hash_map::ObservableHashMap as HashMap,
    hash_set::ObservableHashSet as HashSet, vec::ObservableVec as Vec,
};

use crate::subscribers_store::{
    common::BasicSubStore,
    progressable::{ProgressableObservableValue, ProgressableSubStore},
};

pub type ProgressableHashSet<T> =
    HashSet<T, ProgressableSubStore<T>, ProgressableObservableValue<T>>;
pub type ObservableHashSet<T> = HashSet<T, BasicSubStore<T>, T>;

pub type ProgressableVec<T> =
    Vec<T, ProgressableSubStore<T>, ProgressableObservableValue<T>>;
pub type ObservableVec<T> = Vec<T, BasicSubStore<T>, T>;

pub type ProgressableHashMap<K, V> = HashMap<
    K,
    V,
    ProgressableSubStore<(K, V)>,
    ProgressableObservableValue<(K, V)>,
>;
pub type ObservableHashMap<K, V> = HashMap<K, V, BasicSubStore<(K, V)>, (K, V)>;
