//! Definitions and implementations of [Control API]'s `Pipeline`.
//!
//! [Control API]: https://tinyurl.com/yxsqplq7

use std::{
    collections::{hash_map::Iter, HashMap},
    iter::IntoIterator,
};

use serde::Deserialize;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline<T> {
    pipeline: HashMap<String, T>,
}

impl<T> Pipeline<T> {
    /// Creates new [`Pipeline`] from provided [`HashMap`].
    pub fn new(pipeline: HashMap<String, T>) -> Self {
        Self { pipeline }
    }

    /// Iterates over pipeline by reference.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.into_iter()
    }

    /// Lookups element of [`Pipeline`] by ID.
    #[inline]
    pub fn get(&self, id: &str) -> Option<&T> {
        self.pipeline.get(id)
    }
}

impl<'a, T> IntoIterator for &'a Pipeline<T> {
    type IntoIter = Iter<'a, String, T>;
    type Item = (&'a String, &'a T);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.iter()
    }
}
