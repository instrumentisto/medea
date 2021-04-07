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
    /// Creates new [`Pipeline`] from provided [`HashMap`].
    #[inline]
    #[must_use]
    pub fn new(pipeline: HashMap<K, V>) -> Self {
        Self { pipeline }
    }

    /// Iterates over pipeline by reference.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.into_iter()
    }

    /// Lookups element of [`Pipeline`] by ID.
    #[inline]
    #[must_use]
    pub fn get(&self, id: &K) -> Option<&V> {
        self.pipeline.get(id)
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
