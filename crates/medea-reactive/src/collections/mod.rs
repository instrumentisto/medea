//! Implementations of reactive collections based on [`std::collections`].

#![allow(clippy::module_name_repetitions)]

pub mod hash_map;
pub mod hash_set;
pub mod vec;

use crate::subscribers_store::{common, progressable};

/// Reactive hash set based on [`HashSet`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashSet<T> = hash_set::ObservableHashSet<
    T,
    progressable::SubStore<T>,
    progressable::Value<T>,
>;
/// Reactive hash set based on [`HashSet`].
pub type ObservableHashSet<T> =
    hash_set::ObservableHashSet<T, common::SubStore<T>, T>;

/// Reactive vector based on [`Vec`] with ability to recognise when all updates
/// was processed by subscribers.
pub type ProgressableVec<T> =
    vec::ObservableVec<T, progressable::SubStore<T>, progressable::Value<T>>;
/// Reactive vector based on [`Vec`].
pub type ObservableVec<T> = vec::ObservableVec<T, common::SubStore<T>, T>;

/// Reactive hash map based on [`HashMap`] with ability to recognise when all
/// updates was processed by subscribers.
pub type ProgressableHashMap<K, V> = hash_map::ObservableHashMap<
    K,
    V,
    progressable::SubStore<(K, V)>,
    progressable::Value<(K, V)>,
>;
/// Reactive hash map based on [`HashMap`].
pub type ObservableHashMap<K, V> =
    hash_map::ObservableHashMap<K, V, common::SubStore<(K, V)>, (K, V)>;
