//! Definitions and implementations of [Control API]'s `Pipeline`.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::{hash_map::Iter, HashMap},
    hash::Hash,
    iter::IntoIterator,
};

use serde::Deserialize;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline<K: Hash + Eq, V> {
    pipeline: HashMap<K, V>,
}

impl<K: Hash + Eq, V> Pipeline<K, V> {
    /// Creates a new [`Pipeline`] from the provided [`HashMap`].
    #[inline]
    #[must_use]
    pub fn new(pipeline: HashMap<K, V>) -> Self {
        Self { pipeline }
    }

    /// Iterates over this [`Pipeline`] by reference.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
    }

    /// Lookups an element of this [`Pipeline`] by its ID.
    #[inline]
    #[must_use]
    pub fn get(&self, id: &K) -> Option<&V> {
        self.pipeline.get(id)
    }

    /// Indicates whether this [`Pipeline`] contains a value with the specified
    /// ID.
    #[inline]
    #[must_use]
    pub fn contains_key(&self, id: &K) -> bool {
        self.pipeline.contains_key(id)
    }
}

impl<'a, K: Eq + Hash, V> IntoIterator for &'a Pipeline<K, V> {
    type IntoIter = Iter<'a, K, V>;
    type Item = (&'a K, &'a V);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.iter()
    }
}
