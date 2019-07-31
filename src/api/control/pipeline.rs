//! Control API specification Pipeline definition.

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
pub struct Pipeline<T> {
    pipeline: HashMap<String, T>,
}

impl<T> Pipeline<T> {
    pub fn iter(&self) -> impl Iterator<Item = (&String, &T)> {
        self.into_iter()
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.pipeline.get(id)
    }
}

impl<T> IntoIterator for Pipeline<T> {
    type IntoIter = IntoIter<String, T>;
    type Item = (String, T);

    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Pipeline<T> {
    type IntoIter = Iter<'a, String, T>;
    type Item = (&'a String, &'a T);

    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.iter()
    }
}
