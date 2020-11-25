//! Implementations of reactive collections based on [`std::collections`].

#![allow(clippy::module_name_repetitions)]

pub mod hash_map;
pub mod hash_set;
pub mod vec;

use self::{
    hash_map::ObservableHashMap as HashMap,
    hash_set::ObservableHashSet as HashSet, vec::ObservableVec as Vec,
};

use crate::subscribers_store::{common, progressable};

pub type ProgressableHashSet<T> =
    HashSet<T, progressable::SubStore<T>, progressable::Value<T>>;
pub type ObservableHashSet<T> = HashSet<T, common::SubStore<T>, T>;

pub type ProgressableVec<T> =
    Vec<T, progressable::SubStore<T>, progressable::Value<T>>;
pub type ObservableVec<T> = Vec<T, common::SubStore<T>, T>;

pub type ProgressableHashMap<K, V> =
    HashMap<K, V, progressable::SubStore<(K, V)>, progressable::Value<(K, V)>>;
pub type ObservableHashMap<K, V> =
    HashMap<K, V, common::SubStore<(K, V)>, (K, V)>;
