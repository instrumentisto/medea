//! Control API specification Pipeline definition.

use std::{
    collections::{
        hash_map::{IntoIter, Iter},
        HashMap,
    },
    iter::IntoIterator,
};

use serde::Deserialize;

use crate::api::control::Element;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pipeline: HashMap<String, Element>,
}

impl Pipeline {
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Element)> {
        self.into_iter()
    }
}

impl IntoIterator for Pipeline {
    type IntoIter = IntoIter<String, Element>;
    type Item = (String, Element);

    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.into_iter()
    }
}

impl<'a> IntoIterator for &'a Pipeline {
    type IntoIter = Iter<'a, String, Element>;
    type Item = (&'a String, &'a Element);

    fn into_iter(self) -> Self::IntoIter {
        self.pipeline.iter()
    }
}
