//! Definitions and implementations of [Control API]'s `Pipeline`.
//!
//! [Control API]: http://tiny.cc/380uaz

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
    /// Iterate over pipeline by reference.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.into_iter()
    }

    /// Lookup element of [`Pipeline`] by ID.
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
