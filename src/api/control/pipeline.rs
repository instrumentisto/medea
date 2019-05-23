use crate::api::control::Entity;

use serde::Deserialize;
use std::{collections::HashMap, convert::TryFrom as _};

#[derive(Clone, Deserialize, Debug)]
pub struct Pipeline {
    pub pipeline: HashMap<String, Entity>,
}
