//! Control API specification Pipeline definition.

use serde::Deserialize;

use std::collections::HashMap;

use crate::api::control::Element;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pub pipeline: HashMap<String, Element>,
}
