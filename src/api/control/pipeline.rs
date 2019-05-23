use crate::api::control::Entity;

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pub pipeline: HashMap<String, Entity>,
}
