//! Control API specification Pipeline definition.

use crate::api::control::Entity;

use serde::Deserialize;
use std::collections::HashMap;

/// Entity that represents some pipeline of spec.
#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pub pipeline: HashMap<String, Entity>,
}
