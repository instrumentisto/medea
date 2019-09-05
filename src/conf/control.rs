use serde::{Deserialize, Serialize};
use smart_default::SmartDefault;

/// [Control API] settings.
///
/// [Control API]: http://tiny.cc/380uaz
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, SmartDefault)]
#[serde(default)]
pub struct Control {
    /// Path to directory with static [Сontrol API] specs.
    ///
    /// [Control API]: http://tiny.cc/380uaz
    #[default(String::from("specs/"))]
    pub static_specs_dir: String,
}
