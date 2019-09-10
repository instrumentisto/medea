//! Definitions and implementations of [Control API]'s `Pipeline`.
//!
//! [Control API]: http://tiny.cc/380uaz

use std::{
    collections::{
        hash_map::{IntoIter, Iter},
        HashMap,
    },
    iter::IntoIterator,
};

use serde::Deserialize;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline<T>(HashMap<String, T>);

impl<T> Pipeline<T> {
    /// Creates new [`Pipeline`] from provided [`HashMap`].
    pub fn new(pipeline: HashMap<String, T>) -> Self {
        Self(pipeline)
    }

    /// Iterate over pipeline by reference.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.into_iter()
    }

    /// Lookup element of [`Pipeline`] by ID.
    #[inline]
    pub fn get(&self, id: &str) -> Option<&T> {
        self.0.get(id)
    }
}

impl<T> IntoIterator for Pipeline<T> {
    type IntoIter = IntoIter<String, T>;
    type Item = (String, T);

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Pipeline<T> {
    type IntoIter = Iter<'a, String, T>;
    type Item = (&'a String, &'a T);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
