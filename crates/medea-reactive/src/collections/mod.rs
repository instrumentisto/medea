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

/// Reactive hash set based on [`HashSet`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashSet<T> =
    HashSet<T, progressable::SubStore<T>, progressable::Value<T>>;
/// Reactive hash set based on [`HashSet`].
pub type ObservableHashSet<T> = HashSet<T, common::SubStore<T>, T>;

/// Reactive vector based on [`Vec`] with ability to recognise when all updates
/// was processed by subscribers.
pub type ProgressableVec<T> =
    Vec<T, progressable::SubStore<T>, progressable::Value<T>>;
/// Reactive vector based on [`Vec`].
pub type ObservableVec<T> = Vec<T, common::SubStore<T>, T>;

/// Reactive hash map based on [`HashMap`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashMap<K, V> =
    HashMap<K, V, progressable::SubStore<(K, V)>, progressable::Value<(K, V)>>;
/// Reactive hash map based on [`HashMap`].
pub type ObservableHashMap<K, V> =
    HashMap<K, V, common::SubStore<(K, V)>, (K, V)>;
