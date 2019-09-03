use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// Control API settings.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct Control {
    /// Path to directory with static control API specs.
    #[default(String::from("./specs/"))]
    pub static_specs_dir: String,
}
